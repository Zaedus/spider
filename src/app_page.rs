use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Object;
use gtk::{gio, glib};
use std::cell::RefCell;

use ashpd::WindowIdentifier;

use crate::apps::{self, AppDetails};
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
            let wid = WindowIdentifier::from_native(&self.obj().root().unwrap()).await;
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
            self.icon_image
                .set_paintable(Some(&details.to_gdk_texture(256)));
            self.title_entry.set_text(details.title.as_str());
            self.url_entry.set_text(details.url.as_str());
            self.titlebar_color.set_active(details.has_titlebar_color);

            self.setup_menu();
        }
        async fn set_unsaved_icon(&self, file: &gio::File) -> anyhow::Result<()> {
            let (buffer, _etag) = file.load_contents_future().await?;
            let extension = file.basename();
            let extension = extension
                .as_ref()
                .and_then(|x| x.extension())
                .and_then(|x| x.to_str());
            let image =
                util::Image::from_buffer(buffer.to_vec(), extension.is_some_and(|x| x == "svg"))?;
            self.unsaved_icon.replace(Some(image.buffer.to_vec()));
            self.icon_image
                .set_paintable(Some(&image.to_gdk_texture(32)));
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
