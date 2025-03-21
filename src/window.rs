/* window.rs
 *
 * Copyright 2024 Unknown
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::anyhow;
use ashpd::WindowIdentifier;
use glib::clone;
use gtk::{gio, glib};

use crate::app_page::AppPage;
use crate::app_row::AppRow;
use crate::application::settings;
use crate::apps::{copy_app_dir, get_app_details, get_app_icon, install_app, uninstall_app};
use crate::create_app_dialog::{gen_unique_id, CreateAppDialog, APP_ID_LENGTH};
use crate::home_page::HomePage;

mod imp {

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/zaedus/spider/window.ui")]
    pub struct SpiderWindow {
        // Template widgets
        #[template_child]
        pub split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub apps_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub home_page: TemplateChild<HomePage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpiderWindow {
        const NAME: &'static str = "SpiderWindow";
        type Type = super::SpiderWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SpiderWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.setup_gactions();
            obj.refresh();
            obj.load_window_size();
            self.apps_listbox.unselect_all();
        }
    }
    impl WidgetImpl for SpiderWindow {}
    impl WindowImpl for SpiderWindow {
        fn close_request(&self) -> glib::Propagation {
            let settings = settings();
            let size = self.obj().default_size();
            settings
                .set_int("window-width", size.0)
                .expect("Failed to save window size");
            settings
                .set_int("window-height", size.1)
                .expect("Failed to save window size");
            glib::Propagation::Proceed
        }
    }
    impl ApplicationWindowImpl for SpiderWindow {}
    impl AdwApplicationWindowImpl for SpiderWindow {}

    #[gtk::template_callbacks]
    impl SpiderWindow {
        #[template_callback]
        fn on_add_clicked(&self, _: gtk::Button) {
            let dialog = CreateAppDialog::new();
            dialog.present(Some(&self.obj().clone()));
        }
        #[template_callback]
        fn on_app_selected(&self, row: Option<AppRow>) {
            if let Some(row) = row {
                if let Some(details) = row.imp().details.get() {
                    let page = AppPage::new(details.clone());
                    self.split_view.set_content(Some(&page));
                    self.split_view.set_show_content(true);
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct SpiderWindow(ObjectSubclass<imp::SpiderWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl SpiderWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }
    fn setup_gactions(&self) {
        self.add_action_entries([
            gio::ActionEntry::builder("refresh")
                .activate(move |win: &Self, _, _| win.refresh())
                .build(),
            gio::ActionEntry::builder("delete")
                .parameter_type(Some(&String::static_variant_type()))
                .activate(move |win: &Self, _, id| {
                    win.confirm_delete_app(
                        id.expect("no id provided")
                            .get::<String>()
                            .expect("invalid id type provided"),
                    );
                })
                .build(),
            gio::ActionEntry::builder("reinstall")
                .parameter_type(Some(&String::static_variant_type()))
                .activate(move |win: &Self, _, id| {
                    win.confirm_reinstall_app(
                        id.expect("no id provided")
                            .get::<String>()
                            .expect("invalid id type provided"),
                    );
                })
                .build(),
            gio::ActionEntry::builder("notify")
                .parameter_type(Some(&String::static_variant_type()))
                .activate(move |win: &Self, _, msg| {
                    win.toast(
                        msg.expect("no message provided")
                            .get::<String>()
                            .expect("invalid message type provided")
                            .as_str(),
                    )
                })
                .build(),
        ]);
    }
    fn selected_page_id(&self) -> Option<String> {
        self.imp()
            .apps_listbox
            .selected_row()
            .and_downcast::<AppRow>()
            .map(|x| x.id())
    }

    async fn reinstall_app(&self, id: String) -> anyhow::Result<()> {
        // Generate with the new id format
        let mut details = get_app_details(&id).ok_or(anyhow!("failed to get app details"))?;
        let icon = get_app_icon(&id).await?;
        let id = if id.len() != APP_ID_LENGTH {
            let new_id = gen_unique_id();
            copy_app_dir(id.as_str(), new_id.as_str())?;
            uninstall_app(id.as_str()).await?;
            new_id
        } else {
            id
        };

        details.id = id;

        install_app(
            &details,
            icon,
            &WindowIdentifier::from_native(&self.root().unwrap()).await,
        )
        .await?;

        self.refresh();

        Ok(())
    }

    async fn delete_app(&self, id: String) -> anyhow::Result<()> {
        uninstall_app(id.as_str()).await?;
        self.refresh();
        Ok(())
    }

    fn confirm_reinstall_app(&self, id: String) {
        let confirm_dialog = adw::AlertDialog::new(
            Some("Are you sure you want to reinstall this app?"),
            Some("This can fix faulty installations."),
        );
        confirm_dialog.add_responses(&[("reinstall", "Reinstall"), ("cancel", "Cancel")]);
        confirm_dialog.set_response_appearance("reinstall", adw::ResponseAppearance::Destructive);
        confirm_dialog.present(Some(self));
        confirm_dialog.connect_response(
            Some("reinstall"),
            clone!(
                #[strong(rename_to=_self)]
                self,
                #[strong]
                id,
                move |_, _| {
                    glib::spawn_future_local(clone!(
                        #[strong]
                        _self,
                        #[strong]
                        id,
                        async move {
                            let message = match _self.reinstall_app(id).await {
                                Ok(_) => "Successfully reinstalled app!".to_string(),
                                Err(err) => err.to_string(),
                            };
                            _self.toast(message.as_str());
                        }
                    ));
                }
            ),
        );
    }

    fn confirm_delete_app(&self, id: String) {
        let confirm_dialog = adw::AlertDialog::new(
            Some("Are you sure you want to delete this app?"),
            Some("This action CANNOT be undone."),
        );
        confirm_dialog.add_responses(&[("delete", "Delete"), ("cancel", "Cancel")]);
        confirm_dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        confirm_dialog.present(Some(self));
        confirm_dialog.connect_response(
            Some("delete"),
            clone!(
                #[strong(rename_to=_self)]
                self,
                #[strong]
                id,
                move |_, _| {
                    glib::spawn_future_local(clone!(
                        #[strong]
                        _self,
                        #[strong]
                        id,
                        async move {
                            let message = match _self.delete_app(id).await {
                                Ok(_) => "Successfully deleted app!".to_string(),
                                Err(err) => err.to_string(),
                            };
                            _self.toast(message.as_str());
                        }
                    ));
                }
            ),
        );
    }

    fn refresh(&self) {
        let imp = self.imp();
        let selected_id = self.selected_page_id();
        imp.apps_listbox.remove_all();

        let settings = settings();
        for id in settings.get::<Vec<String>>("app-ids") {
            let row = AppRow::new(id);
            imp.apps_listbox.append(&row);
            if let Some(selected_id) = selected_id.as_deref() {
                if selected_id == row.id() {
                    imp.apps_listbox.select_row(Some(&row));
                }
            }
        }
        if self.selected_page_id().is_none() {
            self.imp()
                .split_view
                .set_content(Some(&HomePage::default()));
        }
    }
    fn toast(&self, message: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(message));
    }
    fn load_window_size(&self) {
        let settings = settings();
        self.set_default_size(settings.int("window-width"), settings.int("window-height"));
    }
}
