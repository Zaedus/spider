use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Object;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;

use ashpd::WindowIdentifier;

use crate::apps::{self, AppDetails};
use crate::util;

#[inline]
fn solid_color(r: u8, g: u8, b: u8) -> gdk::RGBA {
    gdk::RGBA::new(
        (r as f32) / 255.0,
        (g as f32) / 255.0,
        (b as f32) / 255.0,
        1.0,
    )
}

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
    #[template(resource = "/xyz/zaedus/spider/app_page.ui")]
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
        pub lbg_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub lfg_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub dbg_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub dfg_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub light_colors: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub dark_colors: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub headerbar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub normal_headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub edit_headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub page_menu: TemplateChild<gio::Menu>,
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
            if let Ok(file) = self.get_new_icon().await {
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
            let light_fg = if self.light_colors.enables_expansion() {
                Some(self.lfg_color.rgba().to_string())
            } else {
                None
            };
            let light_bg = if self.light_colors.enables_expansion() {
                Some(self.lbg_color.rgba().to_string())
            } else {
                None
            };
            let dark_fg = if self.dark_colors.enables_expansion() {
                Some(self.dfg_color.rgba().to_string())
            } else {
                None
            };
            let dark_bg = if self.dark_colors.enables_expansion() {
                Some(self.dbg_color.rgba().to_string())
            } else {
                None
            };
            let icon = self.unsaved_icon.borrow();
            let icon = if icon.is_some() {
                icon.clone()
            } else {
                details.icon.clone()
            };

            let unsaved = AppDetails {
                url: self.url_entry.text().to_string(),
                title: self.title_entry.text().to_string(),
                light_fg,
                light_bg,
                dark_fg,
                dark_bg,
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
                        apps::save_app_details(&unsaved_details)?;
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

            self.light_colors
                .set_enable_expansion(details.light_bg.is_some() || details.light_fg.is_some());
            self.dark_colors
                .set_enable_expansion(details.dark_bg.is_some() || details.dark_fg.is_some());

            self.dbg_color.set_rgba(
                &details
                    .dark_bg
                    .as_ref()
                    .and_then(|x| gdk::RGBA::parse(x).ok())
                    .unwrap_or(solid_color(36, 36, 36)),
            );
            self.dfg_color.set_rgba(
                &details
                    .dark_fg
                    .as_ref()
                    .and_then(|x| gdk::RGBA::parse(x).ok())
                    .unwrap_or(solid_color(255, 255, 255)),
            );
            self.lbg_color.set_rgba(
                &details
                    .light_bg
                    .as_ref()
                    .and_then(|x| gdk::RGBA::parse(x).ok())
                    .unwrap_or(solid_color(250, 250, 250)),
            );
            self.lfg_color.set_rgba(
                &details
                    .light_fg
                    .as_ref()
                    .and_then(|x| gdk::RGBA::parse(x).ok())
                    .unwrap_or(solid_color(50, 50, 50)),
            );
            self.setup_menu();
        }
        fn setup_signals(&self) {
            fn update_details<T>(_self: &AppPage) -> impl Fn(&T, &glib::ParamSpec) {
                clone!(
                    #[weak(rename_to=_self)]
                    _self,
                    move |_: &T, _: &glib::ParamSpec| {
                        _self.update_unsaved_details();
                    }
                )
            }
            self.light_colors
                .connect_notify_local(Some("enable-expansion"), update_details(self));
            self.dark_colors
                .connect_notify_local(Some("enable-expansion"), update_details(self));
            self.lbg_color
                .connect_notify_local(Some("rgba"), update_details(self));
            self.lfg_color
                .connect_notify_local(Some("rgba"), update_details(self));
            self.dbg_color
                .connect_notify_local(Some("rgba"), update_details(self));
            self.dfg_color
                .connect_notify_local(Some("rgba"), update_details(self));
        }
        async fn get_new_icon(&self) -> anyhow::Result<gio::File> {
            let filter = gtk::FileFilter::new();
            filter.add_pixbuf_formats();

            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);

            let root = self.obj().root();
            let file = gtk::FileDialog::builder()
                .accept_label("Select")
                .modal(true)
                .title("App Icon")
                .filters(&filters)
                .build()
                .open_future(root.and_downcast_ref::<gtk::Window>())
                .await?;

            Ok(file)
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
