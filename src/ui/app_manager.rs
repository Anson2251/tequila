use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use gtk::prelude::*;
use crate::prefix::config::{RegisteredExecutable, PrefixConfig};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppManagerModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    available_executables: Vec<RegisteredExecutable>,
    selected_executable: Option<usize>,
    scanning: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppManagerMsg {
    ScanForApplications,
    AddExecutable(usize),
    RemoveExecutable(usize),
    LaunchExecutable(usize),
    UpdateExecutableList(Vec<RegisteredExecutable>),
    SelectExecutable(usize),
    ConfigUpdated(PrefixConfig),
    ScanComplete(Result<Vec<RegisteredExecutable>, String>),
}

#[relm4::component(pub)]
impl SimpleComponent for AppManagerModel {
    type Init = (PathBuf, PrefixConfig);
    type Input = AppManagerMsg;
    type Output = AppManagerMsg;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 10,
            set_margin_all: 10,

            gtk::Label {
                set_label: "Application Manager",
                add_css_class: "heading",
                set_margin_bottom: 10,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,

                gtk::Button {
                    set_label: "Scan for Applications",
                    set_sensitive: !model.scanning,
                    connect_clicked => AppManagerMsg::ScanForApplications,
                    add_css_class: "suggested-action",
                },

                gtk::Spinner {
                    set_spinning: model.scanning,
                    set_visible: model.scanning,
                },
            },

            gtk::Separator {},

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_homogeneous: true,

                // Available executables
                gtk::Frame {
                    set_label: Some("Available Applications"),
                    set_hexpand: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        set_margin_all: 10,

                        gtk::Label {
                            set_label: &format!("{} applications found", model.available_executables.len()),
                            add_css_class: "caption",
                            set_visible: !model.available_executables.is_empty(),
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                            set_min_content_height: 200,
                            set_visible: !model.available_executables.is_empty(),

                            #[name = "available_list"]
                            gtk::ListBox {
                                set_css_classes: &["boxed-list"],
                            },
                        },

                        gtk::Label {
                            set_label: "No applications found\nClick 'Scan for Applications' to search",
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_wrap: true,
                            set_visible: model.available_executables.is_empty(),
                            add_css_class: "dim-label",
                        },
                    },
                },

                // Registered executables
                gtk::Frame {
                    set_label: Some("Registered Applications"),
                    set_hexpand: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        set_margin_all: 10,

                        gtk::Label {
                            set_label: &format!("{} applications registered", model.config.registered_executables.len()),
                            add_css_class: "caption",
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                            set_min_content_height: 200,

                            #[name = "registered_list"]
                            gtk::ListBox {
                                set_css_classes: &["boxed-list"],
                            },
                        },

                        gtk::Label {
                            set_label: "No registered applications\nAdd applications from left panel",
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_wrap: true,
                            set_visible: model.config.registered_executables.is_empty(),
                            add_css_class: "dim-label",
                        },
                    },
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,
                set_margin_top: 10,

                gtk::Button {
                    set_label: "Add Selected",
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender, selected = model.selected_executable] => move |_| {
                        if let Some(index) = selected {
                            sender.input(AppManagerMsg::AddExecutable(index));
                        }
                    },
                    add_css_class: "suggested-action",
                },

                gtk::Button {
                    set_label: "Remove Selected",
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender, selected = model.selected_executable] => move |_| {
                        if let Some(index) = selected {
                            sender.input(AppManagerMsg::RemoveExecutable(index));
                        }
                    },
                    add_css_class: "destructive-action",
                },

                gtk::Button {
                    set_label: "Launch",
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender, selected = model.selected_executable] => move |_| {
                        if let Some(index) = selected {
                            sender.input(AppManagerMsg::LaunchExecutable(index));
                        }
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefix_path, config) = init;
        
        let model = AppManagerModel {
            prefix_path,
            config: config.clone(),
            available_executables: Vec::new(),
            selected_executable: None,
            scanning: false,
        };

        let widgets = view_output!();

        // Populate registered executables list
        Self::populate_registered_list(&model, &widgets);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppManagerMsg::ScanForApplications => {
                self.scanning = true;
                self.selected_executable = None;

                // Start scanning in a separate thread
                let prefix_path = self.prefix_path.clone();
                let sender_clone = sender.clone();
                
                std::thread::spawn(move || {
                    // Create a temporary PrefixManager for scanning
                    let prefix_manager = crate::prefix::manager::PrefixManager::new(prefix_path.parent().unwrap_or(&prefix_path).to_path_buf());
                    
                    match prefix_manager.scan_for_applications(&prefix_path) {
                        Ok(executables) => {
                            sender_clone.input(AppManagerMsg::ScanComplete(Ok(executables)));
                        }
                        Err(e) => {
                            sender_clone.input(AppManagerMsg::ScanComplete(Err(format!("Scan failed: {}", e))));
                        }
                    }
                });
            }
            AppManagerMsg::AddExecutable(index) => {
                if index < self.available_executables.len() {
                    let executable = self.available_executables[index].clone();
                    self.config.add_executable(executable);
                    sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::RemoveExecutable(index) => {
                if index < self.config.registered_executables.len() {
                    self.config.remove_executable(index);
                    self.selected_executable = None;
                    sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::LaunchExecutable(index) => {
                if index < self.config.registered_executables.len() {
                    let executable = &self.config.registered_executables[index];
                    
                    // Create a temporary PrefixManager for launching
                    let prefix_manager = crate::prefix::manager::PrefixManager::new(self.prefix_path.parent().unwrap_or(&self.prefix_path).to_path_buf());
                    
                    match prefix_manager.launch_executable(&self.prefix_path, executable) {
                        Ok(_) => {
                            println!("Successfully launched: {}", executable.name);
                        }
                        Err(e) => {
                            eprintln!("Failed to launch executable '{}': {}", executable.name, e);
                            // TODO: Show error dialog to user
                        }
                    }
                }
            }
            AppManagerMsg::UpdateExecutableList(executables) => {
                self.available_executables = executables;
                self.selected_executable = None;
            }
            AppManagerMsg::SelectExecutable(index) => {
                self.selected_executable = Some(index);
            }
            AppManagerMsg::ConfigUpdated(config) => {
                self.config = config;
            }
            AppManagerMsg::ScanComplete(result) => {
                self.scanning = false;
                match result {
                    Ok(executables) => {
                        self.available_executables = executables;
                        self.selected_executable = None;
                    }
                    Err(error) => {
                        eprintln!("Scan failed: {}", error);
                    }
                }
            }
        }
    }
}

impl AppManagerModel {
    fn populate_available_list(
        model: &AppManagerModel,
        widgets: &<AppManagerModel as SimpleComponent>::Widgets,
    ) {
        // Clear existing items
        while let Some(row) = widgets.available_list.first_child() {
            widgets.available_list.remove(&row);
        }

        // Add available executables to list
        for (index, executable) in model.available_executables.iter().enumerate() {
            let exec_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(8)
                .margin_end(8)
                .build();

            // Icon or placeholder
            let icon_widget = if let Some(icon_path) = &executable.icon_path {
                if icon_path.exists() {
                    gtk::Image::from_file(icon_path)
                } else {
                    gtk::Image::from_icon_name("application-x-executable")
                }
            } else {
                gtk::Image::from_icon_name("application-x-executable")
            };

            icon_widget.set_pixel_size(24);
            exec_box.append(&icon_widget);

            // Executable info
            let info_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .build();

            let name_label = gtk::Label::builder()
                .label(&executable.name)
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();

            info_box.append(&name_label);

            if let Some(description) = &executable.description {
                let desc_label = gtk::Label::builder()
                    .label(description)
                    .halign(gtk::Align::Start)
                    // .add_css_class("caption")
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                info_box.append(&desc_label);
            }

            exec_box.append(&info_box);

            let row = gtk::ListBoxRow::builder().child(&exec_box).build();
            widgets.available_list.append(&row);
        }
    }

    fn populate_registered_list(
        model: &AppManagerModel,
        widgets: &<AppManagerModel as SimpleComponent>::Widgets,
    ) {
        // Clear existing items
        while let Some(row) = widgets.registered_list.first_child() {
            widgets.registered_list.remove(&row);
        }

        // Add registered executables to list
        for (index, executable) in model.config.registered_executables.iter().enumerate() {
            let exec_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(8)
                .margin_end(8)
                .build();

            // Icon or placeholder
            let icon_widget = if let Some(icon_path) = &executable.icon_path {
                if icon_path.exists() {
                    gtk::Image::from_file(icon_path)
                } else {
                    gtk::Image::from_icon_name("application-x-executable")
                }
            } else {
                gtk::Image::from_icon_name("application-x-executable")
            };

            icon_widget.set_pixel_size(24);
            exec_box.append(&icon_widget);

            // Executable info
            let info_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .build();

            let name_label = gtk::Label::builder()
                .label(&executable.name)
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();

            info_box.append(&name_label);

            if let Some(description) = &executable.description {
                let desc_label = gtk::Label::builder()
                    .label(description)
                    .halign(gtk::Align::Start)
                    // .add_css_class("caption")
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                info_box.append(&desc_label);
            }

            exec_box.append(&info_box);

            let row = gtk::ListBoxRow::builder().child(&exec_box).build();
            widgets.registered_list.append(&row);
        }
    }
}