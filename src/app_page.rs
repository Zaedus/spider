use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::Object;
use gtk::{glib, gdk};
use std::cell::{OnceCell, RefCell};

use crate::apps::AppDetails;

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/xyz/zaedus/spider/app_page.ui")]
    #[properties(wrapper_type = super::AppPage)]
    pub struct AppPage {
        pub details: OnceCell<AppDetails>,

        #[property(get, set)]
        pub id: RefCell<String>,

        #[template_child] pub icon_image: TemplateChild<gtk::Image>,
        #[template_child] pub url_entry: TemplateChild<adw::EntryRow>,
        #[template_child] pub title_entry: TemplateChild<adw::EntryRow>,
        #[template_child] pub ltb_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child] pub ltf_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child] pub dtb_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child] pub dtf_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child] pub lt_colors: TemplateChild<adw::ExpanderRow>,
        #[template_child] pub dt_colors: TemplateChild<adw::ExpanderRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppPage {
        const NAME: &'static str = "AppPage";
        type Type = super::AppPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AppPage {
        fn constructed(&self) {
            self.parent_constructed();

            self.lt_colors.set_enable_expansion(false);
            self.dt_colors.set_enable_expansion(false);

            self.dtb_color.set_rgba(&gdk::RGBA::new(36.0/255.0,  36.0/255.0,  36.0/255.0,  1.0));
            self.dtf_color.set_rgba(&gdk::RGBA::new(255.0/255.0, 255.0/255.0, 255.0/255.0, 1.0));
            self.ltb_color.set_rgba(&gdk::RGBA::new(250.0/255.0, 250.0/255.0, 250.0/255.0, 1.0));
            self.ltf_color.set_rgba(&gdk::RGBA::new(50.0/255.0,  50.0/255.0,  50.0/255.0,  1.0));
        }
    }
    impl WidgetImpl for AppPage {}
    impl NavigationPageImpl for AppPage {}

    #[gtk::template_callbacks]
    impl AppPage {
        #[template_callback]
        fn on_show_clicked(&self, _: gtk::Button) {
            self.obj().activate_action("app.open-app", Some(&self.id.borrow().to_variant())).unwrap();
        }
    }
}


glib::wrapper! {
    pub struct AppPage(ObjectSubclass<imp::AppPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AppPage {
    pub fn new(details: AppDetails) -> Self {
        let obj: Self = Object::builder()
            .property("id", &details.id)
            .property("title", &details.title)
            .build();
        let imp = obj.imp();
        imp.details.set(details.clone()).unwrap();
        imp.icon_image.set_paintable(Some(&details.to_gdk_texture(256)));
        imp.title_entry.set_text(details.title.as_str());
        imp.url_entry.set_text(details.url.as_str());
        obj
    }
}

