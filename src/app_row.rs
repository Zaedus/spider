use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Object;
use gtk::glib;
use std::cell::{OnceCell, RefCell};

use crate::apps::{get_app_details, get_app_icon, AppDetails};

mod imp {

    use super::*;

    #[derive(Default, Debug, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/xyz/zaedus/spider/app_row.ui")]
    #[properties(wrapper_type = super::AppRow)]
    pub struct AppRow {
        #[template_child]
        pub icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        #[template_child]
        pub subtitle: TemplateChild<gtk::Label>,

        #[property(get, set = Self::on_id_set)]
        pub id: RefCell<String>,

        pub details: OnceCell<AppDetails>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppRow {
        const NAME: &'static str = "AppRow";
        type Type = super::AppRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AppRow {}
    impl WidgetImpl for AppRow {}
    impl ListBoxRowImpl for AppRow {}

    impl AppRow {
        fn on_id_set(&self, id: String) {
            self.id.replace(id.clone());
            glib::spawn_future_local(clone!(
                #[weak(rename_to = _self)]
                self,
                #[strong]
                id,
                async move {
                    let icon = get_app_icon(id.as_str()).await.unwrap();
                    let details = get_app_details(id).with_icon(icon);
                    _self
                        .details
                        .set(details.clone())
                        .expect("attempted to set id more than once");

                    _self.title.set_label(&details.title);
                    _self.subtitle.set_label(&details.url);
                    _self.icon.set_paintable(Some(&details.to_gdk_texture(64)));
                }
            ));
        }
    }
}

glib::wrapper! {
    pub struct AppRow(ObjectSubclass<imp::AppRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AppRow {
    pub fn new(id: String) -> Self {
        Object::builder().property("id", id).build()
    }
}
