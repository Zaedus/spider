use anyhow::bail;
use anyhow::{anyhow, Result};
use futures::future::join_all;
use gtk::{gdk, gio, glib, prelude::*};
use isahc::{config, prelude::*};
use lazy_static::lazy_static;
use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

use webkit::prelude::WebViewExt;

#[derive(Debug)]
pub struct WebsiteMeta {
    pub icon: Option<Image>,
    pub title: Option<String>,
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
    pub async fn from_buffer(buffer: Vec<u8>, is_svg: bool) -> Result<Self> {
        if is_svg {
            return Ok(Image {
                buffer,
                size: ImageSize::Variable,
            });
        }
        let mut loader = glycin::Loader::new_vec(buffer);
        loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);
        let glycin_image = loader.load().await.map_err(|e| anyhow!("{e}"))?;
        let details = glycin_image.details();
        let (w, h) = (details.width(), details.height());
        if w != h {
            bail!("Image is not square")
        }

        let texture = glycin_image
            .next_frame()
            .await
            .map_err(|e| anyhow!("{e}"))?
            .texture();
        let png = texture.save_to_png_bytes().to_vec();
        Ok(Image {
            buffer: png,
            size: ImageSize::Sized((w, h)),
        })
    }
    pub async fn load_texture(&self) -> Result<gdk::Texture> {
        load_texture(self.buffer.clone()).await
    }
}

lazy_static! {
    static ref icon_selector: Selector = Selector::parse(
        "link[rel='icon'], link[rel='shortcut icon'], link[rel^='apple-touch-icon']"
    )
    .unwrap();
    static ref title_selctor: Selector = Selector::parse("title").unwrap();
    static ref http: isahc::HttpClient = isahc::HttpClient::builder()
        .redirect_policy(config::RedirectPolicy::Limit(10))
        .build()
        .unwrap();
}

// Sane size to render SVGs to that's better than the default
const SVG_RENDER_SIZE: u32 = 256;

pub async fn load_texture(buffer: Vec<u8>) -> Result<gdk::Texture> {
    let mut loader = glycin::Loader::new_vec(buffer);
    loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);
    let image = loader.load().await.map_err(|e| anyhow!("{e}"))?;
    let mime = image.mime_type();
    let frame = if matches!(mime.as_str(), "image/svg+xml" | "image/svg+xml-compressed") {
        image
            .specific_frame(glycin::FrameRequest::new().scale(SVG_RENDER_SIZE, SVG_RENDER_SIZE))
            .await
    } else {
        image.next_frame().await
    }
    .map_err(|e| anyhow!("{e}"))?;
    Ok(frame.texture())
}

async fn get_image_metadata(url: Url) -> Result<Image> {
    let mut response = http.get_async(url.to_string()).await?;
    if !response.status().is_success() {
        bail!("{url}: HTTP {}", response.status());
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let is_svg = content_type.as_deref() == Some("image/svg+xml")
        || url
            .path_segments()
            .and_then(|mut x| x.next_back())
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".svg"));
    if let Some(ct) = content_type.as_deref() {
        if !is_svg && !ct.starts_with("image/") {
            bail!("{url}: not an image (Content-Type: {ct})");
        }
    }
    let buffer = response.bytes().await?;

    Image::from_buffer(buffer, is_svg)
        .await
        .map_err(|e| anyhow!("{url}: {e}"))
}

pub async fn get_website_meta(url: Url) -> Result<WebsiteMeta> {
    let mut req = http.get_async(url.to_string()).await?;
    let html = req.text().await?;
    let url: Url = Url::parse(req.effective_uri().unwrap().to_string().as_str()).unwrap();
    let doc = Html::parse_document(html.as_str());
    let mut paths = doc
        .select(&icon_selector)
        .filter_map(|elm| elm.attr("href"))
        .collect::<HashSet<&str>>();
    paths.insert("favicon.ico");
    paths.insert("favicon.png");
    paths.insert("/favicon.ico");
    paths.insert("/favicon.png");
    let paths = paths
        .into_iter()
        .filter_map(|path| url.join(path).ok())
        .collect::<HashSet<Url>>();
    let metadata = join_all(paths.into_iter().map(get_image_metadata)).await;
    let best_image = metadata
        .iter()
        .filter_map(|x| x.as_ref().ok())
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
    Ok(WebsiteMeta {
        icon: best_image.cloned(),
        title,
    })
}

/// How long to wait for a page to load in the hidden metadata webview
const WEBVIEW_META_TIMEOUT_SECS: u32 = 20;

/// Extracts website data (title + favicon candidates) by rendering the page
/// in an offscreen ephemeral WebKitWebView and running JavaScript on it.
///
/// Unlike [`get_website_meta`], this sees the page exactly like the browser
/// will: JS-rendered titles/icons are picked up and responses that block
/// non-browser clients still succeed.
pub async fn get_website_meta_via_webview(url: Url) -> Result<WebsiteMeta> {
    let uri = url.to_string();

    let context = webkit::WebContext::new();
    let session = webkit::NetworkSession::new_ephemeral();
    let settings = webkit::Settings::builder()
        .enable_javascript_markup(true)
        .media_playback_requires_user_gesture(true)
        .enable_html5_database(false)
        .enable_html5_local_storage(false)
        .build();
    let webview = webkit::WebView::builder()
        .web_context(&context)
        .network_session(&session)
        .settings(&settings)
        .user_content_manager(&webkit::UserContentManager::new())
        .build();

    let (sender, receiver) = async_channel::bounded::<Result<WebsiteMeta>>(1);

    // Collects everything in one pass. Icon hrefs are absolute since
    // `link.href` resolves them against the document. Tab/newline are
    // stripped so the result can be parsed line-by-line.
    const EXTRACT_SCRIPT: &str = r#"
(() => {
  const lines = [];
  lines.push(`T\t${(document.title || "").replace(/[\t\r\n]/g, " ")}`);
  for (const link of document.querySelectorAll(
    "link[rel~='icon'], link[rel='shortcut icon'], link[rel='apple-touch-icon']"
  )) {
    if (!link.href) continue;
    let size = 0;
    const sizes = (link.sizes && link.sizes.value) || "";
    for (const match of sizes.matchAll(/(\d+)x\d+/g)) {
      size = Math.max(size, parseInt(match[1]));
    }
    if (!Number.isFinite(size)) size = 0;
    lines.push(`I\t${size}\t${link.href}`);
  }
  return lines.join("\n");
})()
"#;

    webview.connect_load_changed(move |webview, event| {
        if event != webkit::LoadEvent::Finished {
            return;
        }
        let webview = webview.clone();
        let sender = sender.clone();
        let url = url.clone();
        glib::spawn_future_local(async move {
            let result = match webview
                .evaluate_javascript_future(EXTRACT_SCRIPT, None, None)
                .await
            {
                Ok(value) => {
                    let text = value
                        .to_string_as_bytes()
                        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                        .unwrap_or_default();
                    parse_meta_text(&text, &url).await
                }
                Err(err) => Err(anyhow!("{err}")),
            };
            let _ = sender.send(result).await;
        });
    });

    webview.load_uri(uri.as_str());

    match futures::future::select(
        Box::pin(receiver.recv()),
        Box::pin(glib::timeout_future_seconds(WEBVIEW_META_TIMEOUT_SECS)),
    )
    .await
    {
        futures::future::Either::Left((result, _)) => result.map_err(|e| anyhow!("{e}"))?,
        futures::future::Either::Right(_) => bail!("Timed out loading {uri}"),
    }
}

/// Parses the tab-separated output of [`EXTRACT_SCRIPT`]:
/// `T\t<title>` then `I\t<size>\t<href>` lines.
async fn parse_meta_text(text: &str, url: &Url) -> Result<WebsiteMeta> {
    let mut title = None;
    let mut icons: Vec<(u32, Url)> = Vec::new();

    for line in text.lines() {
        match line.split_once('\t') {
            Some(("T", value)) => {
                if !value.is_empty() {
                    title = Some(value.to_string());
                }
            }
            Some(("I", rest)) => {
                if let Some((size, href)) = rest.split_once('\t') {
                    if let (Ok(size), Ok(href)) = (size.parse::<u32>(), url.join(href)) {
                        icons.push((size, href));
                    }
                }
            }
            _ => (),
        }
    }

    // Prefer the biggest declared icon; fall back to whatever exists
    let best = icons.iter().max_by_key(|(size, _)| *size);
    let icon = match best {
        Some((_, href)) => Some(get_image_metadata(href.clone()).await?),
        None => None,
    };

    Ok(WebsiteMeta { icon, title })
}

pub async fn icon_from_dialog(
    window: Option<&(impl IsA<gtk::Window> + Clone + 'static)>,
) -> Result<gio::File> {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Images"));
    for mime in [
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/svg+xml",
        "image/gif",
        "image/bmp",
        "image/x-icon",
        "image/vnd.microsoft.icon",
        "image/avif",
        "image/heif",
        "image/tiff",
    ] {
        filter.add_mime_type(mime);
    }

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let file = gtk::FileDialog::builder()
        .accept_label("Select")
        .modal(true)
        .title("App Icon")
        .filters(&filters)
        .build()
        .open_future(window)
        .await?;

    Ok(file)
}
