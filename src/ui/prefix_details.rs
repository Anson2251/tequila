use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use gtk::prelude::*;
use crate::prefix::config::PrefixConfig;
use std::path::PathBuf;
use tracker;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug)]
#[tracker::track]
pub struct PrefixDetailsModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    editing: bool,
    description_updated: bool,
    prefix_index: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum PrefixDetailsMsg {
    ToggleEdit,
    SaveConfig,
    UpdateName(String),
    UpdateDescription(String),
    UpdateArchitecture(String),
    UpdateWineVersion(String),
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
                        

                            gtk::ComboBoxText {
                                append_text: "win32",
                                append_text: "win64",
                                #[track = "model.changed(PrefixDetailsModel::config())"]
                                set_active_id: Some(&model.config.architecture),
                                #[track = "model.changed(PrefixDetailsModel::editing())"]
                                set_sensitive: model.editing,
                                connect_changed[sender] => move |combo| {
                                    if let Some(arch) = combo.active_id() {
                                        sender.input(PrefixDetailsMsg::UpdateArchitecture(arch.to_string()));
                                    }
                                },
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
                                #[track = "model.changed(PrefixDetailsModel::editing())"]
                                set_editable: model.editing,
                                #[track = "model.changed(PrefixDetailsModel::editing())"]
                                set_sensitive: model.editing,
                                connect_changed[sender] => move |entry| {
                                    sender.input(PrefixDetailsMsg::UpdateWineVersion(entry.text().to_string()));
                                },
                            },
                        },
                    },

                    gtk::Label {
                        set_label: "Description:",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_min_content_height: 100,

                        

                        #[name = "description_text"]
                        gtk::TextView {
                            set_buffer: Some(&gtk::TextBuffer::new(None)),
                            set_hexpand: true,
                            set_vexpand: true,

                            #[track = "model.changed(PrefixDetailsModel::editing())"]
                            set_editable: model.editing,
                            #[track = "model.changed(PrefixDetailsModel::editing())"]
                            set_sensitive: model.editing,
                            set_wrap_mode: gtk::WrapMode::WordChar,
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
                        connect_clicked[sender, config = model.config.clone()] => move |_| {
                            sender.input(PrefixDetailsMsg::ConfigUpdated(config.clone()));
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
            editing: false,
            description_updated: false,
            prefix_index: 0,
            tracker: 0,
        };

        let widgets = view_output!();

        // Initialize description text
        if let Some(description) = &model.config.description {
            widgets.description_text.buffer().set_text(description);
        }

        // Set up buffer change handler for description
        let buffer = widgets.description_text.buffer();
        let sender_clone = sender.clone();
        buffer.connect_changed(move |buffer| {
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, true);
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
                    // Save changes
                    sender.input(PrefixDetailsMsg::SaveConfig);
                } else {
                    self.set_editing(true);
                    println!("Setting to editing true");
                }
            }
            PrefixDetailsMsg::SaveConfig => {
                self.set_editing(false);

                // Update last modified timestamp before saving
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
            PrefixDetailsMsg::UpdateDescription(description) => {
                self.config.description = if description.is_empty() { None } else { Some(description) };
            }
            PrefixDetailsMsg::UpdateArchitecture(architecture) => {
                self.config.architecture = architecture;
            }
            PrefixDetailsMsg::UpdateWineVersion(version) => {
                self.config.wine_version = if version.is_empty() { None } else { Some(version) };
            }
            PrefixDetailsMsg::ConfigUpdated(config) => {
                self.set_config(config.clone());
                self.set_editing(false);
                self.set_description_updated(true);
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
