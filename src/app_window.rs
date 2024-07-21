use adw::subclass::prelude::*;
use glib::Object;
use gtk::glib;
use gtk::prelude::*;
use webkit::prelude::*;
use webkit::WebView;
use std::cell::RefCell;

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
        pub id: RefCell<String>
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

            let webview = WebView::new();

            let settings = webkit::Settings::new();
            settings.set_enable_webgl(true);
            settings.set_enable_webrtc(true);
            settings.set_enable_media_capabilities(true);
            settings.set_enable_encrypted_media(true);
            webview.set_settings(&settings);

            let session = webview.network_session().unwrap();

            let cookie_mgr = session.cookie_manager().unwrap();
            let mut cookie_file = glib::user_data_dir();
            cookie_file.push(glib::application_name().unwrap());
            cookie_file.push(id.as_str());
            std::fs::create_dir_all(cookie_file.clone()).unwrap();
            cookie_file.push("cookies");
            cookie_mgr.set_persistent_storage(cookie_file.to_str().unwrap(), webkit::CookiePersistentStorage::Sqlite);

            webview.load_uri(&details.url);
            self.toolbar.set_content(Some(&webview));
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
        Object::builder()
            .property("id", id)
            .build()
    }
}
