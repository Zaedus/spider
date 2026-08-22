use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Object;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;

use ashpd::WindowIdentifier;

use crate::apps::{self, AppDetails};
use crate::application;
use crate::util;

fn menu_item_and_target(label: &str, action_name: &str, action_target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action_name), Some(&action_target.to_variant()));
    item
}

#[derive(Debug, PartialEq)]
enum DiffSignificance {
    // No difference between app details
    NoDifference,
    // Some settings need changing
    Settings,
    // Necessary to request permission to reinstall desktop file
    DesktopReinstall,
}

mod imp {

    use anyhow::anyhow;

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/zaedus/spider/app_page.ui")]
    pub struct AppPage {
        // TODO: Make this a GObject property somehow
        details: RefCell<AppDetails>,

        // Better to wrap in an Option to avoid overwriting with empty data
        unsaved_details: RefCell<Option<AppDetails>>,

        unsaved_icon: RefCell<Option<Vec<u8>>>,

        #[template_child]
        pub icon_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub title_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub headerbar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub normal_headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub edit_headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub page_menu: TemplateChild<gio::Menu>,
        #[template_child]
        pub titlebar_color: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub user_agent_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub user_agent_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub domain_restriction_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub domains_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub proxy_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub proxy_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub autostart_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub background_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub permissions_group: TemplateChild<adw::PreferencesGroup>,

        // Dynamically built permission summary rows
        permission_rows: RefCell<Vec<gtk::Widget>>,

        // gsettings "apps-settings" change handler, disconnected on dispose
        settings_handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppPage {
        const NAME: &'static str = "AppPage";
        type Type = super::AppPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AppPage {
        fn constructed(&self) {
            self.parent_constructed();

            self.setup_signals();
        }

        fn dispose(&self) {
            if let Some(handler) = self.settings_handler.borrow_mut().take() {
                application::settings().disconnect(handler);
            }
        }
    }
    impl WidgetImpl for AppPage {}
    impl NavigationPageImpl for AppPage {}

    #[gtk::template_callbacks]
    impl AppPage {
        #[template_callback]
        fn on_cancel_clicked(&self, _: gtk::Button) {
            self.reset();
        }
        #[template_callback]
        fn update_unsaved_details_cb(&self, _: gtk::Widget) {
            self.update_unsaved_details();
        }
        #[template_callback]
        fn update_unsaved_details_notify_cb(&self, _: glib::ParamSpec) {
            self.update_unsaved_details();
        }
        #[template_callback]
        async fn on_save_clicked(&self, _: gtk::Button) {
            if let Err(err) = self.save_details().await {
                self.toast(err.to_string());
            }
        }
        #[template_callback]
        async fn on_icon_clicked(&self, _: gtk::Button) {
            if let Ok(file) =
                util::icon_from_dialog(self.obj().root().and_downcast_ref::<gtk::Window>()).await
            {
                if let Err(err) = self.set_unsaved_icon(&file).await {
                    self.toast(err.to_string())
                }
            }
        }
    }

    impl AppPage {
        fn toast(&self, message: String) {
            self.obj()
                .activate_action("win.notify", Some(&message.to_variant()))
                .unwrap();
        }
        fn setup_menu(&self) {
            self.page_menu.remove_all();

            let details = self.details.borrow();
            self.page_menu.append_item(&menu_item_and_target(
                "Open Window",
                "app.open-app",
                &details.id,
            ));
            self.page_menu
                .append_item(&menu_item_and_target("Delete", "win.delete", &details.id));
            self.page_menu.append_item(&menu_item_and_target(
                "Reinstall",
                "win.reinstall",
                &details.id,
            ));
        }
        fn diff_significance(&self) -> DiffSignificance {
            let unsaved = self.unsaved_details.borrow().clone().unwrap();
            let current = self.details.borrow();
            if current.eq(&unsaved) {
                DiffSignificance::NoDifference
            } else if unsaved.title != current.title || unsaved.icon != current.icon {
                DiffSignificance::DesktopReinstall
            } else {
                DiffSignificance::Settings
            }
        }
        fn update_unsaved_details(&self) {
            let details = self.details.borrow().clone();
            let icon = self.unsaved_icon.borrow();
            let icon = if icon.is_some() {
                icon.clone()
            } else {
                details.icon.clone()
            };

            let unsaved = AppDetails {
                url: self.url_entry.text().to_string(),
                title: self.title_entry.text().to_string(),
                has_titlebar_color: self.titlebar_color.is_active(),
                user_agent: self
                    .user_agent_expander
                    .enables_expansion()
                    .then(|| self.user_agent_entry.text().to_string()),
                allowed_domains: self
                    .domain_restriction_expander
                    .enables_expansion()
                    .then(|| {
                        self.domains_entry
                            .text()
                            .split(',')
                            .map(|d| d.trim().to_lowercase())
                            .filter(|d| !d.is_empty())
                            .collect::<Vec<String>>()
                    }),
                proxy_url: self
                    .proxy_expander
                    .enables_expansion()
                    .then(|| self.proxy_entry.text().to_string())
                    .filter(|x| !x.is_empty()),
                autostart: self.autostart_row.is_active(),
                run_in_background: self.background_row.is_active(),
                icon,
                ..details
            };
            self.unsaved_details.replace(Some(unsaved));
            let diff_sig = self.diff_significance();
            self.headerbar_stack.set_visible_child(
                &if diff_sig != DiffSignificance::NoDifference {
                    &self.edit_headerbar
                } else {
                    &self.normal_headerbar
                }
                .get(),
            );
        }
        async fn save_details(&self) -> anyhow::Result<()> {
            let wid = WindowIdentifier::from_native(&self.obj().root().unwrap())
                .await
                .ok_or(anyhow!("failed to get window"))?;
            let unsaved_details = self.unsaved_details.borrow().clone();
            if let Some(unsaved_details) = unsaved_details {
                match self.diff_significance() {
                    DiffSignificance::Settings => {
                        unsaved_details.save()?;
                    }
                    DiffSignificance::DesktopReinstall => {
                        apps::install_app(
                            &unsaved_details,
                            unsaved_details.icon.clone().unwrap(),
                            &wid,
                        )
                        .await?;
                    }
                    _ => (),
                }
                // Keep the XDG autostart entry in sync with the saved
                // setting (creating/removing it is idempotent)
                apps::set_app_autostart(&unsaved_details, unsaved_details.autostart)?;
                self.set_details(&unsaved_details);
            }
            self.update_unsaved_details();
            self.obj().activate_action("win.refresh", None)?;
            Ok(())
        }
        pub fn reset(&self) {
            let details = self.details.borrow().clone();
            self.set_details(&details);
            self.unsaved_icon.replace(None);
            self.update_unsaved_details();
        }
        pub fn set_details(&self, details: &AppDetails) {
            self.details.replace(details.clone());
            if details.icon.is_some() {
                let details = details.clone();
                let icon_image = self.icon_image.clone();
                glib::spawn_future_local(async move {
                    if let Ok(texture) = details.load_texture().await {
                        icon_image.set_paintable(Some(&texture));
                    }
                });
            } else {
                self.icon_image.set_paintable(gdk::Paintable::NONE);
            }
            self.title_entry.set_text(details.title.as_str());
            self.url_entry.set_text(details.url.as_str());
            self.titlebar_color.set_active(details.has_titlebar_color);
            self.user_agent_expander
                .set_enable_expansion(details.user_agent.is_some());
            if let Some(user_agent) = &details.user_agent {
                self.user_agent_entry.set_text(user_agent.as_str());
            }
            self.domain_restriction_expander
                .set_enable_expansion(details.allowed_domains.is_some());
            if let Some(domains) = &details.allowed_domains {
                self.domains_entry.set_text(domains.join(", ").as_str());
            }
            self.proxy_expander
                .set_enable_expansion(details.proxy_url.is_some());
            if let Some(proxy_url) = &details.proxy_url {
                self.proxy_entry.set_text(proxy_url.as_str());
            }
            self.autostart_row.set_active(details.autostart);
            self.background_row.set_active(details.run_in_background);

            self.setup_menu();
            self.setup_permissions();

            // Keep the permissions list live: decisions are saved by the
            // app's own window process, so rebuild whenever the underlying
            // settings change.
            if self.settings_handler.borrow().is_none() {
                let handler = application::settings().connect_changed(
                    Some("apps-settings"),
                    clone!(
                        #[weak(rename_to=_self)]
                        self,
                        move |_, _| {
                            _self.setup_permissions();
                        }
                    ),
                );
                *self.settings_handler.borrow_mut() = Some(handler);
            }
        }
        async fn set_unsaved_icon(&self, file: &gio::File) -> anyhow::Result<()> {
            let (buffer, _etag) = file.load_contents_future().await?;
            let extension = file.basename();
            let extension = extension
                .as_ref()
                .and_then(|x| x.extension())
                .and_then(|x| x.to_str());
            let image =
                util::Image::from_buffer(buffer.to_vec(), extension.is_some_and(|x| x == "svg"))
                    .await?;
            self.unsaved_icon.replace(Some(image.buffer.to_vec()));
            let texture = image.load_texture().await?;
            self.icon_image.set_paintable(Some(&texture));
            self.update_unsaved_details();
            Ok(())
        }
        fn setup_signals(&self) {
            self.titlebar_color.connect_active_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |_| {
                    _self.update_unsaved_details();
                }
            ));
            // Toggling a switch doesn't emit "activated", so listen for
            // the active property instead
            self.autostart_row.connect_active_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |_| {
                    _self.update_unsaved_details();
                }
            ));
            self.background_row.connect_active_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |_| {
                    _self.update_unsaved_details();
                }
            ));
            // The enable-switch of the expanders likewise needs
            // notify::enable-expansion to mark unsaved changes
            self.user_agent_expander
                .connect_enable_expansion_notify(clone!(
                    #[weak(rename_to=_self)]
                    self,
                    move |_| {
                        _self.update_unsaved_details();
                    }
                ));
            self.domain_restriction_expander
                .connect_enable_expansion_notify(clone!(
                    #[weak(rename_to=_self)]
                    self,
                    move |_| {
                        _self.update_unsaved_details();
                    }
                ));
            self.proxy_expander.connect_enable_expansion_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |_| {
                    _self.update_unsaved_details();
                }
            ));
        }
        /// Rebuilds the "Website Permissions" list: one row per origin
        /// that has saved decisions, each with a button to revoke them.
        fn setup_permissions(&self) {
            // Clear previous rows
            for row in self.permission_rows.borrow_mut().drain(..) {
                self.permissions_group.remove(&row);
            }

            let id = self.details.borrow().id.clone();
            let summaries = apps::get_permission_summaries(&id);

            let mut rows = self.permission_rows.borrow_mut();
            if summaries.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("No permissions requested");
                row.set_sensitive(false);
                self.permissions_group.add(&row);
                rows.push(row.upcast());
                return;
            }

            for (origin, kinds) in summaries {
                let row = adw::ActionRow::new();
                row.set_title(origin.as_str());
                row.set_subtitle(
                    &kinds
                        .iter()
                        .map(|kind| apps::permission_label(kind))
                        .collect::<Vec<_>>()
                        .join(", "),
                );

                let revoke_button = gtk::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .tooltip_text("Revoke all access")
                    .valign(gtk::Align::Center)
                    .css_classes(["flat", "destructive-action"])
                    .build();
                revoke_button.connect_clicked(clone!(
                    #[weak(rename_to=_self)]
                    self,
                    #[weak]
                    row,
                    move |_| {
                        let id = _self.details.borrow().id.clone();
                        let origin = row.title().to_string();
                        if let Err(err) = apps::clear_origin_permissions(&id, &origin) {
                            _self.toast(err.to_string());
                            return;
                        }
                        _self.toast(format!("Revoked access for {origin}"));
                        _self.setup_permissions();
                    }
                ));

                row.add_suffix(&revoke_button);
                self.permissions_group.add(&row);
                rows.push(row.upcast());
            }
        }
    }
}

glib::wrapper! {
    pub struct AppPage(ObjectSubclass<imp::AppPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AppPage {
    pub fn new(details: AppDetails) -> Self {
        let obj: Self = Object::builder().property("title", &details.title).build();
        let imp = obj.imp();
        imp.set_details(&details);
        obj
    }
}
