use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use webkit::prelude::*;
use webkit::soup;
use webkit::{HardwareAccelerationPolicy, PolicyDecisionType, WebView};

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
        #[template_child]
        pub webview_container: TemplateChild<adw::Bin>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub forward_button: TemplateChild<gtk::Button>,

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
            klass.bind_template_callbacks();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AppWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_gestures();
            self.obj().setup_gactions();
        }
    }
    impl WidgetImpl for AppWindow {}
    impl WindowImpl for AppWindow {
        fn close_request(&self) -> glib::Propagation {
            let size = self.obj().default_size();
            let mut details = self.details.borrow().clone();
            details.window_width = size.0;
            details.window_height = size.1;
            details.window_maximize = self.obj().is_maximized();
            details.save().unwrap(); // App is closing, shouldn't fail really ever
            glib::Propagation::Proceed
        }
    }
    impl ApplicationWindowImpl for AppWindow {}
    impl AdwApplicationWindowImpl for AppWindow {}

    impl AppWindow {
        pub fn set_details(&self, details: &AppDetails) {
            self.details.replace(details.clone());

            // Configure window
            self.obj()
                .set_widget_name(format!("s{}", details.id).as_str());
            self.obj().set_title(Some(details.title.as_str()));
            self.obj().load_window_size();

            // Set up the WebView
            let webview = self.create_webview();
            webview.load_uri(&details.url);
            self.webview_container.set_child(Some(&webview));
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
                .enable_webaudio(true)
                .enable_media(true)
                .enable_mediasource(true)
                .enable_encrypted_media(true)
                .enable_media_capabilities(true)
                .hardware_acceleration_policy(HardwareAccelerationPolicy::Always)
                .enable_2d_canvas_acceleration(true)
                .enable_html5_local_storage(true)
                .enable_html5_database(true)
                .enable_hyperlink_auditing(true)
                .enable_site_specific_quirks(true)
                .enable_developer_extras(true)
                .build();

            // Build network session
            let network_session = webkit::NetworkSession::builder()
                .cache_directory(app_cache_dir.to_str().unwrap())
                .data_directory(app_data_dir.join("data").to_str().unwrap())
                .build();

            network_session.connect_download_started(|_, dl| {
                dl.connect_decide_destination(move |dl, dest| {
                    let dest = dest.to_string();
                    glib::spawn_future_local(clone!(
                        #[weak]
                        dl,
                        async move {
                            let dialog = gtk::FileDialog::builder()
                                .accept_label("Save")
                                .title("Download file")
                                .modal(false)
                                .initial_name(dest.as_str())
                                .build();
                            if let Some(path) = dialog
                                .save_future(None::<&gtk::Window>)
                                .await
                                .ok()
                                .and_then(|f| f.path())
                            {
                                let path = path.as_os_str().to_str().unwrap();
                                dl.set_destination(path);
                            }
                        }
                    ));
                    true
                });
            });

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

            webview.connect_decide_policy(|webview, decision, decision_type| {
                if decision_type == PolicyDecisionType::NewWindowAction {
                    let mut action =
                        decision.property::<webkit::NavigationAction>("navigation-action");
                    if let Some(uri) = action.request().and_then(|a| a.uri()) {
                        open::that_detached(uri).unwrap();

                        decision.ignore();
                        return false;
                    }
                }
                if decision_type == PolicyDecisionType::Response {
                    let response = decision.property::<webkit::URIResponse>("response");
                    let headers = response.property::<soup::MessageHeaders>("http-headers");
                    if headers.one("Content-Type").and_then(|x| {
                        if webview.can_show_mime_type(x.as_str()) {
                            Some(())
                        } else {
                            None
                        }
                    }).is_none() {
                        decision.download();
                        return true;
                    }
                }
                true
            });

            webview.connect_estimated_load_progress_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |webview: &WebView| {
                    let progress = webview.estimated_load_progress();
                    _self
                        .progress_bar
                        .set_fraction(if progress == 1.0 { 0.0 } else { progress });
                }
            ));

            webview.connect_uri_notify(clone!(
                #[weak(rename_to=_self)]
                self,
                move |webview: &WebView| {
                    _self.update_nav_buttons(webview);
                }
            ));

            webview
        }

        pub fn go_back(&self) {
            let webview = self.webview.borrow();
            webview.go_back();
            self.update_nav_buttons(&webview);
        }
        pub fn go_forward(&self) {
            let webview = self.webview.borrow();
            webview.go_forward();
            self.update_nav_buttons(&webview);
        }

        fn update_nav_buttons(&self, webview: &WebView) {
            // The back_forward_list hasn't updated by this point
            // We correct by comparing the current uri with where it was in the list
            // This is probably rife with edge cases lol
            let list = webview.back_forward_list().unwrap();

            if list.back_item().and_then(|x| x.uri()) == webview.uri() {
                self.back_button.set_sensitive(list.back_list().len() != 1);
                self.forward_button
                    .set_sensitive(list.forward_list().is_empty());
            } else if list.forward_item().and_then(|x| x.uri()) == webview.uri() {
                self.back_button.set_sensitive(list.back_list().is_empty());
                self.forward_button
                    .set_sensitive(list.forward_list().len() != 1);
            } else {
                self.forward_button.set_sensitive(webview.can_go_forward());
                self.back_button.set_sensitive(webview.can_go_back());
            }
        }
    }

    #[gtk::template_callbacks]
    impl AppWindow {
        #[template_callback]
        fn on_back_clicked(&self, _: gtk::Button) {
            self.go_back()
        }
        #[template_callback]
        fn on_forward_clicked(&self, _: gtk::Button) {
            self.go_forward();
        }
    }
}

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<imp::AppWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget, gio::ActionMap, gio::ActionGroup, gtk::Native, gtk::ShortcutManager;
}

impl AppWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P, details: &AppDetails) -> Self {
        let obj: Self = glib::Object::builder()
            .property("application", application)
            .build();
        obj.imp().set_details(details);
        obj
    }

    pub fn id(&self) -> String {
        self.imp().details.borrow().id.clone()
    }

    fn setup_gactions(&self) {
        self.add_action_entries([
            gio::ActionEntry::builder("forward")
                .activate(move |win: &Self, _, _| win.imp().go_forward())
                .build(),
            gio::ActionEntry::builder("back")
                .activate(move |win: &Self, _, _| win.imp().go_back())
                .build(),
        ]);
    }
    fn setup_gestures(&self) {
        let gesture = gtk::GestureClick::new();
        gesture.set_button(0);

        // Prevents children (the webview) from seeing the Claimed events
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Handle back (8) and forward (9) mouse button events
        gesture.connect_pressed(clone!(
            #[weak(rename_to=_self)]
            self,
            move |gesture, _, _, _| {
                if gesture.current_button() == 8 {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    _self.imp().go_back();
                }
                if gesture.current_button() == 9 {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    _self.imp().go_forward();
                }
            }
        ));

        self.add_controller(gesture);
    }
    fn load_window_size(&self) {
        let details = self.imp().details.borrow();
        self.set_default_size(details.window_width, details.window_height);

        if details.window_maximize {
            self.maximize();
        }
    }
}
