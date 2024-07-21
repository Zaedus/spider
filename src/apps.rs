use std::collections::HashMap;

use ashpd::{
    desktop::{
        dynamic_launcher::{DynamicLauncherProxy, LauncherType, PrepareInstallOptions},
        Icon,
    },
    WindowIdentifier,
};
use gtk::prelude::SettingsExtManual;

use crate::{application::settings, config, util::Image};

type AppsSettings = HashMap<String, HashMap<String, String>>;

#[derive(Default, Debug)]
pub struct AppDetails {
    pub id: String,
    pub url: String,
    pub title: String,
    pub icon: Option<Vec<u8>>,
}

impl AppDetails {
    pub fn new(id: String, title: String, url: String,) -> Self {
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
}

#[inline]
fn id_to_desktop(id: String) -> String {
    format!("{}.{}.desktop", config::APP_ID, id)
}

fn save_app(details: AppDetails) -> anyhow::Result<()> {
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

pub async fn get_app_icon(id: String) -> anyhow::Result<Vec<u8>> {
    let desktop_id = id_to_desktop(id.clone());
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
        icon: None,
    }
}

pub async fn install_app(
    details: AppDetails,
    icon: Image,
    wid: &WindowIdentifier,
) -> anyhow::Result<()> {
    let proxy = DynamicLauncherProxy::new().await?;
    let icon = Icon::Bytes(icon.buffer);

    let options = PrepareInstallOptions::default()
        .modal(true)
        .editable_icon(true)
        .editable_name(true)
        .launcher_type(LauncherType::Application);

    let response = proxy
        .prepare_install(wid, details.title.as_str(), icon, options)
        .await?
        .response()?;
    println!("{:?}", response.icon());

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
            id_to_desktop(details.id.clone()).as_str(),
            desktop_content.as_str(),
        )
        .await?;

    save_app(details)?;

    Ok(())
}
