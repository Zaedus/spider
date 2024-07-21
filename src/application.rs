use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gio, glib};


use crate::config::{self, VERSION};
use crate::SpiderWindow;

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
        ]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();
        let about = adw::AboutWindow::builder()
            .transient_for(&window)
            .application_name("spider")
            .application_icon(config::APP_ID)
            .developer_name("Zaedus")
            .version(VERSION)
            .developers(vec!["Zaedus"])
            .copyright("© 2024 Zaedus")
            .build();

        about.present();
    }
}
