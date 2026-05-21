use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AudioModel {
    editing: bool,
    audio_driver: Option<String>,
}

#[derive(Debug)]
pub enum AudioMsg {
    SetEditing(bool),
    SetAudioDriver(Option<String>),
    UpdateAudioDriver(String),
}

#[relm4::component(pub)]
impl SimpleComponent for AudioModel {
    type Init = Option<String>;
    type Input = AudioMsg;
    type Output = AudioMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Audio Settings",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::Label {
                    set_label: "Audio Driver:",
                    set_halign: gtk::Align::Start,
                },

                gtk::ComboBoxText {
                    append_text: "Default",
                    append_text: "PulseAudio",
                    append_text: "ALSA",
                    append_text: "OSS",
                    append_text: "CoreAudio",
                    #[track = "model.changed(AudioModel::audio_driver())"]
                    set_active: aud_code_to_index(model.audio_driver.as_deref().unwrap_or("")),
                    #[track = "model.changed(AudioModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(idx) = combo.active() {
                            sender.input(AudioMsg::UpdateAudioDriver(aud_index_to_code(idx as u32).to_string()));
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
        let model = AudioModel {
            editing: false,
            audio_driver: init,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            AudioMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            AudioMsg::SetAudioDriver(driver) => {
                self.set_audio_driver(driver);
            }
            AudioMsg::UpdateAudioDriver(driver) => {
                self.set_audio_driver(Some(driver.clone()));
                let _ = sender.output(AudioMsg::UpdateAudioDriver(driver));
            }
        }
    }
}

fn aud_code_to_index(code: &str) -> Option<u32> {
    match code {
        "" => Some(0),
        "pulse" => Some(1),
        "alsa" => Some(2),
        "oss" => Some(3),
        "coreaudio" => Some(4),
        _ => None,
    }
}

fn aud_index_to_code(idx: u32) -> &'static str {
    match idx {
        0 => "",
        1 => "pulse",
        2 => "alsa",
        3 => "oss",
        4 => "coreaudio",
        _ => "",
    }
}
