use std::process::Command;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::anyhow;
use gtk::{gio, glib};

use crate::app_window::AppWindow;
use crate::apps::clean_app_dirs;
use crate::apps::get_app_details;
use crate::apps::AppsSettings;
use crate::config;
use crate::config::APP_ID;
use crate::SpiderWindow;
use glib::{OptionArg, OptionFlags};

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
            obj.add_main_option(
                "list-applications",
                glib::Char::from(b'l'),
                OptionFlags::NONE,
                OptionArg::None,
                "lists the known applications",
                None,
            );
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
            obj.set_accels_for_action("win.back", &["<alt>Left", "Back"]);
            obj.set_accels_for_action("win.forward", &["<alt>Right", "Forward"]);
        }
    }

    impl ApplicationImpl for SpiderApplication {
        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
            let application = self.obj();

            // Clean up
            clean_app_dirs().unwrap();

            // If listing application via the command line
            if command_line
                .options_dict()
                .lookup::<bool>("list-applications")
                .unwrap_or(None)
                .unwrap_or(false)
            {
                let apps_settings = settings().get::<AppsSettings>("apps-settings");
                for id in settings().get::<Vec<String>>("app-ids") {
                    if let Some(title) = apps_settings.get(&id).and_then(|x| x.get("title")) {
                        println!("{id}\t{title}");
                    }
                }
                return glib::ExitCode::SUCCESS;
            }

            // Get or create window to present
            let window: gtk::Window = if let Some(id) = command_line.arguments().get(1) {
                match get_app_details(&id.to_string_lossy())
                    .ok_or(anyhow!("No app with id {:?}", id))
                {
                    Ok(details) => AppWindow::new(&self.obj().clone(), &details).upcast(),
                    Err(err) => {
                        eprintln!("Error: {err}");
                        return glib::ExitCode::FAILURE;
                    }
                }
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
    pub fn new(flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("flags", flags)
            .property("application-id", Self::gen_app_id())
            .build()
    }

    fn gen_app_id() -> String {
        let args: Vec<String> = std::env::args().take(2).collect();

        if let Some(id) = args.get(1) {
            if settings().get::<Vec<String>>("app-ids").contains(id) {
                return format!("{}.{}", APP_ID, id);
            }
        };

        APP_ID.to_string()
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
                    let id = id
                        .expect("no id provided")
                        .get::<String>()
                        .expect("invalid id type provided");
                    if let Err(err) = app.open_app(&id) {
                        eprintln!("Failed to open app {id}: {err}")
                    }
                })
                .build(),
        ]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();
        let about = adw::AboutDialog::builder()
            .application_name("Spider")
            .application_icon(config::APP_ID)
            .developer_name("Zaedus")
            .version(config::VERSION)
            .developers(vec!["Zaedus"])
            .copyright("© 2024 Zaedus")
            .website("https://github.com/Zaedus/spider")
            .issue_url("https://github.com/Zaedus/spider/issues")
            .build();

        about.present(Some(&window));
    }

    fn open_app(&self, id: &str) -> anyhow::Result<()> {
        Command::new("spider").arg(id).spawn()?;
        Ok(())
    }
}
