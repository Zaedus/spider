/* window.rs
 *
 * Copyright 2024 Unknown
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::app_row::AppRow;
use crate::application::settings;
use crate::create_app_dialog::CreateAppDialog;
use crate::config;

mod imp {

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/xyz/zaedus/spider/window.ui")]
    pub struct SpiderWindow {
        // Template widgets
        #[template_child]
        pub split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub apps_listbox: TemplateChild<gtk::ListBox>,

    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpiderWindow {
        const NAME: &'static str = "SpiderWindow";
        type Type = super::SpiderWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SpiderWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.setup_gactions();
            obj.refresh();
            self.apps_listbox.unselect_all();
        }
    }
    impl WidgetImpl for SpiderWindow {}
    impl WindowImpl for SpiderWindow {}
    impl ApplicationWindowImpl for SpiderWindow {}
    impl AdwApplicationWindowImpl for SpiderWindow {}

    #[gtk::template_callbacks]
    impl SpiderWindow {
        #[template_callback]
        fn on_add_clicked(&self, _: gtk::Button) {
            let dialog = CreateAppDialog::new();
            dialog.present(Some(&self.obj().clone()));
        }
    }
}

glib::wrapper! {
    pub struct SpiderWindow(ObjectSubclass<imp::SpiderWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,        @implements gio::ActionGroup, gio::ActionMap;
}

impl SpiderWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }
    fn setup_gactions(&self) {
        self.add_action_entries([
            gio::ActionEntry::builder("refresh")
                .activate(move |app: &Self, _, _| app.refresh())
                .build(),
        ]);
    }
    fn refresh(&self) {
        let imp = self.imp();
        imp.apps_listbox.remove_all();
        //let hidden = adw::ActionRow::new();
        //hidden.set_selectable(false);
        //imp.apps_listbox.append(&hidden);
        
        let settings = settings();
        for id in settings.get::<Vec<String>>("app-ids") {
            imp.apps_listbox.append(&AppRow::new(id));
        }
    }
}
