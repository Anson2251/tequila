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
                    append_text: "Default",
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
                    set_active: win_code_to_index(model.windows_version.as_deref().unwrap_or("")),
                    #[track = "model.changed(WindowsVersionModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(idx) = combo.active() {
                            sender.input(WindowsVersionMsg::UpdateWindowsVersion(win_index_to_code(idx as u32).to_string()));
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

fn win_code_to_index(code: &str) -> Option<u32> {
    match code {
        "" | "none" => Some(0),
        "win10" => Some(1),
        "win81" => Some(2),
        "win8" => Some(3),
        "win7" => Some(4),
        "vista" => Some(5),
        "winxp" => Some(6),
        "win2k" => Some(7),
        "winme" => Some(8),
        "win98" => Some(9),
        "win95" => Some(10),
        _ => None,
    }
}

fn win_index_to_code(idx: u32) -> &'static str {
    match idx {
        0 => "",
        1 => "win10",
        2 => "win81",
        3 => "win8",
        4 => "win7",
        5 => "vista",
        6 => "winxp",
        7 => "win2k",
        8 => "winme",
        9 => "win98",
        10 => "win95",
        _ => "",
    }
}
