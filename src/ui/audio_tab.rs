use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
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
                    append_text: "PulseAudio",
                    append_text: "ALSA",
                    append_text: "OSS",
                    append_text: "CoreAudio",
                    append_text: "Disabled",
                    #[track = "model.changed(AudioModel::audio_driver())"]
                    set_active_id: model.audio_driver.as_deref(),
                    #[track = "model.changed(AudioModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(driver) = combo.active_id() {
                            sender.input(AudioMsg::UpdateAudioDriver(driver.to_string()));
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