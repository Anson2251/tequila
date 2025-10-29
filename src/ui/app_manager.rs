use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use relm4::factory::{FactoryComponent, FactorySender, FactoryVecDeque, DynamicIndex};
use gtk::prelude::*;
use crate::prefix::config::{RegisteredExecutable, PrefixConfig};
use std::path::PathBuf;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AppManagerModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    #[tracker::do_not_track]
    available_executables: FactoryVecDeque<AvailableExecutable>,
    #[tracker::do_not_track]
    registered_executables: FactoryVecDeque<RegisteredExecutableItem>,
    selected_executable: Option<usize>,
    scanning: bool,
    show_info_dialog: bool,
    info_dialog_executable: Option<RegisteredExecutable>,
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
    PrefixPathUpdated(PathBuf),
    ScanComplete(Result<Vec<RegisteredExecutable>, String>),
    AddSelected,
    RemoveSelected,
    LaunchSelected,
    ShowInfoDialog(usize),
    CloseInfoDialog,
}

// Factory component for available executables (remains as list for now)
#[derive(Debug)]
struct AvailableExecutable {
    executable: RegisteredExecutable,
    index: usize,
}

#[derive(Debug)]
enum AvailableExecutableMsg {
    Select,
}

#[derive(Debug)]
enum AvailableExecutableOutput {
    Selected(usize),
}

#[relm4::factory]
impl FactoryComponent for AvailableExecutable {
    type Init = (RegisteredExecutable, usize);
    type Input = AvailableExecutableMsg;
    type Output = AvailableExecutableOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        gtk::ListBoxRow {
            set_selectable: true,
            set_activatable: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_margin_top: 8,
                set_margin_bottom: 8,
                set_margin_start: 8,
                set_margin_end: 8,

                // Icon or placeholder
                gtk::Image {
                    set_pixel_size: 24,
                    #[watch]
                    set_from_file: self.executable.icon_path.as_ref(),
                    set_icon_name: Some("application-x-executable"),
                },

                // Executable info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 2,
                    set_hexpand: true,

                    gtk::Label {
                        #[watch]
                        set_label: &self.executable.name,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &self.executable.description.as_deref().unwrap_or(""),
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        add_css_class: "caption",
                        #[watch]
                        set_visible: self.executable.description.is_some(),
                    },
                },
            }
        }
    }

    fn init_model(
        init: Self::Init,
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let (executable, index) = init;
        Self { executable, index }
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            AvailableExecutableMsg::Select => {
                // Selection is handled through the output
            }
        }
    }
}

// Grid-based factory component for registered executables
#[derive(Debug)]
struct RegisteredExecutableItem {
    executable: RegisteredExecutable,
    index: usize,
    selected: bool,
}


#[derive(Debug)]
enum RegisteredExecutableMsg {
    Select,
    Launch,
    Remove,
    ShowInfo,
}

#[derive(Debug)]
enum RegisteredExecutableOutput {
    Selected(usize),
    Launch(usize),
    Remove(usize),
    ShowInfo(usize),
}

#[relm4::factory]
impl FactoryComponent for RegisteredExecutableItem {
    type Init = (RegisteredExecutable, usize);
    type Input = RegisteredExecutableMsg;
    type Output = RegisteredExecutableOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_all: 10,
            set_width_request: 32,
            set_height_request: 32,
            set_focusable: true,
            // set_selectable: true,
            
            // Selection indicator
            #[watch]
            set_css_classes: if self.selected { &["card", "selected"] } else { &["card"] },
            
            // Click handler for selection
            // set_cursor: gtk::gdk::Cursor::from_name("pointer", None).as_ref(),
            set_cursor_from_name: Some("pointer"),
            // connect_focus_on_click_notify[sender, index = self.index] => move |_, _| {
            //     sender.input(RegisteredExecutableMsg::Select);
            //     sender.output(RegisteredExecutableOutput::Selected(index)).unwrap();
            //     gtk::glib::Propagation::Proceed
            // },

            // Icon
            gtk::Image {
                set_pixel_size: 48,
                #[watch]
                set_from_file: self.executable.icon_path.as_ref(),
                set_icon_name: Some("application-x-executable"),
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
            },

            // Name
            gtk::Label {
                #[watch]
                set_label: &self.executable.name,
                set_halign: gtk::Align::Center,
                set_ellipsize: gtk::pango::EllipsizeMode::End,
                set_max_width_chars: 15,
                set_lines: 2,
            },

            // Action buttons (visible on hover/selection)
            // gtk::Box {
            //     set_orientation: gtk::Orientation::Horizontal,
            //     set_spacing: 4,
            //     set_halign: gtk::Align::Center,
            //     set_margin_top: 5,
            //     #[watch]
            //     set_visible: self.selected,

            //     gtk::Button {
            //         set_icon_name: "media-playback-start-symbolic",
            //         set_tooltip_text: Some("Launch"),
            //         add_css_class: "circular",
            //         add_css_class: "suggested-action",
            //         connect_clicked[sender, index = self.index] => move |_| {
            //             sender.output(RegisteredExecutableOutput::Launch(index)).unwrap();
            //         },
            //     },

            //     gtk::Button {
            //         set_icon_name: "dialog-information-symbolic",
            //         set_tooltip_text: Some("Info"),
            //         add_css_class: "circular",
            //         connect_clicked[sender, index = self.index] => move |_| {
            //             sender.output(RegisteredExecutableOutput::ShowInfo(index)).unwrap();
            //         },
            //     },

            //     gtk::Button {
            //         set_icon_name: "user-trash-symbolic",
            //         set_tooltip_text: Some("Remove"),
            //         add_css_class: "circular",
            //         add_css_class: "destructive-action",
            //         connect_clicked[sender, index = self.index] => move |_| {
            //             sender.output(RegisteredExecutableOutput::Remove(index)).unwrap();
            //         },
            //     },
            // },
        }
    }

    fn init_model(
        init: Self::Init,
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let (executable, index) = init;
        Self {
            executable,
            index,
            selected: false,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            RegisteredExecutableMsg::Select => {
                self.selected = true;
            }
            RegisteredExecutableMsg::Launch => {
                // Launch is handled through the output
            }
            RegisteredExecutableMsg::Remove => {
                // Remove is handled through the output
            }
            RegisteredExecutableMsg::ShowInfo => {
                // Show info is handled through the output
            }
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for AppManagerModel {
    type Init = (PathBuf, PrefixConfig);
    type Input = AppManagerMsg;
    type Output = AppManagerMsg;
    type Widgets = AppManagerWidgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 10,
            set_margin_all: 10,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,

                gtk::Button {
                    set_label: "Scan for Applications",
                    #[track = "model.changed(AppManagerModel::scanning())"]
                    set_sensitive: !model.scanning,
                    connect_clicked => AppManagerMsg::ScanForApplications,
                    add_css_class: "suggested-action",
                },

                gtk::Spinner {
                    #[track = "model.changed(AppManagerModel::scanning())"]
                    set_spinning: model.scanning,
                    #[track = "model.changed(AppManagerModel::scanning())"]
                    set_visible: model.scanning,
                },
            },

            gtk::Separator {},

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_homogeneous: true,

                // Available executables (left panel)
                gtk::Frame {
                    set_label: Some("Available Applications"),
                    set_hexpand: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        set_margin_all: 10,

                        gtk::Label {
                            #[watch]
                            set_label: &format!("{} applications found", model.available_executables.len()),
                            add_css_class: "caption",
                            #[watch]
                            set_visible: model.available_executables.len() > 0,
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                            set_min_content_height: 200,
                            #[watch]
                            set_visible: model.available_executables.len() > 0,

                            #[local_ref]
                            available_list_box -> gtk::ListBox {
                                set_css_classes: &["boxed-list"],
                                set_selection_mode: gtk::SelectionMode::Single,
                                connect_row_selected[sender] => move |_, row| {
                                    if let Some(row) = row {
                                        let index = row.index();
                                        sender.input(AppManagerMsg::SelectExecutable(index as usize));
                                    }
                                },
                            },
                        },

                        gtk::Label {
                            set_label: "No applications found\nClick 'Scan for Applications' to search",
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_wrap: true,
                            #[watch]
                            set_visible: model.available_executables.len() == 0,
                            add_css_class: "dim-label",
                        },
                    },
                },

                // Registered executables (right panel - grid layout)
                gtk::Frame {
                    set_label: Some("Registered Applications"),
                    set_hexpand: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        set_margin_all: 10,

                        gtk::Label {
                            #[watch]
                            set_label: &format!("{} applications registered", model.registered_executables.len()),
                            add_css_class: "caption",
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                            set_min_content_height: 200,

                            #[local_ref]
                            registered_grid -> gtk::FlowBox {
                                set_row_spacing: 10,
                                set_column_spacing: 10,
                                set_margin_all: 5,
                                set_max_children_per_line: 4,
                                set_selection_mode: gtk::SelectionMode::None,
                                set_homogeneous: true,
                            },
                        },

                        gtk::Label {
                            set_label: "No registered applications\nAdd applications from left panel",
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_wrap: true,
                            #[watch]
                            set_visible: model.registered_executables.len() == 0,
                            add_css_class: "dim-label",
                        },
                    },
                },
            },

            // Action bar at bottom
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,
                set_margin_top: 10,

                gtk::Button {
                    set_label: "Add Selected",
                    #[track = "model.changed(AppManagerModel::selected_executable()) || model.changed(AppManagerModel::scanning())"]
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender] => move |_| {
                        sender.input(AppManagerMsg::AddSelected);
                    },
                    add_css_class: "suggested-action",
                },

                gtk::Button {
                    set_label: "Remove Selected",
                    #[track = "model.changed(AppManagerModel::selected_executable()) || model.changed(AppManagerModel::scanning())"]
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender] => move |_| {
                        sender.input(AppManagerMsg::RemoveSelected);
                    },
                    add_css_class: "destructive-action",
                },

                gtk::Button {
                    set_label: "Launch",
                    #[track = "model.changed(AppManagerModel::selected_executable()) || model.changed(AppManagerModel::scanning())"]
                    set_sensitive: model.selected_executable.is_some() && !model.scanning,
                    connect_clicked[sender] => move |_| {
                        sender.input(AppManagerMsg::LaunchSelected);
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
        
        // Initialize factory for available executables
        let available_executables = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                AvailableExecutableOutput::Selected(index) => AppManagerMsg::SelectExecutable(index),
            });

        // Initialize factory for registered executables (grid layout)
        let registered_executables = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |output| match output {
                RegisteredExecutableOutput::Selected(index) => AppManagerMsg::SelectExecutable(index),
                RegisteredExecutableOutput::Launch(index) => AppManagerMsg::LaunchExecutable(index),
                RegisteredExecutableOutput::Remove(index) => AppManagerMsg::RemoveExecutable(index),
                RegisteredExecutableOutput::ShowInfo(index) => AppManagerMsg::ShowInfoDialog(index),
            });

        let model = AppManagerModel {
            prefix_path,
            config: config.clone(),
            available_executables,
            registered_executables,
            selected_executable: None,
            scanning: false,
            show_info_dialog: false,
            info_dialog_executable: None,
            tracker: 0
        };

        // Get references to the factory widgets
        let available_list_box = model.available_executables.widget();
        let registered_grid = model.registered_executables.widget();

        let widgets = view_output!();

        // sender.input(AppManagerMsg::ScanForApplications);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            AppManagerMsg::ScanForApplications => {
                self.set_scanning(true);
                self.set_selected_executable(None);

                println!("Scanning for applications... {}", &self.prefix_path.display());

                // Simple synchronous scanning - no background threads
                let prefix_manager = crate::prefix::Manager::new(self.prefix_path.parent().unwrap_or(&self.prefix_path).to_path_buf());
                match prefix_manager.scan_for_applications(&self.prefix_path) {
                    Ok(executables) => {
                        println!("Scanning complete, found {} executables", executables.len());
                        
                        // Update available executables factory
                        self.available_executables.guard().clear();
                        for (index, executable) in executables.iter().enumerate() {
                            self.available_executables.guard().push_back((executable.clone(), index));
                        }
                        
                        self.set_selected_executable(None);
                    }
                    Err(e) => {
                        eprintln!("Scan failed: {}", e);
                    }
                }
                self.set_scanning(false);
            }
            AppManagerMsg::AddExecutable(index) => {
                println!("trying to add executable");
                if let Some(executable) = self.available_executables.get(index).map(|item| item.executable.clone()) {
                    println!("Adding executable: {}", executable.name);
                    
                    self.config.add_executable(executable.clone());
                    
                    // Update registered executables factory
                    self.registered_executables.guard().clear();
                    for (idx, exe) in self.config.registered_executables.iter().enumerate() {
                        self.registered_executables.guard().push_back((exe.clone(), idx));
                    }
                    
                    let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::RemoveExecutable(index) => {
                if index < self.config.registered_executables.len() {
                    self.config.remove_executable(index);
                    self.set_selected_executable(None);
                    
                    // Update registered executables factory
                    self.registered_executables.guard().clear();
                    for (idx, exe) in self.config.registered_executables.iter().enumerate() {
                        self.registered_executables.guard().push_back((exe.clone(), idx));
                    }
                    
                    let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::LaunchExecutable(index) => {
                if let Some(executable) = self.registered_executables.get(index).map(|item| &item.executable) {
                    // Create a temporary PrefixManager for launching
                    let prefix_manager = crate::prefix::Manager::new(self.prefix_path.parent().unwrap_or(&self.prefix_path).to_path_buf());
                    
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
                // Update available executables factory
                self.available_executables.guard().clear();
                for (index, executable) in executables.iter().enumerate() {
                    self.available_executables.guard().push_back((executable.clone(), index));
                }
                self.set_selected_executable(None);
            }
            AppManagerMsg::SelectExecutable(index) => {
                println!("Selected executable: {}", index);
                self.set_selected_executable(Some(index));
                
                // Update selection state in registered executables
                self.registered_executables.guard().clear();
                for (idx, exe) in self.config.registered_executables.iter().enumerate() {
                    self.registered_executables.guard().push_back((exe.clone(), idx));
                }
            }
            AppManagerMsg::ConfigUpdated(config) => {
                self.set_config(config);
            }
            AppManagerMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
            }
            AppManagerMsg::ScanComplete(_) => {
                // This message is no longer used with synchronous scanning
            }
            AppManagerMsg::AddSelected => {
                if let Some(index) = self.selected_executable {
                    println!("Add selected: {}", index);
                    sender.input(AppManagerMsg::AddExecutable(index));
                }
            }
            AppManagerMsg::RemoveSelected => {
                if let Some(index) = self.selected_executable {
                    println!("Remove selected: {}", index);
                    sender.input(AppManagerMsg::RemoveExecutable(index));
                }
            }
            AppManagerMsg::LaunchSelected => {
                if let Some(index) = self.selected_executable {
                    println!("Launch selected: {}", index);
                    sender.input(AppManagerMsg::LaunchExecutable(index));
                }
            }
            AppManagerMsg::ShowInfoDialog(index) => {
                if let Some(executable) = self.registered_executables.get(index).map(|item| item.executable.clone()) {
                    self.set_info_dialog_executable(Some(executable));
                    self.set_show_info_dialog(true);
                }
            }
            AppManagerMsg::CloseInfoDialog => {
                self.set_show_info_dialog(false);
                self.set_info_dialog_executable(None);
            }
        }
    }
}

// Info dialog component for showing executable metadata
#[relm4::component]
impl SimpleComponent for InfoDialogModel {
    type Init = RegisteredExecutable;
    type Input = InfoDialogMsg;
    type Output = InfoDialogMsg;
    type Widgets = InfoDialogWidgets;

    view! {
        gtk::Dialog {
            set_title: Some("Executable Information"),
            set_modal: true,
            set_default_width: 400,
            set_default_height: 300,
            
            add_button: ("Close", gtk::ResponseType::Close),
            
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 20,
                
                // Header with icon and name
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 15,
                    set_margin_bottom: 15,
                    
                    gtk::Image {
                        set_pixel_size: 64,
                        set_from_file: model.executable.icon_path.as_ref(),
                        set_icon_name: Some("application-x-executable"),
                    },
                    
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        
                        gtk::Label {
                            set_label: &model.executable.name,
                            add_css_class: "heading",
                            set_halign: gtk::Align::Start,
                        },
                        
                        gtk::Label {
                            set_label: &model.executable.description.as_deref().unwrap_or("No description available"),
                            set_halign: gtk::Align::Start,
                            set_wrap: true,
                            set_wrap_mode: gtk::pango::WrapMode::WordChar,
                            add_css_class: "dim-label",
                        },
                    },
                },
                
                gtk::Separator {},
                
                // Metadata
                gtk::Frame {
                    set_label: Some("Details"),
                    
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,
                        set_margin_all: 15,
                        
                        // Executable path
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 5,
                            
                            gtk::Label {
                                set_label: "Path:",
                                set_width_chars: 12,
                                set_halign: gtk::Align::Start,
                            },
                            
                            gtk::Label {
                                set_label: &model.executable.executable_path.display().to_string(),
                                set_halign: gtk::Align::Start,
                                set_ellipsize: gtk::pango::EllipsizeMode::Start,
                                set_hexpand: true,
                                add_css_class: "monospace",
                                add_css_class: "caption",
                            },
                        },
                        
                        // Working directory
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 5,
                            
                            gtk::Label {
                                set_label: "Working Dir:",
                                set_width_chars: 12,
                                set_halign: gtk::Align::Start,
                            },
                            
                            gtk::Label {
                                set_label: "Not available",
                                set_halign: gtk::Align::Start,
                                set_ellipsize: gtk::pango::EllipsizeMode::Start,
                                set_hexpand: true,
                                add_css_class: "caption",
                            },
                        },
                        
                        // Arguments
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 5,
                            
                            gtk::Label {
                                set_label: "Arguments:",
                                set_width_chars: 12,
                                set_halign: gtk::Align::Start,
                            },
                            
                            gtk::Label {
                                set_label: "Not available",
                                set_halign: gtk::Align::Start,
                                set_hexpand: true,
                                add_css_class: "caption",
                            },
                        },
                        
                        // Environment variables
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 5,
                            
                            gtk::Label {
                                set_label: "Environment:",
                                set_width_chars: 12,
                                set_halign: gtk::Align::Start,
                            },
                            
                            gtk::Label {
                                set_label: "Not available",
                                set_halign: gtk::Align::Start,
                                set_hexpand: true,
                                add_css_class: "caption",
                            },
                        },
                    },
                },
            },
            
            connect_response[sender] => move |_, response| {
                if response == gtk::ResponseType::Close {
                    sender.input(InfoDialogMsg::Close);
                }
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = InfoDialogModel {
            executable: init,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            InfoDialogMsg::Close => {
                let _ = sender.output(InfoDialogMsg::Close);
            }
        }
    }
}

#[derive(Debug)]
struct InfoDialogModel {
    executable: RegisteredExecutable,
}

#[derive(Debug)]
enum InfoDialogMsg {
    Close,
}