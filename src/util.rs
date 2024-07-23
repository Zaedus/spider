use anyhow::bail;
use futures::future::join_all;
use gdk_pixbuf::Pixbuf;
use gtk::{gdk, gio, glib};
use image::{codecs::png::PngEncoder, ImageEncoder, ImageFormat};
use isahc::{config, prelude::*};
use lazy_static::lazy_static;
use scraper::{Html, Selector};
use std::{collections::HashSet, path::Path};
use url::Url;

#[derive(Debug)]
pub struct WebsiteMeta {
    pub icon: Image,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ImageSize {
    Sized((u32, u32)),
    Variable,
}

impl ImageSize {
    pub fn size(&self) -> u32 {
        match self {
            ImageSize::Variable => u32::MAX,
            ImageSize::Sized((w, _)) => *w,
        }
    }
}

impl PartialOrd for ImageSize {
    fn lt(&self, other: &Self) -> bool {
        self.size().lt(&other.size())
    }
    fn le(&self, other: &Self) -> bool {
        self.size().le(&other.size())
    }
    fn gt(&self, other: &Self) -> bool {
        self.size().gt(&other.size())
    }
    fn ge(&self, other: &Self) -> bool {
        self.size().ge(&other.size())
    }
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImageSize {
    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.size() > other.size() {
            self
        } else {
            other
        }
    }
    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.size() < other.size() {
            self
        } else {
            other
        }
    }
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.size().cmp(&other.size())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Image {
    pub buffer: Vec<u8>,
    pub size: ImageSize,
}

impl PartialOrd for Image {
    fn lt(&self, other: &Self) -> bool {
        self.size.lt(&other.size)
    }
    fn le(&self, other: &Self) -> bool {
        self.size.le(&other.size)
    }
    fn gt(&self, other: &Self) -> bool {
        self.size.gt(&other.size)
    }
    fn ge(&self, other: &Self) -> bool {
        self.size.ge(&other.size)
    }
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Image {
    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.size > other.size {
            self
        } else {
            other
        }
    }
    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.size < other.size {
            self
        } else {
            other
        }
    }
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.size.cmp(&other.size)
    }
}

impl Image {
    pub fn from_buffer(buffer: Vec<u8>, is_svg: bool) -> anyhow::Result<Self> {
        if is_svg {
            Ok(Image {
                buffer, 
                size: ImageSize::Variable
            })
        } else {
            let format = image::guess_format(buffer.as_slice())?;
            let image = image::load_from_memory_with_format(buffer.as_slice(), format)?;
            if image.width() != image.height() {
                bail!("Image is not square")
            }
            let buffer = if format != ImageFormat::Png {
                let mut encbuf = Vec::new();
                PngEncoder::new(&mut encbuf).write_image(
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ExtendedColorType::Rgba8,
                )?;
                encbuf
            } else {
                buffer
            };
            Ok(Image {
                buffer, 
                size: ImageSize::Sized((image.width(), image.height()))
            })
        }
    }
    pub fn to_gdk_texture(&self, size: i32) -> gdk::Texture {
        to_gdk_texture(&self.buffer, size)
    }
}

lazy_static! {
    static ref icon_selector: Selector = Selector::parse(
        "link[rel='icon'], link[rel='shortcut icon'], link[rel^='apple-touch-icon']"
    )
    .unwrap();
    static ref title_selctor: Selector = Selector::parse("title").unwrap();
    static ref http: isahc::HttpClient = isahc::HttpClient::builder()
        .redirect_policy(config::RedirectPolicy::Limit(3))
        .build()
        .unwrap();
}

pub fn to_gdk_texture(buffer: &[u8], size: i32) -> gdk::Texture {
    let bytes = glib::Bytes::from(buffer);
    let stream = gio::MemoryInputStream::from_bytes(&bytes);
    let pixbuf =
        Pixbuf::from_stream_at_scale(&stream, size, size, true, gio::Cancellable::NONE)
            .unwrap();
    gdk::Texture::for_pixbuf(&pixbuf)
}


async fn get_image_metadata(url: Url) -> anyhow::Result<Image> {
    let mut response = http.get_async(url.to_string()).await?;
    let buffer = response.bytes().await?;
    let extension = url.path_segments()
        .and_then(|x| x.last())
        .and_then(|x| Path::new(x).extension())
        .and_then(|x| x.to_str());

    Image::from_buffer(buffer, extension.is_some_and(|x| x == "svg"))
}

pub async fn get_website_meta(url: Url) -> anyhow::Result<WebsiteMeta> {
    let html = http.get_async(url.to_string()).await?.text().await?;
    let doc = Html::parse_document(html.as_str());
    let mut paths = doc
        .select(&icon_selector)
        .filter_map(|elm| elm.attr("href"))
        .collect::<HashSet<&str>>();
    paths.insert("/favicon.ico");
    paths.insert("/favicon.png");

    println!("{:?}", paths);
    println!("{:?}", url.join("/favicon.ico").map(|x| x.to_string()));
    let metadata = join_all(
        paths
            .into_iter()
            .filter_map(|path| url.join(path).ok())
            .map(get_image_metadata),
    )
    .await;
    let best_image = metadata
        .iter()
        .filter_map(|x| {
            println!("{x:?}");
            x.as_ref().ok()
        })
        .filter(|x| {
            if let ImageSize::Sized((w, _)) = x.size {
                w <= 256
            } else {
                true
            }
        })
        .fold(None, |acc: Option<&Image>, x| {
            Some(match acc {
                Some(a) => a.max(x),
                None => x,
            })
        });
    let title = doc
        .select(&title_selctor)
        .map(|x| x.text())
        .next()
        .map(|x| x.collect::<String>());
    if let Some(icon) = best_image.cloned() {
        if let Some(title) = title {
            Ok(WebsiteMeta { icon, title })
        } else {
            bail!("failed to fetch title")
        }
    } else {
        bail!("failed to fetch icon")
    }
}
