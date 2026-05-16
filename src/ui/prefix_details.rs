use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use gtk::prelude::*;
use crate::prefix::config::PrefixConfig;
use std::path::PathBuf;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct PrefixDetailsModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    saved_config: PrefixConfig,
    editing: bool,
    prefix_index: usize,
    #[tracker::do_not_track]
    description_buffer: gtk::TextBuffer,
    #[tracker::do_not_track]
    suppress_update: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum PrefixDetailsMsg {
    ToggleEdit,
    SaveConfig,
    UpdateName(String),
    UpdateDescription(String),
    // Architecture and Wine Version are auto-detected and read-only
    CancelEdit,
    ConfigUpdated(PrefixConfig),
    PrefixPathUpdated(PathBuf),
    SetPrefixIndex(usize),
}

#[relm4::component(pub)]
impl SimpleComponent for PrefixDetailsModel {
    type Init = (PathBuf, PrefixConfig);
    type Input = PrefixDetailsMsg;
    type Output = PrefixDetailsMsg;

    view! {
        #[root]
        gtk::ScrolledWindow {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,

                    
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    set_margin_all: 10,

                    gtk::Label {
                        set_label: "Name:",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Entry {
                        #[track = "model.changed(PrefixDetailsModel::config())"]
                        set_text: &model.config.name,
                        set_hexpand: true,
                        #[track = "model.changed(PrefixDetailsModel::editing())"]
                        set_editable: model.editing,
                        #[track = "model.changed(PrefixDetailsModel::editing())"]
                        set_sensitive: model.editing,
                        connect_changed[sender] => move |entry| {
                            sender.input(PrefixDetailsMsg::UpdateName(entry.text().to_string()));
                        },
                    },

                    gtk::Box {
                        set_hexpand: true,
                        set_spacing: 10,
                        set_margin_top: 12,
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,

                            gtk::Label {
                                set_label: "Architecture:",
                                set_halign: gtk::Align::Start,
                            },
                        

                            gtk::Entry {
                                #[track = "model.changed(PrefixDetailsModel::config())"]
                                set_text: &model.config.architecture,
                                set_hexpand: true,
                                set_editable: false,
                                set_sensitive: false,
                                add_css_class: "monospace",
                            },
                        },
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,

                            gtk::Label {
                                set_label: "Wine Version:",
                                set_halign: gtk::Align::Start,
                            },

                            gtk::Entry {
                                #[track = "model.changed(PrefixDetailsModel::config())"]
                                set_text: &model.config.wine_version.as_deref().unwrap_or(""),
                                set_hexpand: true,
                                set_editable: false,
                                set_sensitive: false,
                                add_css_class: "monospace",
                            },
                        },
                    },

                    gtk::Label {
                        set_label: "Description:",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_min_content_height: 100,

                        #[name = "description_text"]
                        gtk::TextView {
                            set_hexpand: true,
                            set_vexpand: true,
                            #[track = "model.changed(PrefixDetailsModel::editing())"]
                            set_editable: model.editing,
                            set_wrap_mode: gtk::WrapMode::WordChar,
                            add_css_class: "desc-text",
                            set_css_classes: &["view", "card", "desc-text"],
                        },
                    },

                    gtk::Box {
                        set_hexpand: true,
                        set_spacing: 10,
                        set_margin_top: 12,
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::Box {
                            set_hexpand: true,
                            set_orientation: gtk::Orientation::Horizontal,

                            gtk::Label {
                                set_label: "Created:",
                                set_halign: gtk::Align::Start,
                            },

                            gtk::Label {
                                set_label: &model.config.creation_date.format("%Y-%m-%d %H:%M:%S").to_string(),
                                set_halign: gtk::Align::Start,
                                add_css_class: "caption",
                            },
                        },

                        gtk::Box {
                            set_hexpand: true,
                            set_orientation: gtk::Orientation::Horizontal,
                            gtk::Label {
                                set_label: "Last Modified:",
                                set_halign: gtk::Align::Start,
                            },

                            gtk::Label {
                                set_label: &model.config.last_modified.format("%Y-%m-%d %H:%M:%S").to_string(),
                                set_halign: gtk::Align::Start,
                                add_css_class: "caption",
                            },
                        }
                    },
                    

                    gtk::Box {
                        set_hexpand: true,
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Label {
                            set_label: "Path: ",
                            set_halign: gtk::Align::Start,
                        },

                        gtk::Label {
                            #[track = "model.changed(PrefixDetailsModel::prefix_path())"]
                            set_label: &model.prefix_path.to_string_lossy(),
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_halign: gtk::Align::End,
                    set_margin_top: 10,

                    gtk::Button {
                        #[track = "model.changed(PrefixDetailsModel::editing())"]
                        set_label: if model.editing { "Save" } else { "Edit" },
                        #[track = "model.changed(PrefixDetailsModel::editing())"]
                        add_css_class: if model.editing { "suggested-action" } else { "" },
                        connect_clicked => PrefixDetailsMsg::ToggleEdit,
                    },

                    gtk::Button {
                        set_label: "Cancel",
                        #[track = "model.changed(PrefixDetailsModel::editing())"]
                        set_visible: model.editing,
                        connect_clicked[sender] => move |_| {
                            sender.input(PrefixDetailsMsg::CancelEdit);
                        },
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
        let (prefix_path, config) = init;
        
        let model = PrefixDetailsModel {
            prefix_path,
            config: config.clone(),
            saved_config: config,
            editing: false,
            prefix_index: 0,
            description_buffer: gtk::TextBuffer::new(None),
            suppress_update: false,
            tracker: 0,
        };

        // Initialize description text from config into the model's buffer
        if let Some(description) = &model.config.description {
            model.description_buffer.set_text(description);
        }

        let widgets = view_output!();

        // Connect the buffer to the widget (only set once, never replaced)
        widgets.description_text.set_buffer(Some(&model.description_buffer));

        // Track user edits
        let buf = model.description_buffer.clone();
        let sender_clone = sender.clone();
        buf.connect_changed(move |_buf| {
            let (start, end) = _buf.bounds();
            let text = _buf.text(&start, &end, true);
            sender_clone.input(PrefixDetailsMsg::UpdateDescription(text.to_string()));
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            PrefixDetailsMsg::ToggleEdit => {
                if self.editing {
                    println!("Saving editing");
                    sender.input(PrefixDetailsMsg::SaveConfig);
                } else {
                    self.saved_config = self.config.clone();
                    self.set_editing(true);
                    println!("Setting to editing true");
                }
            }
            PrefixDetailsMsg::SaveConfig => {
                // Capture description from buffer before saving
                let (start, end) = self.description_buffer.bounds();
                let text = self.description_buffer.text(&start, &end, true);
                self.config.description = if text.is_empty() { None } else { Some(text.to_string()) };

                self.set_editing(false);
                self.config.update_last_modified();

                // Save config to file
                if let Err(e) = self.config.save_to_file(&self.prefix_path) {
                    eprintln!("Failed to save config after editing prefix details: {}", e);
                } else {
                    println!("Config saved successfully after editing prefix details");
                }

                let _ = sender.output(PrefixDetailsMsg::ConfigUpdated(self.config.clone()));
            }
            PrefixDetailsMsg::UpdateName(name) => {
                self.config.name = name;
            }
            PrefixDetailsMsg::UpdateDescription(desc) => {
                self.config.description = if desc.is_empty() { None } else { Some(desc) };
            }
            PrefixDetailsMsg::CancelEdit => {
                let text = self.saved_config.description.as_deref().unwrap_or("");
                self.description_buffer.set_text(text);
                self.set_config(self.saved_config.clone());
                self.set_editing(false);
            }
            PrefixDetailsMsg::ConfigUpdated(config) => {
                if let Some(ref desc) = config.description {
                    self.description_buffer.set_text(desc);
                } else {
                    self.description_buffer.set_text("");
                }
                self.set_config(config.clone());
                self.saved_config = config;
                self.set_editing(false);
            }
            PrefixDetailsMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
            }
            PrefixDetailsMsg::SetPrefixIndex(index) => {
                self.set_prefix_index(index);
            }
            // PrefixDetailsMsg::ShowAppManager => {
            //     // This message will be handled by the parent component (main.rs)
            //     let _ = sender.output(PrefixDetailsMsg::ShowAppManager);
            // }
        }
    }
}
