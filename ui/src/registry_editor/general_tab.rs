use adw::prelude::*;
use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, adw, gtk};
use tracker;

#[derive(Debug)]
pub struct GeneralSettings {
    pub windows_version: Option<String>,
    pub audio_driver: Option<String>,
    pub log_pixels: Option<u32>,
    pub virtual_desktop_enabled: bool,
    pub virtual_desktop_width: u32,
    pub virtual_desktop_height: u32,
}

#[derive(Debug)]
#[tracker::track]
pub struct GeneralTabModel {
    editing: bool,
    windows_version: Option<String>,
    audio_driver: Option<String>,
    log_pixels: Option<u32>,
    virtual_desktop_enabled: bool,
    virtual_desktop_width: u32,
    virtual_desktop_height: u32,
}

#[derive(Debug)]
pub enum GeneralTabInput {
    SetEditing(bool),
    LoadSettings(GeneralSettings),
    UpdateField(String, String),
}

#[derive(Debug)]
pub enum GeneralTabOutput {
    SettingChanged(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for GeneralTabModel {
    type Init = GeneralSettings;
    type Input = GeneralTabInput;
    type Output = GeneralTabOutput;

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_vexpand: true,
            set_hexpand: true,
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vscrollbar_policy: gtk::PolicyType::Automatic,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_all: 18,
                set_vexpand: true,
                set_hexpand: true,

                adw::PreferencesGroup {
                set_title: "General Settings",
                set_description: Some("Configure basic Wine registry settings such as Windows version, audio, display, and virtual desktop"),

                // ── Windows Version ──
                adw::ActionRow {
                    set_title: "Windows Version",
                    set_subtitle: "Set the Windows version reported to applications",

                    add_suffix = &gtk::DropDown {
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_model: Some(&gtk::StringList::new(&[
                            "Default", "Windows 10", "Windows 8.1", "Windows 8",
                            "Windows 7", "Windows Vista", "Windows XP", "Windows 2000",
                            "Windows ME", "Windows 98", "Windows 95",
                        ])),
                        #[track = "model.changed(GeneralTabModel::windows_version())"]
                        set_selected: win_code_to_index(model.windows_version.as_deref().unwrap_or("")).unwrap_or(0),
                        #[track = "model.changed(GeneralTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_selected_notify[sender] => move |dd| {
                            sender.input(GeneralTabInput::UpdateField(
                                "windows_version".into(),
                                win_index_to_code(dd.selected()).to_string(),
                            ));
                        },
                    },
                },

                // ── Audio Driver ──
                adw::ActionRow {
                    set_title: "Audio Driver",
                    set_subtitle: "Select the audio backend",

                    add_suffix = &gtk::DropDown {
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_model: Some(&gtk::StringList::new(&[
                            "Default", "PulseAudio", "ALSA", "OSS", "CoreAudio",
                        ])),
                        #[track = "model.changed(GeneralTabModel::audio_driver())"]
                        set_selected: aud_code_to_index(model.audio_driver.as_deref().unwrap_or("")).unwrap_or(0),
                        #[track = "model.changed(GeneralTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_selected_notify[sender] => move |dd| {
                            sender.input(GeneralTabInput::UpdateField(
                                "audio_driver".into(),
                                aud_index_to_code(dd.selected()).to_string(),
                            ));
                        },
                    },
                },

                // ── DPI Scaling ──
                adw::ActionRow {
                    set_title: "DPI Scaling",
                    set_subtitle: "LogPixels value (96 = 100%, 120 = 125%, 144 = 150%, 192 = 200%)",

                    add_suffix = &gtk::SpinButton {
                        set_valign: gtk::Align::Center,
                        set_width_chars: 6,
                        set_adjustment: &gtk::Adjustment::builder()
                            .lower(96.0).upper(480.0).step_increment(1.0).page_increment(24.0)
                            .value(model.log_pixels.unwrap_or(96) as f64)
                            .build(),
                        #[track = "model.changed(GeneralTabModel::log_pixels())"]
                        set_value: model.log_pixels.unwrap_or(96) as f64,
                        #[track = "model.changed(GeneralTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_changed[sender] => move |spin| {
                            sender.input(GeneralTabInput::UpdateField(
                                "log_pixels".into(),
                                spin.text().to_string(),
                            ));
                        },
                    },
                },

                // ── Virtual Desktop ──
                adw::ActionRow {
                    set_title: "Virtual Desktop",
                    set_subtitle: "Emulate a virtual desktop resolution",
                    set_visible: cfg!(not(target_os = "macos")),

                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        #[track = "model.changed(GeneralTabModel::virtual_desktop_enabled())"]
                        set_active: model.virtual_desktop_enabled,
                        #[track = "model.changed(GeneralTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_active_notify[sender] => move |sw| {
                            sender.input(GeneralTabInput::UpdateField(
                                "vd_enabled".into(), sw.is_active().to_string(),
                            ));
                        },
                    },
                },

                adw::ActionRow {
                    #[watch]
                    set_visible: model.virtual_desktop_enabled,
                    set_title: "Desktop Width",

                    add_suffix = &gtk::Entry {
                        set_width_chars: 6,
                        set_valign: gtk::Align::Center,
                        #[track = "model.changed(GeneralTabModel::virtual_desktop_width())"]
                        set_text: &model.virtual_desktop_width.to_string(),
                        set_editable: model.editing,
                        set_sensitive: model.editing && model.virtual_desktop_enabled,
                        connect_changed[sender] => move |entry| {
                            sender.input(GeneralTabInput::UpdateField(
                                "vd_width".into(),
                                entry.text().to_string(),
                            ));
                        },
                    },
                },

                adw::ActionRow {
                    #[watch]
                    set_visible: model.virtual_desktop_enabled,
                    set_title: "Desktop Height",

                    add_suffix = &gtk::Entry {
                        set_width_chars: 6,
                        set_valign: gtk::Align::Center,
                        #[track = "model.changed(GeneralTabModel::virtual_desktop_height())"]
                        set_text: &model.virtual_desktop_height.to_string(),
                        set_editable: model.editing,
                        set_sensitive: model.editing && model.virtual_desktop_enabled,
                        connect_changed[sender] => move |entry| {
                            sender.input(GeneralTabInput::UpdateField(
                                "vd_height".into(),
                                entry.text().to_string(),
                            ));
                        },
                    },
                },
            }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = GeneralTabModel {
            editing: false,
            windows_version: init.windows_version,
            audio_driver: init.audio_driver,
            log_pixels: init.log_pixels,
            virtual_desktop_enabled: init.virtual_desktop_enabled,
            virtual_desktop_width: init.virtual_desktop_width,
            virtual_desktop_height: init.virtual_desktop_height,
            tracker: 0,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            GeneralTabInput::SetEditing(v) => {
                self.set_editing(v);
            }
            GeneralTabInput::LoadSettings(s) => {
                self.set_windows_version(s.windows_version);
                self.set_audio_driver(s.audio_driver);
                self.set_log_pixels(s.log_pixels);
                self.set_virtual_desktop_enabled(s.virtual_desktop_enabled);
                self.set_virtual_desktop_width(s.virtual_desktop_width);
                self.set_virtual_desktop_height(s.virtual_desktop_height);
            }
            GeneralTabInput::UpdateField(field, value) => match field.as_str() {
                "windows_version" => {
                    self.set_windows_version(Some(value.clone()));
                    let _ = sender.output(GeneralTabOutput::SettingChanged(
                        "Software\\Wine".into(),
                        format!("Version={}", value),
                    ));
                }
                "audio_driver" => {
                    self.set_audio_driver(Some(value.clone()));
                    let _ = sender.output(GeneralTabOutput::SettingChanged(
                        "Software\\Wine\\Drivers\\Audio".into(),
                        value,
                    ));
                }
                "log_pixels" => {
                    if let Ok(v) = value.parse::<u32>() {
                        self.set_log_pixels(Some(v));
                        let _ = sender.output(GeneralTabOutput::SettingChanged(
                            "Control Panel\\Desktop".into(),
                            format!("LogPixels={}", v),
                        ));
                    }
                }
                "vd_enabled" => {
                    if let Ok(v) = value.parse::<bool>() {
                        self.set_virtual_desktop_enabled(v);
                        let _ = sender.output(GeneralTabOutput::SettingChanged(
                            "Software\\Wine\\Explorer".into(),
                            if v {
                                "Desktop=Default".into()
                            } else {
                                String::new()
                            },
                        ));
                    }
                }
                "vd_width" => {
                    if let Ok(w) = value.parse::<u32>() {
                        self.set_virtual_desktop_width(w);
                        let _ = sender.output(GeneralTabOutput::SettingChanged(
                            "Software\\Wine\\Explorer\\Desktops".into(),
                            format!("Default={}x{}", w, self.virtual_desktop_height),
                        ));
                    }
                }
                "vd_height" => {
                    if let Ok(h) = value.parse::<u32>() {
                        self.set_virtual_desktop_height(h);
                        let _ = sender.output(GeneralTabOutput::SettingChanged(
                            "Software\\Wine\\Explorer\\Desktops".into(),
                            format!("Default={}x{}", self.virtual_desktop_width, h),
                        ));
                    }
                }
                _ => {}
            },
        }
    }
}

fn win_code_to_index(code: &str) -> Option<u32> {
    Some(match code {
        "" | "none" => 0,
        "win10" => 1,
        "win81" => 2,
        "win8" => 3,
        "win7" => 4,
        "vista" => 5,
        "winxp" => 6,
        "win2k" => 7,
        "winme" => 8,
        "win98" => 9,
        "win95" => 10,
        _ => return None,
    })
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

fn aud_code_to_index(code: &str) -> Option<u32> {
    Some(match code {
        "" => 0,
        "pulse" => 1,
        "alsa" => 2,
        "oss" => 3,
        "coreaudio" => 4,
        _ => return None,
    })
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
