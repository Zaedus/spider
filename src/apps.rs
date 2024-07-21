use std::collections::HashSet;

use ashpd::{
    desktop::{
        dynamic_launcher::{DynamicLauncherProxy, LauncherType, PrepareInstallOptions},
        Icon,
    },
    WindowIdentifier,
};
use gtk::{gio, prelude::SettingsExtManual};

use crate::{config, util::Image};

pub struct AppData {
    pub id: String,
    pub title: String,
}

fn save_app(id: String) -> anyhow::Result<()> {
    let settings = gio::Settings::new(config::APP_ID);
    let mut apps = settings.get::<Vec<String>>("app-ids");
    if !apps.contains(&id) {
        apps.push(id);
    }
    settings.set("app-ids", apps)?;

    Ok(())
}

pub async fn install_app(
    app_data: AppData,
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
        .prepare_install(wid, app_data.title.as_str(), icon, options)
        .await?
        .response()?;

    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Terminal=false
Type=Application
Categories=Network;
Exec=env spider {}"#,
        app_data.title, app_data.id
    );
    println!("{}", desktop_content);
    proxy
        .install(
            response.token(),
            format!("{}.{}.desktop", config::APP_ID, app_data.id).as_str(),
            desktop_content.as_str(),
        )
        .await?;

    save_app(app_data.id)?;

    Ok(())
}
