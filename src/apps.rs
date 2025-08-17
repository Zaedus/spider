use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::bail;
use ashpd::{
    desktop::{
        dynamic_launcher::{DynamicLauncherProxy, LauncherType, PrepareInstallOptions},
        Icon,
    },
    WindowIdentifier,
};
use dircpy::copy_dir;
use gtk::prelude::SettingsExtManual;
use gtk::{gdk, glib};
use lazy_static::lazy_static;

use crate::{application::settings, config, util::to_gdk_texture};

pub type AppsSettings = HashMap<String, HashMap<String, String>>;

lazy_static! {
    static ref data_dir: PathBuf = glib::user_data_dir().join(glib::application_name().unwrap());
    static ref cache_dir: PathBuf = glib::user_cache_dir().join(glib::application_name().unwrap());
}

#[derive(Debug, Clone)]
pub struct AppDetails {
    pub id: String,
    pub url: String,
    pub title: String,
    pub icon: Option<Vec<u8>>,
    pub has_titlebar_color: bool,
    pub window_width: i32,
    pub window_height: i32,
    pub window_maximize: bool,
    pub user_agent: Option<String>,
}

impl PartialEq for AppDetails {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.url == other.url
            && self.title == other.title
            && self.icon == other.icon
            && self.has_titlebar_color == other.has_titlebar_color
            && self.user_agent == other.user_agent
    }
}

impl Default for AppDetails {
    fn default() -> Self {
        Self {
            id: "".into(),
            url: "".into(),
            title: "".into(),
            has_titlebar_color: true,
            icon: None,
            window_width: 400,
            window_height: 400,
            window_maximize: false,
            user_agent: None,
        }
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
            (
                "hastitlebarcolor".to_string(),
                self.has_titlebar_color.to_string(),
            ),
            ("windowwidth".to_string(), self.window_width.to_string()),
            ("windowheight".to_string(), self.window_height.to_string()),
            (
                "windowmaximize".to_string(),
                self.window_maximize.to_string(),
            ),
        ];
        if let Some(user_agent) = &self.user_agent {
            kv_pairs.push(("useragent".to_string(), user_agent.clone()));
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
    pub fn save(&self) -> anyhow::Result<()> {
        let settings = settings();
        let mut apps = settings.get::<Vec<String>>("app-ids");
        if !apps.contains(&self.id) {
            apps.push(self.id.clone());
        }

        let mut apps_settings = settings.get::<AppsSettings>("apps-settings");
        apps_settings.insert(self.id.clone(), self.to_hashmap());

        settings.set("app-ids", apps)?;
        settings.set("apps-settings", apps_settings)?;

        Ok(())
    }
}

#[inline]
fn id_to_desktop(id: &str) -> String {
    format!("{}.{}.desktop", config::APP_ID, id)
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

pub fn get_app_details(id: &str) -> Option<AppDetails> {
    let settings = settings();
    let settings = settings.get::<AppsSettings>("apps-settings");
    let settings = settings.get(id)?;
    Some(AppDetails {
        id: id.to_string(),
        url: settings.get("url").unwrap().to_string(),
        title: settings.get("title").unwrap().to_string(),
        has_titlebar_color: settings
            .get("hastitlebarcolor")
            .is_none_or(|x| x != "false"),
        icon: None,
        window_width: settings
            .get("windowwidth")
            .and_then(|x| x.parse::<i32>().ok())
            .unwrap_or(400),
        window_height: settings
            .get("windowheight")
            .and_then(|x| x.parse::<i32>().ok())
            .unwrap_or(400),
        window_maximize: settings
            .get("windowmaximize")
            .and_then(|x| x.parse::<bool>().ok())
            .unwrap_or(false),
        user_agent: settings.get("useragent").map(|x| x.to_string()),
    })
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

    let response = match proxy
        .prepare_install(wid, details.title.as_str(), icon, options)
        .await
    {
        Err(ashpd::Error::Zbus(ashpd::zbus::Error::MethodError(_, msg, _))) => {
            let mut msg = msg.unwrap_or("unknown".to_string());
            if msg == "Dynamic launcher icon failed validation" {
                msg = "Invalid icon, maybe bad size or format".to_string();
            }
            bail!(msg);
        }
        Err(err) => return Err(err.into()),
        Ok(good) => good,
    }
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

    details.save()?;

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

pub fn copy_app_dir(old_id: &str, new_id: &str) -> anyhow::Result<()> {
    for folder in [data_dir.to_path_buf(), cache_dir.to_path_buf()] {
        if !folder.exists() {
            continue;
        }
        for item in std::fs::read_dir(folder.clone()).unwrap().flatten() {
            if item.file_type().unwrap().is_dir() && item.file_name().to_string_lossy() == old_id {
                copy_dir(folder.join(old_id), folder.join(new_id))?;
                break;
            }
        }
    }
    Ok(())
}
