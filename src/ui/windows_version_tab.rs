use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct WindowsVersionModel {
    editing: bool,
    windows_version: Option<String>,
}

#[derive(Debug)]
pub enum WindowsVersionMsg {
    SetEditing(bool),
    SetWindowsVersion(Option<String>),
    UpdateWindowsVersion(String),
}

#[relm4::component(pub)]
impl SimpleComponent for WindowsVersionModel {
    type Init = Option<String>;
    type Input = WindowsVersionMsg;
    type Output = WindowsVersionMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Windows Version",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::Label {
                    set_label: "Version:",
                    set_halign: gtk::Align::Start,
                },

                gtk::ComboBoxText {
                    append_text: "Windows 10",
                    append_text: "Windows 8.1",
                    append_text: "Windows 8",
                    append_text: "Windows 7",
                    append_text: "Windows Vista",
                    append_text: "Windows XP",
                    append_text: "Windows 2000",
                    append_text: "Windows ME",
                    append_text: "Windows 98",
                    append_text: "Windows 95",
                    #[track = "model.changed(WindowsVersionModel::windows_version())"]
                    set_active_id: model.windows_version.as_deref(),
                    #[track = "model.changed(WindowsVersionModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(version) = combo.active_id() {
                            sender.input(WindowsVersionMsg::UpdateWindowsVersion(version.to_string()));
                        }
                    },
                },
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = WindowsVersionModel {
            editing: false,
            windows_version: init,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            WindowsVersionMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            WindowsVersionMsg::SetWindowsVersion(version) => {
                self.set_windows_version(version);
            }
            WindowsVersionMsg::UpdateWindowsVersion(version) => {
                self.set_windows_version(Some(version.clone()));
                let _ = sender.output(WindowsVersionMsg::UpdateWindowsVersion(version));
            }
        }
    }
}