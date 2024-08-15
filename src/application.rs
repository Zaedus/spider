use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::anyhow;
use gtk::{gio, glib};

use crate::app_window::AppWindow;
use crate::apps::clean_app_dirs;
use crate::apps::get_app_details;
use crate::config;
use crate::SpiderWindow;

pub fn settings() -> gio::Settings {
    gio::Settings::new(config::APP_ID)
}

mod imp {

    use super::*;

    #[derive(Debug, Default)]
    pub struct SpiderApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for SpiderApplication {
        const NAME: &'static str = "SpiderApplication";
        type Type = super::SpiderApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for SpiderApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
        }
    }

    impl ApplicationImpl for SpiderApplication {
        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
            let application = self.obj();

            // Clean up
            clean_app_dirs().unwrap();

            // Get or create window to present
            let window = if let Some(id) = command_line.arguments().get(1) {
                self.obj()
                    .create_app(id.to_string_lossy().to_string())
                    .unwrap()
                    .upcast()
            } else if let Some(window) = application.active_window() {
                window
            } else {
                SpiderWindow::new(&*application).upcast()
            };
            window.present();

            glib::ExitCode::SUCCESS
        }
    }

    impl GtkApplicationImpl for SpiderApplication {}
    impl AdwApplicationImpl for SpiderApplication {}
}

glib::wrapper! {
    pub struct SpiderApplication(ObjectSubclass<imp::SpiderApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl SpiderApplication {
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    fn setup_gactions(&self) {
        self.add_action_entries([
            gio::ActionEntry::builder("quit")
                .activate(move |app: &Self, _, _| app.quit())
                .build(),
            gio::ActionEntry::builder("about")
                .activate(move |app: &Self, _, _| app.show_about())
                .build(),
            gio::ActionEntry::builder("open-app")
                .parameter_type(Some(&String::static_variant_type()))
                .activate(move |app: &Self, _, id| {
                    app.create_app(
                        id.expect("no id provided")
                            .get::<String>()
                            .expect("invalid id type provided"),
                    )
                    .unwrap()
                    .present()
                })
                .build(),
            gio::ActionEntry::builder("close-app")
                .parameter_type(Some(&String::static_variant_type()))
                .activate(move |app: &Self, _, id| {
                    app.close_app(
                        id.expect("no id provided")
                            .get::<String>()
                            .expect("invalid id type provided"),
                    )
                })
                .build(),
        ]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();
        let about = adw::AboutWindow::builder()
            .transient_for(&window)
            .application_name("Spider")
            .application_icon(config::APP_ID)
            .developer_name("Zaedus")
            .version(config::VERSION)
            .developers(vec!["Zaedus"])
            .copyright("© 2024 Zaedus")
            .build();

        about.present();
    }

    fn find_app(&self, id: &str) -> Option<AppWindow> {
        for window in self.windows() {
            if let Ok(window) = window.downcast::<AppWindow>() {
                if window.id() == id {
                    return Some(window);
                }
            }
        }
        None
    }

    fn close_app(&self, id: String) {
        if let Some(window) = self.find_app(&id) {
            window.set_hide_on_close(false);
            window.close();
        }
    }

    fn create_app(&self, id: String) -> anyhow::Result<AppWindow> {
        if let Some(window) = self.find_app(&id) {
            Ok(window)
        } else {
            let details = get_app_details(id.as_str()).ok_or(anyhow!("No app with id {}", id))?;
            let window = AppWindow::new(&details);
            self.add_window(&window);
            Ok(window)
        }
    }
}
