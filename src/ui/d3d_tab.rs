use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct D3DModel {
    editing: bool,
    d3d_renderer: Option<String>,
    d3d_csmt: Option<u32>,
    offscreen_rendering_mode: Option<String>,
    video_memory_size: Option<u32>,
}

#[derive(Debug)]
pub enum D3DMsg {
    SetEditing(bool),
    SetD3DSettings {
        renderer: Option<String>,
        csmt: Option<u32>,
        offscreen_mode: Option<String>,
        video_memory: Option<u32>,
    },
    UpdateD3DRenderer(String),
    UpdateD3DCSMT(bool),
    UpdateOffscreenRenderingMode(String),
    UpdateVideoMemorySize(String),
}

#[relm4::component(pub)]
impl SimpleComponent for D3DModel {
    type Init = (
        Option<String>,
        Option<u32>,
        Option<String>,
        Option<u32>,
    );
    type Input = D3DMsg;
    type Output = D3DMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Direct3D Settings",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                // D3D Renderer
                gtk::Label {
                    set_label: "Renderer:",
                    set_halign: gtk::Align::Start,
                },

                gtk::ComboBoxText {
                    append_text: "OpenGL",
                    append_text: "Vulkan",
                    append_text: "GDI",
                    #[track = "model.changed(D3DModel::d3d_renderer())"]
                    set_active_id: model.d3d_renderer.as_deref(),
                    #[track = "model.changed(D3DModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(renderer) = combo.active_id() {
                            sender.input(D3DMsg::UpdateD3DRenderer(renderer.to_string()));
                        }
                    },
                },

                // CSMT
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_margin_top: 10,

                    gtk::CheckButton {
                        set_label: Some("Enable CSMT"),
                        #[track = "model.changed(D3DModel::d3d_csmt())"]
                        set_active: model.d3d_csmt.unwrap_or(0) != 0,
                        #[track = "model.changed(D3DModel::editing())"]
                        set_sensitive: model.editing,
                        connect_toggled[sender] => move |check| {
                            sender.input(D3DMsg::UpdateD3DCSMT(check.is_active()));
                        },
                    },
                },

                // Offscreen Rendering Mode
                gtk::Label {
                    set_label: "Offscreen Rendering Mode:",
                    set_halign: gtk::Align::Start,
                    set_margin_top: 10,
                },

                gtk::ComboBoxText {
                    append_text: "FBO",
                    append_text: "Backbuffer",
                    #[track = "model.changed(D3DModel::offscreen_rendering_mode())"]
                    set_active_id: model.offscreen_rendering_mode.as_deref(),
                    #[track = "model.changed(D3DModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |combo| {
                        if let Some(mode) = combo.active_id() {
                            sender.input(D3DMsg::UpdateOffscreenRenderingMode(mode.to_string()));
                        }
                    },
                },

                // Video Memory Size
                gtk::Label {
                    set_label: "Video Memory Size (MB):",
                    set_halign: gtk::Align::Start,
                    set_margin_top: 10,
                },

                gtk::Entry {
                    #[track = "model.changed(D3DModel::video_memory_size())"]
                    set_text: &model.video_memory_size.map(|s| s.to_string()).unwrap_or_default(),
                    #[track = "model.changed(D3DModel::editing())"]
                    set_editable: model.editing,
                    #[track = "model.changed(D3DModel::editing())"]
                    set_sensitive: model.editing,
                    connect_changed[sender] => move |entry| {
                        sender.input(D3DMsg::UpdateVideoMemorySize(entry.text().to_string()));
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
        let (renderer, csmt, offscreen_mode, video_memory) = init;
        
        let model = D3DModel {
            editing: false,
            d3d_renderer: renderer,
            d3d_csmt: csmt,
            offscreen_rendering_mode: offscreen_mode,
            video_memory_size: video_memory,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            D3DMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            D3DMsg::SetD3DSettings { renderer, csmt, offscreen_mode, video_memory } => {
                self.set_d3d_renderer(renderer);
                self.set_d3d_csmt(csmt);
                self.set_offscreen_rendering_mode(offscreen_mode);
                self.set_video_memory_size(video_memory);
            }
            D3DMsg::UpdateD3DRenderer(renderer) => {
                self.set_d3d_renderer(Some(renderer.clone()));
                let _ = sender.output(D3DMsg::UpdateD3DRenderer(renderer));
            }
            D3DMsg::UpdateD3DCSMT(enabled) => {
                self.set_d3d_csmt(Some(if enabled { 1 } else { 0 }));
                let _ = sender.output(D3DMsg::UpdateD3DCSMT(enabled));
            }
            D3DMsg::UpdateOffscreenRenderingMode(mode) => {
                self.set_offscreen_rendering_mode(Some(mode.clone()));
                let _ = sender.output(D3DMsg::UpdateOffscreenRenderingMode(mode));
            }
            D3DMsg::UpdateVideoMemorySize(size_str) => {
                if let Ok(size) = size_str.parse::<u32>() {
                    self.set_video_memory_size(Some(size));
                    let _ = sender.output(D3DMsg::UpdateVideoMemorySize(size_str));
                }
            }
        }
    }
}