use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use ashpd::{
    desktop::{
        dynamic_launcher::{DynamicLauncherProxy, LauncherType, PrepareInstallOptions},
        Icon,
    },
    WindowIdentifier,
};
use gtk::prelude::SettingsExtManual;
use gtk::{gdk, glib};
use lazy_static::lazy_static;

use crate::{application::settings, config, util::to_gdk_texture};

type AppsSettings = HashMap<String, HashMap<String, String>>;

lazy_static! {
    static ref data_dir: PathBuf = glib::user_data_dir().join(glib::application_name().unwrap());
    static ref cache_dir: PathBuf = glib::user_cache_dir().join(glib::application_name().unwrap());
}

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
        let mut kv_pairs = vec![
            ("url".to_string(), self.url.clone()),
            ("title".to_string(), self.title.clone()),
        ];
        if let Some(v) = &self.dark_fg {
            kv_pairs.push(("darkfg".into(), v.clone()));
        }
        if let Some(v) = &self.dark_bg {
            kv_pairs.push(("darkbg".into(), v.clone()));
        }
        if let Some(v) = &self.light_fg {
            kv_pairs.push(("lightfg".into(), v.clone()));
        }
        if let Some(v) = &self.light_bg {
            kv_pairs.push(("lightbg".into(), v.clone()));
        }

        kv_pairs.into_iter().collect()
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
    let app_data_dir = data_dir.join(id);
    let app_cache_dir = cache_dir.join(id);
    if app_data_dir.exists() {
        std::fs::remove_dir_all(app_data_dir)?;
    }
    if app_cache_dir.exists() {
        std::fs::remove_dir_all(app_cache_dir)?;
    }
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

/// Removes all instances of app folders whos IDs no longer exist
/// THIS IS A HALF SOLUTION AND PATCH ON A PROBLEM
/// The patch: Currently, webkit still holds access to some files even a bit after
/// the window has been closed which causes many problems with fully purging
/// these directories during or shortly after running.
/// The half solution: It is good to ensure that there aren't hidden artifiacts of
/// "deleted" web apps which could contain tokens.
pub fn clean_app_dirs() -> anyhow::Result<()> {
    let settings = settings();
    let app_ids: HashSet<String> = settings.get::<Vec<String>>("app-ids").into_iter().collect();
    for folder in [data_dir.to_path_buf(), cache_dir.to_path_buf()] {
        if !folder.exists() {
            continue;
        }
        for item in std::fs::read_dir(folder).unwrap().flatten() {
            if item.file_type().unwrap().is_dir()
                && !app_ids.contains(&item.file_name().to_string_lossy().to_string())
            {
                std::fs::remove_dir_all(item.path()).unwrap();
            }
        }
    }
    Ok(())
}
