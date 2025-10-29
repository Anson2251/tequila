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
    editing: bool,
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
    // ShowAppManager,
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

                // gtk::Label {
                //     set_label: "Prefix Details",
                //     add_css_class: "heading",
                //     set_margin_bottom: 10,
                // },

                // gtk::Box {
                //     set_orientation: gtk::Orientation::Horizontal,
                //     set_spacing: 10,
                //     set_halign: gtk::Align::End,

                //     gtk::Button {
                //         set_label: "Manage Apps",
                //         connect_clicked => PrefixDetailsMsg::ShowAppManager,
                //         add_css_class: "suggested-action",
                //     },
                // },



                
                    // set_label_widget: Some(&{
                    //     gtk::Label::builder()
                    //         .label("Prefix Details")
                    //         .css_classes(["heading"])
                    //         .build()
                    // }),
                    
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

                    

                
                // },

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
            tracker: 0,
        };

        let widgets = view_output!();

        let local_config = &model.config.description.clone();
        // Initialize description text
        if let Some(description) = &local_config {
            widgets.description_text.buffer().set_text(description);
        }

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
                self.set_config(config);
                self.set_editing(false);
            }
            PrefixDetailsMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
            }
            // PrefixDetailsMsg::ShowAppManager => {
            //     // This message will be handled by the parent component (main.rs)
            //     let _ = sender.output(PrefixDetailsMsg::ShowAppManager);
            // }
        }
    }
}

// impl PrefixDetailsModel {
//     fn populate_executables_list(
//         model: &PrefixDetailsModel,
//         widgets: &gtk::ListBox,
//     ) {
//         // Clear existing items
//         while let Some(row) = widgets.first_child() {
//             widgets.remove(&row);
//         }

//         // Add executables to the list
//         for executable in &model.config.registered_executables {
//             let exec_box = gtk::Box::builder()
//                 .orientation(gtk::Orientation::Horizontal)
//                 .spacing(10)
//                 .margin_top(8)
//                 .margin_bottom(8)
//                 .margin_start(8)
//                 .margin_end(8)
//                 .build();

//             // Icon or placeholder
//             let icon_widget = if let Some(icon_path) = &executable.icon_path {
//                 if icon_path.exists() {
//                     gtk::Image::from_file(icon_path)
//                 } else {
//                     gtk::Image::from_icon_name("application-x-executable")
//                 }
//             } else {
//                 gtk::Image::from_icon_name("application-x-executable")
//             };

//             icon_widget.set_pixel_size(32);
//             exec_box.append(&icon_widget);

//             // Executable info
//             let info_box = gtk::Box::builder()
//                 .orientation(gtk::Orientation::Vertical)
//                 .spacing(2)
//                 .hexpand(true)
//                 .build();

//             let name_label = gtk::Label::builder()
//                 .label(&executable.name)
//                 .halign(gtk::Align::Start)
//                 .build();

//             info_box.append(&name_label);

//             if let Some(description) = &executable.description {
//                 let desc_label = gtk::Label::builder()
//                     .label(description)
//                     .halign(gtk::Align::Start)
//                     // .add_css_class("caption")
//                     .build();
//                 info_box.append(&desc_label);
//             }

//             let path_label = gtk::Label::builder()
//                 .label(&executable.executable_path.display().to_string())
//                 .halign(gtk::Align::Start)
//                 // .add_css_class("caption")
//                 .ellipsize(gtk::pango::EllipsizeMode::End)
//                 .build();

//             info_box.append(&path_label);
//             exec_box.append(&info_box);

//             let row = gtk::ListBoxRow::builder().child(&exec_box).build();
//             widgets.append(&row);
//         }

//         if model.config.registered_executables.is_empty() {
//             let no_executables_label = gtk::Label::builder()
//                 .label("No registered executables\nUse the application manager to add executables")
//                 .halign(gtk::Align::Center)
//                 .valign(gtk::Align::Center)
//                 .margin_top(20)
//                 .wrap(true)
//                 .wrap_mode(gtk::pango::WrapMode::WordChar)
//                 .build();

//             no_executables_label.add_css_class("dim-label");

//             let row = gtk::ListBoxRow::builder()
//                 .child(&no_executables_label)
//                 .selectable(false)
//                 .build();

//             widgets.append(&row);
//         }
//     }
// }
