use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, Object};
use gtk::glib;
use std::cell::RefCell;
use webkit::prelude::*;
use webkit::WebView;

use crate::apps::AppDetails;

fn format_css(fg: &str, bg: &str) -> String {
    let css = format!(
        r#"
window {{
    color: {};
    background: {};
}}
"#,
        fg, bg
    );
    css
}

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/xyz/zaedus/spider/app_window.ui")]
    pub struct AppWindow {
        #[template_child]
        pub toolbar: TemplateChild<adw::ToolbarView>,

        pub details: RefCell<AppDetails>,
        pub provider: RefCell<Option<gtk::CssProvider>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppWindow {
        const NAME: &'static str = "AppWindow";
        type Type = super::AppWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = gtk::Settings::default().unwrap();
            settings.connect_gtk_application_prefer_dark_theme_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |_| _self.load_style()
            ));
        }
    }
    impl WidgetImpl for AppWindow {}
    impl WindowImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
    impl AdwApplicationWindowImpl for AppWindow {}

    impl AppWindow {
        pub fn set_details(&self, details: &AppDetails) {
            self.details.replace(details.clone());

            // Configure window
            self.load_style();
            self.obj().set_title(Some(details.title.as_str()));

            // Set up the WebView
            let webview = self.create_webview();
            webview.load_uri(&details.url);
            self.toolbar.set_content(Some(&webview));
        }
        fn try_load_colors(&self, fg: &Option<String>, bg: &Option<String>) {
            #[allow(deprecated)]
            let style_context = self.obj().style_context();

            // Remove old provider
            if let Some(provider) = self.provider.borrow().as_ref() {
                #[allow(deprecated)]
                style_context.remove_provider(provider);
            }

            // Add new one if possible
            if let Some(ref fg) = fg {
                if let Some(ref bg) = bg {
                    let provider = gtk::CssProvider::new();
                    provider.load_from_string(format_css(fg, bg).as_str());
                    #[allow(deprecated)]
                    style_context.add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                    self.provider.replace(Some(provider));
                }
            }
        }
        fn load_style(&self) {
            let details = self.details.borrow();
            let settings = gtk::Settings::default().unwrap();
            if settings.is_gtk_application_prefer_dark_theme() {
                self.try_load_colors(&details.dark_fg, &details.dark_bg);
            } else {
                self.try_load_colors(&details.light_fg, &details.light_bg);
            }
        }
        fn create_webview(&self) -> webkit::WebView {
            let details = self.details.borrow();
            let id = details.id.as_str();

            // Define and create base app directory where webkit cache, data, and cookies are stored
            let app_data_dir = glib::user_data_dir()
                .join(glib::application_name().unwrap())
                .join(id);
            let app_cache_dir = glib::user_cache_dir()
                .join(glib::application_name().unwrap())
                .join(id);
            std::fs::create_dir_all(app_data_dir.clone()).unwrap();
            std::fs::create_dir_all(app_cache_dir.clone()).unwrap();

            // Build settings
            let settings = webkit::Settings::builder()
                .enable_webgl(true)
                .enable_webrtc(true)
                .enable_encrypted_media(true)
                .enable_media_capabilities(true)
                .build();
            // Build network session
            let network_session = webkit::NetworkSession::builder()
                .cache_directory(app_cache_dir.to_str().unwrap())
                .data_directory(app_data_dir.join("data").to_str().unwrap())
                .build();

            // Build cookie manager
            let cookie_manager = network_session.cookie_manager().unwrap();

            cookie_manager.set_persistent_storage(
                app_data_dir.join("cookie").to_str().unwrap(),
                webkit::CookiePersistentStorage::Sqlite,
            );

            // Build WebView
            WebView::builder()
                .network_session(&network_session)
                .settings(&settings)
                .build()
        }
    }
}

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<imp::AppWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AppWindow {
    pub fn new(details: &AppDetails) -> Self {
        let obj: Self = Object::builder().build();
        obj.imp().set_details(details);
        obj
    }
    pub fn id(&self) -> String {
        self.imp().details.borrow().id.clone()
    }
}
