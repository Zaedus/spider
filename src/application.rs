use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::app_window::AppWindow;
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
        // We connect to the activate callback to create a window when the application
        // has been launched. Additionally, this callback notifies us when the user
        // tries to launch a "second instance" of the application. When they try
        // to do that, we'll just present any existing window.
        fn activate(&self) {
            let application = self.obj();
            // Get the current window or create one if necessary
            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = SpiderWindow::new(&*application);
                window.upcast()
            };

            // Ask the window manager/compositor to present the window
            window.present();
        }

        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
            if let Some(id) = command_line.arguments().get(1) {
                let id: String = id.to_str().unwrap().into();
                let window = AppWindow::new(id);
                self.obj().add_window(&window);
                window.present();
                glib::ExitCode::SUCCESS
            } else {
                self.activate();
                glib::ExitCode::SUCCESS
            }
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
                    app.open_app(
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
            .application_name("spider")
            .application_icon(config::APP_ID)
            .developer_name("Zaedus")
            .version(config::VERSION)
            .developers(vec!["Zaedus"])
            .copyright("© 2024 Zaedus")
            .build();

        about.present();
    }

    fn open_app(&self, id: String) {
        for window in self.windows() {
            if let Ok(window) = window.downcast::<AppWindow>() {
                if window.id() == id {
                    window.present();
                    return;
                } 
            }
        }
        let window = AppWindow::new(id);
        self.add_window(&window);
        window.present();
    }
}
