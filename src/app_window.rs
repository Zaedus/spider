use adw::subclass::prelude::*;
use glib::Object;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use webkit::prelude::*;
use webkit::WebView;

use crate::apps::get_app_details;

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/xyz/zaedus/spider/app_window.ui")]
    #[properties(wrapper_type = super::AppWindow)]
    pub struct AppWindow {
        #[template_child]
        pub toolbar: TemplateChild<adw::ToolbarView>,

        #[property(get, set = Self::on_set_id)]
        pub id: RefCell<String>,
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

    #[glib::derived_properties]
    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }
    impl WidgetImpl for AppWindow {}
    impl WindowImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
    impl AdwApplicationWindowImpl for AppWindow {}

    impl AppWindow {
        fn on_set_id(&self, id: String) {
            self.id.replace(id.clone());
            let details = get_app_details(id.clone());

            self.obj().set_title(Some(details.title.as_str()));

            let webview = self.create_webview();
            webview.load_uri(&details.url);

            self.toolbar.set_content(Some(&webview));
        }
        fn create_webview(&self) -> webkit::WebView {
            let id = self.id.borrow();

            // Define and create base app directory where webkit cache, data, and cookies are stored
            let app_data_dir = glib::user_data_dir()
                .join(glib::application_name().unwrap())
                .join(id.as_str());
            let app_cache_dir = glib::user_cache_dir()
                .join(glib::application_name().unwrap())
                .join(id.as_str());
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
    pub fn new(id: String) -> Self {
        Object::builder().property("id", id).build()
    }
}
