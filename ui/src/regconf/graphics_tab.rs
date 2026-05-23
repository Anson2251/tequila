use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, adw};
use gtk::prelude::*;
use adw::prelude::*;
use tracker;

#[derive(Debug, Clone)]
pub struct GraphicsSettings {
    pub renderer: Option<String>,
    pub csmt: Option<u32>,
    pub offscreen_mode: Option<String>,
    pub video_memory: Option<u32>,
}

#[derive(Debug)]
#[tracker::track]
pub struct GraphicsTabModel {
    editing: bool,
    renderer: Option<String>,
    csmt: Option<u32>,
    offscreen_mode: Option<String>,
    video_memory: Option<u32>,
}

#[derive(Debug)]
pub enum GraphicsTabInput {
    SetEditing(bool),
    LoadSettings(GraphicsSettings),
    UpdateField(String, String),
}

#[derive(Debug)]
pub enum GraphicsTabOutput {
    SettingChanged(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for GraphicsTabModel {
    type Init = GraphicsSettings;
    type Input = GraphicsTabInput;
    type Output = GraphicsTabOutput;

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
                set_title: "Graphics Settings",
                set_description: Some("Configure Direct3D rendering, multithreading, and video memory"),

                // ── D3D Renderer ──
                adw::ActionRow {
                    set_title: "Renderer",
                    set_subtitle: "Direct3D rendering backend",

                    add_suffix = &gtk::DropDown {
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_model: Some(&gtk::StringList::new(&["Default", "OpenGL", "Vulkan", "GDI"])),
                        #[track = "model.changed(GraphicsTabModel::renderer())"]
                        set_selected: rdr_code_to_index(model.renderer.as_deref().unwrap_or("")).unwrap_or(0),
                        #[track = "model.changed(GraphicsTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_selected_notify[sender] => move |dd| {
                            sender.input(GraphicsTabInput::UpdateField(
                                "renderer".into(),
                                rdr_index_to_code(dd.selected()).to_string(),
                            ));
                        },
                    },
                },

                // ── CSMT ──
                adw::ActionRow {
                    set_title: "CSMT",
                    set_subtitle: "Command stream multithreading",

                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        #[track = "model.changed(GraphicsTabModel::csmt())"]
                        set_active: model.csmt.unwrap_or(0) != 0,
                        #[track = "model.changed(GraphicsTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_active_notify[sender] => move |sw| {
                            sender.input(GraphicsTabInput::UpdateField(
                                "csmt".into(), sw.is_active().to_string(),
                            ));
                        },
                    },
                },

                // ── Offscreen Rendering Mode ──
                adw::ActionRow {
                    set_title: "Offscreen Rendering Mode",
                    set_subtitle: "Method for rendering offscreen surfaces",

                    add_suffix = &gtk::DropDown {
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_model: Some(&gtk::StringList::new(&["Default", "FBO", "Backbuffer"])),
                        #[track = "model.changed(GraphicsTabModel::offscreen_mode())"]
                        set_selected: off_code_to_index(model.offscreen_mode.as_deref().unwrap_or("")).unwrap_or(0),
                        #[track = "model.changed(GraphicsTabModel::editing())"]
                        set_sensitive: model.editing,
                        connect_selected_notify[sender] => move |dd| {
                            sender.input(GraphicsTabInput::UpdateField(
                                "offscreen_mode".into(),
                                off_index_to_code(dd.selected()).to_string(),
                            ));
                        },
                    },
                },

                // ── Video Memory ──
                adw::ActionRow {
                    set_title: "Video Memory Size",
                    set_subtitle: "Amount of video memory in MB reported to applications",

                    add_suffix = &gtk::Entry {
                        set_width_chars: 8,
                        set_valign: gtk::Align::Center,
                        #[track = "model.changed(GraphicsTabModel::video_memory())"]
                        set_text: &model.video_memory.map(|s| s.to_string()).unwrap_or_default(),
                        set_editable: model.editing,
                        set_sensitive: model.editing,
                        connect_changed[sender] => move |entry| {
                            sender.input(GraphicsTabInput::UpdateField(
                                "video_memory".into(),
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
        let model = GraphicsTabModel {
            editing: false,
            renderer: init.renderer,
            csmt: init.csmt,
            offscreen_mode: init.offscreen_mode,
            video_memory: init.video_memory,
            tracker: 0,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            GraphicsTabInput::SetEditing(v) => {
                self.set_editing(v);
            }
            GraphicsTabInput::LoadSettings(s) => {
                self.set_renderer(s.renderer);
                self.set_csmt(s.csmt);
                self.set_offscreen_mode(s.offscreen_mode);
                self.set_video_memory(s.video_memory);
            }
            GraphicsTabInput::UpdateField(field, value) => {
                match field.as_str() {
                    "renderer" => {
                        self.set_renderer(Some(value.clone()));
                        let _ = sender.output(GraphicsTabOutput::SettingChanged(
                            "Software\\Wine\\Direct3D".into(), format!("renderer={}", value),
                        ));
                    }
                    "csmt" => {
                        let v = if value == "true" { 1 } else { 0 };
                        self.set_csmt(Some(v));
                        let _ = sender.output(GraphicsTabOutput::SettingChanged(
                            "Software\\Wine\\Direct3D".into(), format!("csmt={}", v),
                        ));
                    }
                    "offscreen_mode" => {
                        self.set_offscreen_mode(Some(value.clone()));
                        let _ = sender.output(GraphicsTabOutput::SettingChanged(
                            "Software\\Wine\\Direct3D".into(), format!("OffscreenRenderingMode={}", value),
                        ));
                    }
                    "video_memory" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.set_video_memory(Some(v));
                            let _ = sender.output(GraphicsTabOutput::SettingChanged(
                                "Software\\Wine\\Direct3D".into(), format!("VideoMemorySize={}", v),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn rdr_code_to_index(code: &str) -> Option<u32> {
    Some(match code { "" => 0, "gl" => 1, "vulkan" => 2, "gdi" => 3, _ => return None })
}

fn rdr_index_to_code(idx: u32) -> &'static str {
    match idx { 0 => "", 1 => "gl", 2 => "vulkan", 3 => "gdi", _ => "" }
}

fn off_code_to_index(code: &str) -> Option<u32> {
    Some(match code { "" => 0, "fbo" => 1, "backbuffer" => 2, _ => return None })
}

fn off_index_to_code(idx: u32) -> &'static str {
    match idx { 0 => "", 1 => "fbo", 2 => "backbuffer", _ => "" }
}
