use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, Object};
use gtk::{gdk, glib};
use std::cell::RefCell;
use webkit::prelude::*;
use webkit::{PolicyDecisionType, WebView};

use crate::apps::AppDetails;

fn format_css(id: &str, bg: &str) -> String {
    let css = format!(
        r#"window#s{id} {{
    background: {bg};
}}"#
    );
    css
}

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/zaedus/spider/app_window.ui")]
    pub struct AppWindow {
        #[template_child]
        pub toolbar: TemplateChild<adw::ToolbarView>,

        pub details: RefCell<AppDetails>,
        pub webview: RefCell<webkit::WebView>,
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

    impl ObjectImpl for AppWindow {}
    impl WidgetImpl for AppWindow {}
    impl WindowImpl for AppWindow {}
    impl ApplicationWindowImpl for AppWindow {}
    impl AdwApplicationWindowImpl for AppWindow {}

    impl AppWindow {
        pub fn set_details(&self, details: &AppDetails) {
            self.details.replace(details.clone());

            // Configure window
            self.obj()
                .set_widget_name(format!("s{}", details.id).as_str());
            self.obj().set_title(Some(details.title.as_str()));

            // Set up the WebView
            let webview = self.create_webview();
            webview.load_uri(&details.url);
            self.toolbar.set_content(Some(&webview));
            self.webview.replace(webview);

            self.load_colors(None);
        }

        fn load_colors(&self, bg: Option<&str>) {
            if self.provider.borrow().is_none() {
                let display = gdk::Display::default().unwrap();
                let provider = gtk::CssProvider::new();

                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                self.provider.replace(Some(provider));
            }
            // Add new one if possible
            let provider = self.provider.borrow();
            let provider = provider.as_ref().unwrap();
            provider.load_from_string(
                format_css(
                    self.details.borrow().id.as_str(),
                    bg.unwrap_or("@window_bg_color"),
                )
                .as_str(),
            );
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
                .enable_developer_extras(true)
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

            // Build content manager
            let content_manager = webkit::UserContentManager::new();
            if details.has_titlebar_color {
                let script = webkit::UserScript::new(
                    include_str!("./inject.js"),
                    webkit::UserContentInjectedFrames::TopFrame,
                    webkit::UserScriptInjectionTime::End,
                    &[],
                    &[],
                );
                content_manager.register_script_message_handler("themeColor", None);
                content_manager.connect_script_message_received(
                    Some("themeColor"),
                    clone!(
                        #[weak(rename_to=_self)]
                        self,
                        move |_, value| {
                            let value = value.to_str();
                            if value != "null" {
                                _self.load_colors(Some(value.as_str()));
                            } else {
                                _self.load_colors(None)
                            }
                        }
                    ),
                );
                content_manager.add_script(&script);
            }

            // Build WebView
            let webview = WebView::builder()
                .network_session(&network_session)
                .settings(&settings)
                .user_content_manager(&content_manager)
                .build();

            webview.connect_decide_policy(|_, decision, decision_type| {
                if decision_type == PolicyDecisionType::NewWindowAction {
                    let mut action =
                        decision.property::<webkit::NavigationAction>("navigation-action");
                    if let Some(uri) = action.request().and_then(|a| a.uri()) {
                        open::that_detached(uri).unwrap();

                        decision.ignore();
                        return false;
                    }
                }
                true
            });

            webview
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
