use std::collections::HashMap;

use ashpd::{
    desktop::{
        dynamic_launcher::{DynamicLauncherProxy, LauncherType, PrepareInstallOptions},
        Icon,
    },
    WindowIdentifier,
};
use gdk_pixbuf::Pixbuf;
use gio::MemoryInputStream;
use gtk::prelude::SettingsExtManual;
use gtk::{gdk, gio, glib};

use crate::{application::settings, config, util::{to_gdk_texture, Image}};

type AppsSettings = HashMap<String, HashMap<String, String>>;

#[derive(Default, Debug, Clone)]
pub struct AppDetails {
    pub id: String,
    pub url: String,
    pub title: String,
    pub icon: Option<Vec<u8>>,
    pub dark_fg: Option<String>,
    pub dark_bg: Option<String>,
    pub light_fg: Option<String>,
    pub light_bg: Option<String>,
}

impl PartialEq for AppDetails {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.url == other.url
            && self.title == other.title
            && self.icon == other.icon
            && self.dark_fg == other.dark_fg
            && self.dark_bg == other.dark_bg
            && self.light_fg == other.light_fg
            && self.light_bg == other.light_bg
    }
}

impl AppDetails {
    pub fn new(id: String, title: String, url: String) -> Self {
        Self {
            id,
            url,
            title,
            ..Default::default()
        }
    }
    pub fn to_hashmap(&self) -> HashMap<String, String> {
        HashMap::from([
            ("url".into(), self.url.clone()),
            ("title".into(), self.title.clone()),
        ])
    }
    pub fn with_icon(self, icon: Vec<u8>) -> Self {
        AppDetails {
            icon: Some(icon),
            ..self
        }
    }
    pub fn to_gdk_texture(&self, size: i32) -> gdk::Texture {
        to_gdk_texture(self.icon.clone().unwrap().as_slice(), size)
    }
}

#[inline]
fn id_to_desktop(id: &str) -> String {
    format!("{}.{}.desktop", config::APP_ID, id)
}

pub fn save_app_details(details: &AppDetails) -> anyhow::Result<()> {
    let settings = settings();
    let mut apps = settings.get::<Vec<String>>("app-ids");
    if !apps.contains(&details.id) {
        apps.push(details.id.clone());
    }

    let mut apps_settings = settings.get::<AppsSettings>("apps-settings");
    apps_settings.insert(details.id.clone(), details.to_hashmap());

    settings.set("app-ids", apps)?;
    settings.set("apps-settings", apps_settings)?;

    Ok(())
}

pub fn delete_app_details(id: &str) -> anyhow::Result<()> {
    let settings = settings();
    let mut apps = settings.get::<Vec<String>>("app-ids");
    if let Some(idx) = apps.iter().position(|x| x == id) {
        apps.remove(idx);
    }
    let mut apps_settings = settings.get::<AppsSettings>("apps-settings");
    if apps_settings.contains_key(id) {
        apps_settings.remove(id);
    }

    settings.set("app-ids", apps)?;
    settings.set("apps-settings", apps_settings)?;

    Ok(())
}

pub async fn get_app_icon(id: &str) -> anyhow::Result<Vec<u8>> {
    let desktop_id = id_to_desktop(id);
    let proxy = DynamicLauncherProxy::new().await?;
    let Icon::Bytes(icon) = proxy.icon(desktop_id.as_str()).await?.icon() else {
        unreachable!();
    };
    Ok(icon)
}

pub fn get_app_details(id: String) -> AppDetails {
    let settings = settings();
    let settings = settings.get::<AppsSettings>("apps-settings");
    let settings = settings.get(&id).unwrap();
    AppDetails {
        id,
        url: settings.get("url").unwrap().to_string(),
        title: settings.get("title").unwrap().to_string(),
        dark_fg: settings.get("darkfg").map(|x| x.to_string()),
        dark_bg: settings.get("darkbg").map(|x| x.to_string()),
        light_fg: settings.get("lightfg").map(|x| x.to_string()),
        light_bg: settings.get("lightbg").map(|x| x.to_string()),
        icon: None,
    }
}

pub async fn uninstall_app(id: &str) -> anyhow::Result<()> {
    let proxy = DynamicLauncherProxy::new().await?;

    proxy.uninstall(&id_to_desktop(id)).await?;
    delete_app_details(id)?;

    Ok(())
}

pub async fn install_app(
    details: &AppDetails,
    icon: Vec<u8>,
    wid: &WindowIdentifier,
) -> anyhow::Result<()> {
    let proxy = DynamicLauncherProxy::new().await?;
    let icon = Icon::Bytes(icon);

    let options = PrepareInstallOptions::default()
        .modal(true)
        .editable_icon(false)
        .editable_name(false)
        .launcher_type(LauncherType::Application);

    let response = proxy
        .prepare_install(wid, details.title.as_str(), icon, options)
        .await?
        .response()?;

    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Terminal=false
Type=Application
Categories=Network;
Exec=env spider {}"#,
        details.title, details.id
    );
    proxy
        .install(
            response.token(),
            id_to_desktop(details.id.as_str()).as_str(),
            desktop_content.as_str(),
        )
        .await?;

    save_app_details(details)?;

    Ok(())
}
