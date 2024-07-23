use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use url::Url;

use ashpd::WindowIdentifier;

use crate::{apps::install_app, util};

mod imp {

    use uuid::Uuid;

    use crate::apps::AppDetails;

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/xyz/zaedus/spider/create_app_dialog.ui")]
    pub struct CreateAppDialog {
        #[template_child]
        pub url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub button: TemplateChild<gtk::Button>,
        #[template_child]
        pub button_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub button_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
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

    impl ObjectImpl for CreateAppDialog {}
    impl WidgetImpl for CreateAppDialog {}
    impl AdwDialogImpl for CreateAppDialog {}

    #[gtk::template_callbacks]
    impl CreateAppDialog {
        #[template_callback]
        fn on_url_changed(&self, entry: adw::EntryRow) {
            self.button
                .set_sensitive(Url::parse(entry.text().as_str()).is_ok());
        }
        #[template_callback]
        async fn on_create_clicked(&self, _: gtk::Button) {
            self.button.set_sensitive(false);
            self.button_stack
                .set_visible_child(&self.button_spinner.get());
            self.button_spinner.set_spinning(true);

            if let Ok(url) = Url::parse(self.url_entry.text().as_str()) {
                if let Err(err) = self.create_app(url).await {
                    println!("{err:?}");
                    self.toast_overlay
                        .add_toast(adw::Toast::new(err.to_string().as_str()));
                }
            }

            self.button.set_sensitive(true);
            self.button_stack
                .set_visible_child(&self.button_label.get());
            self.button_spinner.set_spinning(false);
        }
        #[template_callback]
        async fn on_create_clicked_conditional(&self, _: adw::EntryRow) {
            if self.button.is_sensitive() {
                self.on_create_clicked(self.button.get()).await;
            }
        }
    }

    impl CreateAppDialog {
        async fn create_app(&self, url: Url) -> anyhow::Result<()> {
            let website_meta = util::get_website_meta(url.clone()).await?;
            install_app(
                &AppDetails::new(
                    Uuid::new_v4().to_string(),
                    website_meta.title,
                    url.to_string(),
                ),
                website_meta.icon.buffer,
                &WindowIdentifier::from_native(&self.obj().root().unwrap()).await,
            )
            .await?;
            self.obj().activate_action("win.refresh", None)?;
            self.obj().close();
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
