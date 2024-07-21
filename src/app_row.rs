use adw::prelude::*;
use adw::subclass::prelude::*;
use gdk_pixbuf::Pixbuf;
use glib::clone;
use glib::Object;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use gio::MemoryInputStream;

use crate::apps::{get_app_details, get_app_icon};

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

        #[property(get, set = Self::on_id_set)]
        pub id: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppRow {
        const NAME: &'static str = "AppRow";
        type Type = super::AppRow;
        type ParentType = adw::ActionRow;

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
    impl PreferencesRowImpl for AppRow {}
    impl ActionRowImpl for AppRow {}

    impl AppRow {
        fn on_id_set(&self, id: String) {
            self.id.replace(id.clone());
            glib::spawn_future_local(clone!(
                #[weak(rename_to = _self)]
                self,
                #[strong]
                id,
                async move {
                    let icon = get_app_icon(id.clone()).await.unwrap();
                    let details = get_app_details(id).with_icon(icon);
                    _self.title.set_label(&details.title);
                    let bytes = glib::Bytes::from(details.icon.unwrap().as_slice());
                    let stream = MemoryInputStream::from_bytes(&bytes);
                    let pixbuf =
                        Pixbuf::from_stream_at_scale(&stream, 32, 32, true, gio::Cancellable::NONE)
                            .unwrap();
                    let texture = gdk::Texture::for_pixbuf(&pixbuf);
                    _self.icon.set_paintable(Some(&texture));
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
