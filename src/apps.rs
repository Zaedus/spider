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

use crate::{application::settings, config, util::load_texture};

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
    /// When set, navigation inside the app is restricted to these domains
    /// (subdomains included). `None` means unrestricted.
    pub allowed_domains: Option<Vec<String>>,
    pub proxy_url: Option<String>,
    pub autostart: bool,
    pub run_in_background: bool,
}

impl PartialEq for AppDetails {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.url == other.url
            && self.title == other.title
            && self.icon == other.icon
            && self.has_titlebar_color == other.has_titlebar_color
            && self.user_agent == other.user_agent
            && self.allowed_domains == other.allowed_domains
            && self.proxy_url == other.proxy_url
            && self.autostart == other.autostart
            && self.run_in_background == other.run_in_background
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
            allowed_domains: None,
            proxy_url: None,
            autostart: false,
            run_in_background: false,
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
        let kv_pairs = vec![
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
            // Optional fields are always stored (empty string = unset) so
            // they can be cleared again; saves merge over previous entries
            (
                "useragent".to_string(),
                self.user_agent.clone().unwrap_or_default(),
            ),
            (
                "domains".to_string(),
                self.allowed_domains
                    .as_ref()
                    .map(|d| d.join(","))
                    .unwrap_or_default(),
            ),
            (
                "proxyurl".to_string(),
                self.proxy_url.clone().unwrap_or_default(),
            ),
            ("autostart".to_string(), self.autostart.to_string()),
            ("background".to_string(), self.run_in_background.to_string()),
        ];

        kv_pairs.into_iter().collect()
    }
    pub fn with_icon(self, icon: Vec<u8>) -> Self {
        AppDetails {
            icon: Some(icon),
            ..self
        }
    }
    pub async fn load_texture(&self) -> anyhow::Result<gdk::Texture> {
        load_texture(self.icon.clone().unwrap()).await
    }
    pub fn save(&self) -> anyhow::Result<()> {
        let settings = settings();
        let mut apps = settings.get::<Vec<String>>("app-ids");
        if !apps.contains(&self.id) {
            apps.push(self.id.clone());
        }

        let mut apps_settings = settings.get::<AppsSettings>("apps-settings");
        // Merge into any existing entry so data managed elsewhere
        // (e.g. website permissions) survives saves made from stale
        // copies of the details
        let mut kv_pairs = self.to_hashmap();
        if let Some(existing) = apps_settings.get(&self.id) {
            for (key, value) in existing {
                kv_pairs.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        apps_settings.insert(self.id.clone(), kv_pairs);

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

/// Mutates the stored settings map for a single app in place.
fn mutate_app_settings(
    id: &str,
    f: impl FnOnce(&mut HashMap<String, String>),
) -> anyhow::Result<()> {
    let settings = settings();
    let mut apps_settings = settings.get::<AppsSettings>("apps-settings");
    let Some(entry) = apps_settings.get_mut(id) else {
        anyhow::bail!("No app with id {id}");
    };
    f(entry);
    settings.set("apps-settings", apps_settings)?;
    Ok(())
}

/// Prefix used for all website permission entries inside an app's
/// settings map. Keys look like `perm:<origin>:<kind>` and values are
/// `"allow"` or `"deny"`.
pub const PERMISSION_PREFIX: &str = "perm:";

pub const PERMISSION_LABELS: &[(&str, &str)] = &[
    ("camera", "Camera"),
    ("microphone", "Microphone"),
    ("geolocation", "Location"),
    ("notifications", "Notifications"),
];

pub fn permission_label(kind: &str) -> String {
    PERMISSION_LABELS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| kind.to_string())
}

/// Saves the user's decision for `origin` + permission `kind`.
/// `allow == false` stores an explicit denial (so we won't ask again).
pub fn set_app_permission(id: &str, origin: &str, kind: &str, allow: bool) -> anyhow::Result<()> {
    mutate_app_settings(id, |entry| {
        entry.insert(
            format!("{PERMISSION_PREFIX}{origin}:{kind}"),
            if allow { "allow" } else { "deny" }.to_string(),
        );
    })
}

/// The saved decision for `origin` + permission `kind`, if any.
pub fn get_app_permission(id: &str, origin: &str, kind: &str) -> Option<bool> {
    let apps_settings = settings().get::<AppsSettings>("apps-settings");
    apps_settings
        .get(id)?
        .get(format!("{PERMISSION_PREFIX}{origin}:{kind}").as_str())
        .map(|value| value == "allow")
}

/// Removes every stored decision for `origin`, revoking all of its grants.
pub fn clear_origin_permissions(id: &str, origin: &str) -> anyhow::Result<()> {
    mutate_app_settings(id, |entry| {
        let prefix = format!("{PERMISSION_PREFIX}{origin}:");
        entry.retain(|key, _| !key.starts_with(&prefix));
    })
}

/// Returns `(origin, granted kinds)` pairs for every origin that has at
/// least one saved decision, sorted by origin.
pub fn get_permission_summaries(id: &str) -> Vec<(String, Vec<String>)> {
    let mut summaries: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(entry) = settings().get::<AppsSettings>("apps-settings").get(id) {
        for (key, value) in entry {
            let Some(rest) = key.strip_prefix(PERMISSION_PREFIX) else {
                continue;
            };
            // Origins contain colons (https://host:port), kinds don't, so
            // split from the right
            let Some((origin, kind)) = rest.rsplit_once(':') else {
                continue;
            };
            if value != "deny" {
                summaries
                    .entry(origin.to_string())
                    .or_default()
                    .push(kind.to_string());
            }
        }
    }
    let mut summaries: Vec<(String, Vec<String>)> = summaries.into_iter().collect();
    summaries.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, kinds) in &mut summaries {
        kinds.sort();
    }
    summaries
}

fn autostart_desktop_path(id: &str) -> PathBuf {
    glib::user_config_dir()
        .join("autostart")
        .join(id_to_desktop(id))
}

/// Creates or removes an XDG autostart entry for the app.
///
/// The generated desktop file reuses the `Icon=` line from the launcher's
/// own desktop file when it can be found so the autostart entry shows the
/// same icon as the installed app.
pub fn set_app_autostart(details: &AppDetails, enabled: bool) -> anyhow::Result<()> {
    let path = autostart_desktop_path(&details.id);
    if !enabled {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }

    // Inherit the icon from the installed launcher if possible
    let icon_line = [glib::user_data_dir()]
        .into_iter()
        .chain(glib::system_data_dirs())
        .map(|dir| dir.join("applications").join(id_to_desktop(&details.id)))
        .find_map(|desktop_path| {
            std::fs::read_to_string(desktop_path)
                .ok()
                .and_then(|content| {
                    content.lines().find_map(|line| {
                        line.strip_prefix("Icon=")
                            .map(|icon| format!("Icon={}", icon.trim()))
                    })
                })
        });

    let content = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=env spider {}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n{}\n",
        details.title,
        details.id,
        icon_line.unwrap_or_else(|| format!("Icon={}", config::APP_ID)),
    );

    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, content)?;
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
        user_agent: settings
            .get("useragent")
            .map(|x| x.to_string())
            .filter(|x| !x.is_empty()),
        allowed_domains: settings.get("domains").and_then(|x| {
            let domains: Vec<String> = x
                .split(',')
                .map(|d| d.trim().to_lowercase())
                .filter(|d| !d.is_empty())
                .collect();
            (!domains.is_empty()).then_some(domains)
        }),
        proxy_url: settings
            .get("proxyurl")
            .map(|x| x.to_string())
            .filter(|x| !x.is_empty()),
        autostart: settings
            .get("autostart")
            .and_then(|x| x.parse::<bool>().ok())
            .unwrap_or(false),
        run_in_background: settings
            .get("background")
            .and_then(|x| x.parse::<bool>().ok())
            .unwrap_or(false),
    })
}

pub async fn uninstall_app(id: &str) -> anyhow::Result<()> {
    let proxy = DynamicLauncherProxy::new().await?;

    proxy
        .uninstall(&id_to_desktop(id), Default::default())
        .await?;
    let app_data_dir = data_dir.join(id);
    let app_cache_dir = cache_dir.join(id);
    if app_data_dir.exists() {
        std::fs::remove_dir_all(app_data_dir)?;
    }
    if app_cache_dir.exists() {
        std::fs::remove_dir_all(app_cache_dir)?;
    }
    let autostart_path = autostart_desktop_path(id);
    if autostart_path.exists() {
        std::fs::remove_file(autostart_path)?;
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
        .set_modal(true)
        .set_editable_icon(false)
        .set_editable_name(false)
        .set_launcher_type(LauncherType::Application);

    let response = match proxy
        .prepare_install(Some(wid), details.title.as_str(), icon, options)
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
            Default::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_storage_roundtrip() {
        let details = AppDetails::new(
            "testperm01".to_string(),
            "Test".to_string(),
            "https://example.com".to_string(),
        );
        details.save().unwrap();

        set_app_permission("testperm01", "https://example.com", "camera", true).unwrap();
        set_app_permission("testperm01", "https://example.com", "microphone", true).unwrap();
        set_app_permission("testperm01", "https://other.example.com:8443", "geolocation", false)
            .unwrap();

        assert_eq!(
            get_app_permission("testperm01", "https://example.com", "camera"),
            Some(true)
        );

        let summaries = get_permission_summaries("testperm01");
        let (_, kinds) = summaries
            .iter()
            .find(|(o, _)| o == "https://example.com")
            .expect("allowed origin missing from summaries");
        assert!(kinds.contains(&"camera".to_string()));
        assert!(kinds.contains(&"microphone".to_string()));
        // Origins whose stored decisions are all "deny" are not listed.
        assert!(!summaries.iter().any(|(o, _)| o == "https://other.example.com:8443"));

        clear_origin_permissions("testperm01", "https://example.com").unwrap();
        assert_eq!(
            get_app_permission("testperm01", "https://example.com", "camera"),
            None
        );
        // A save must not wipe unrelated permission keys.
        set_app_permission("testperm01", "https://example.com", "notifications", true).unwrap();
        details.save().unwrap();
        assert_eq!(
            get_app_permission("testperm01", "https://example.com", "notifications"),
            Some(true)
        );

        delete_app_details("testperm01").unwrap();
    }
}
