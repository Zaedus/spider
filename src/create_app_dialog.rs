use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use rand::Rng;
use std::cell::{Cell, RefCell};

use url::Url;

use ashpd::WindowIdentifier;

use crate::{
    application::settings,
    apps::{install_app, AppDetails},
    util,
};

use anyhow::bail;

pub const APP_ID_LENGTH: usize = 10;

fn gen_id() -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Uniform::from('a'..='z'))
        .take(APP_ID_LENGTH)
        .collect()
}

pub fn gen_unique_id() -> String {
    // Gen unique ID
    let app_ids = gio::prelude::SettingsExtManual::get::<Vec<String>>(&settings(), "app-ids");
    let mut id = gen_id();
    while app_ids.contains(&id) {
        id = gen_id();
    }
    id
}

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/zaedus/spider/create_app_dialog.ui")]
    #[properties(wrapper_type = super::CreateAppDialog)]
    pub struct CreateAppDialog {
        unsaved_icon: RefCell<Option<Vec<u8>>>,

        #[template_child]
        pub url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub button: TemplateChild<gtk::Button>,
        #[template_child]
        pub button_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub button_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub icon_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub title_entry: TemplateChild<adw::EntryRow>,

        #[property(get, set)]
        pub loading: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CreateAppDialog {
        const NAME: &'static str = "CreateAppDialog";
        type Type = super::CreateAppDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for CreateAppDialog {}
    impl WidgetImpl for CreateAppDialog {}
    impl AdwDialogImpl for CreateAppDialog {}

    #[gtk::template_callbacks]
    impl CreateAppDialog {
        #[template_callback]
        async fn on_url_apply(&self, entry: adw::EntryRow) {
            self.obj().set_loading(true);
            if let Ok(url) = self.parse_url(entry.text().as_str()) {
                // Disable apply button when setting text
                // so it won't keep coming back on every apply
                self.url_entry.set_show_apply_button(false);
                self.url_entry.set_text(url.as_str());
                self.url_entry.set_show_apply_button(true);

                match util::get_website_meta(url).await {
                    Ok(meta) => {
                        self.title_entry
                            .set_text(meta.title.unwrap_or_default().as_str());
                        self.unsaved_icon
                            .replace(meta.icon.as_ref().map(|x| x.buffer.clone()));
                        self.icon_image
                            .set_paintable(meta.icon.map(|x| x.to_gdk_texture(32)).as_ref());
                    }
                    Err(err) => self.toast(err.to_string()),
                }
                self.validate_input();
            } else {
                self.validate_input();
                self.url_entry.set_css_classes(&["error"]);
            }
            self.obj().set_loading(false);
        }
        #[template_callback]
        fn validate_input_cb(&self, _: gtk::Widget) {
            self.validate_input();
        }
        #[template_callback]
        async fn on_icon_clicked(&self, _: gtk::Button) {
            if let Ok(file) =
                util::icon_from_dialog(self.obj().root().and_downcast_ref::<gtk::Window>()).await
            {
                if let Err(err) = self.set_unsaved_icon(&file).await {
                    self.toast(err.to_string())
                } else {
                    self.validate_input();
                }
            }
        }
        #[template_callback]
        async fn on_create_clicked(&self, _: gtk::Button) {
            if self.validate_input() {
                self.button.set_sensitive(false);
                self.button_stack
                    .set_visible_child(&self.button_spinner.get());

                if let Err(err) = install_app(
                    &AppDetails::new(
                        gen_unique_id(),
                        self.title_entry.text().to_string(),
                        self.url_entry.text().to_string(),
                    ),
                    self.unsaved_icon.take().unwrap(),
                    &WindowIdentifier::from_native(&self.obj().root().unwrap()).await,
                )
                .await
                {
                    self.toast(err.to_string());
                } else {
                    self.obj().activate_action("win.refresh", None).unwrap();
                    self.obj().close();
                }
                self.button.set_sensitive(true);
                self.button_stack
                    .set_visible_child(&self.button_label.get());
            }
        }
    }

    impl CreateAppDialog {
        fn parse_url(&self, url: &str) -> anyhow::Result<Url> {
            if let Ok(url) = Url::parse(url) {
                Ok(url)
            } else {
                let url = format!("https://{url}");
                if let Ok(url) = Url::parse(url.as_str()) {
                    Ok(url)
                } else {
                    bail!("");
                }
            }
        }
        fn validate_input(&self) -> bool {
            let valid = Url::parse(self.url_entry.text().as_str()).is_ok()
                && self.unsaved_icon.borrow().is_some()
                && !self.title_entry.text().is_empty();
            self.url_entry.set_css_classes(&[]);
            self.button.set_sensitive(valid);
            valid
        }

        fn toast(&self, message: String) {
            self.toast_overlay
                .add_toast(adw::Toast::new(message.as_str()));
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
            Ok(())
        }
    }
}

glib::wrapper! {
    pub struct CreateAppDialog(ObjectSubclass<imp::CreateAppDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl CreateAppDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}

impl Default for CreateAppDialog {
    fn default() -> Self {
        Self::new()
    }
}
