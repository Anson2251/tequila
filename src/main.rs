use gtk::prelude::*;
use relm4::{ComponentController, ComponentParts, ComponentSender, Controller, RelmApp, RelmWidgetExt, SimpleComponent, Component, gtk};
use std::path::PathBuf;

// Import new modules
mod prefix;
mod ui;

use prefix::{Manager as PrefixManager, WinePrefix};
use ui::{PrefixListModel, PrefixDetailsModel, AppManagerModel};

struct AppModel {
    prefixes: Vec<WinePrefix>,
    prefix_manager: PrefixManager,
    selected_prefix: Option<usize>,
    prefix_list: Controller<PrefixListModel>,
    prefix_details: Controller<PrefixDetailsModel>,
    app_manager: Controller<AppManagerModel>,
    current_view: ViewType,
}

#[derive(Debug, Clone, PartialEq)]
enum ViewType {
    Empty,
    Content,
}

#[derive(Debug)]
#[allow(dead_code)]
enum AppMsg {
    CreatePrefix,
    DeletePrefix(usize),
    LaunchPrefix(usize),
    LaunchExecutable(usize, usize), // prefix index, executable index
    RefreshPrefixes,
    SelectPrefix(usize),
    ShowPrefixDetails(usize),
    // ShowAppManager(usize),
    HideDetails,
    ConfigUpdated(usize, prefix::config::PrefixConfig),
    ScanForApplications(usize),
    ShowCreatePrefixDialog,
    CreatePrefixComplete(String, String), // name, architecture
}

impl AppModel {
    fn scan_wine_prefixes(prefix_manager: &PrefixManager) -> Vec<WinePrefix> {
        match prefix_manager.scan_prefixes() {
            Ok(prefixes) => prefixes,
            Err(e) => {
                eprintln!("Error scanning prefixes: {}", e);
                Vec::new()
            }
        }
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "main_window"]
        gtk::ApplicationWindow {
            set_title: Some("Tequila - Wine Prefix Manager"),
            set_default_width: 800,
            set_default_height: 600,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,

                // Header bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_margin_bottom: 15,

                    gtk::Label {
                        set_label: "Wine Prefixes",
                        add_css_class: "title-1"
                    },

                    gtk::Box {
                        set_hexpand: true,
                        set_halign: gtk::Align::End,

                        gtk::Button {
                            set_label: "Refresh",
                            connect_clicked => AppMsg::RefreshPrefixes,
                        },

                        gtk::Button {
                            set_label: "Create New Prefix",
                            connect_clicked => AppMsg::ShowCreatePrefixDialog,
                            add_css_class: "suggested-action",
                            set_margin_start: 5
                        }
                    }
                },

                // Main content area - updated layout
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,

                    // Left panel - Enhanced prefix list
                    #[name = "prefix_list_container"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_width_request: 240,

                        gtk::Label {
                            set_label: "Prefix List",
                            add_css_class: "heading",
                            set_halign: gtk::Align::Start,
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                            #[local_ref]
                            prefix_list_widget -> gtk::Widget {}
                        }
                    },

                    // Right panel - Dynamic content
                    #[name = "details_container"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_hexpand: true,
                        set_width_request: 550,

                        match model.current_view {
                            ViewType::Empty => {
                                #[name = "empty_view"]
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_halign: gtk::Align::Center,
                                    set_valign: gtk::Align::Center,
                                    set_vexpand: true,

                                    gtk::Image {
                                        set_icon_name: Some("wine-symbolic"),
                                        set_pixel_size: 64,
                                        add_css_class: "dim-label",
                                    },

                                    gtk::Label {
                                        set_label: "No prefix selected",
                                        add_css_class: "title-4",
                                        add_css_class: "dim-label",
                                        set_margin_top: 10,
                                    },

                                    gtk::Label {
                                        set_label: "Select a prefix from the list to view details",
                                        add_css_class: "body",
                                        add_css_class: "dim-label",
                                    }
                                }
                            },
                            ViewType::Content => {
                                gtk::Box {
                                    set_hexpand: true,
                                    set_vexpand: true,
                                    set_orientation: gtk::Orientation::Vertical,

                                    gtk::Notebook {
                                        set_hexpand: true,
                                        set_vexpand: true,
                                        set_show_border: false,

                                        append_page: (
                                            &{
                                                model.app_manager.widget().clone().upcast::<gtk::Widget>()
                                            }, 
                                            Some(&{
                                                gtk::Label::builder().label("Apps").build()
                                            }
                                        )),

                                        append_page: (
                                            &{
                                                model.prefix_details.widget().clone().upcast::<gtk::Widget>()
                                            }, 
                                            Some(&{
                                                gtk::Label::builder().label("Details").build()
                                            }
                                        )),
                                    }
                                }
                            }
                        }
                            // ViewType::Details => {
                            //     #[name = "details_view"]
                            //         gtk::Box {
                            //             set_orientation: gtk::Orientation::Vertical,
                            //             set_spacing: 10,
                            //             set_margin_all: 10,

                            //             // gtk::Box {
                            //             //     set_orientation: gtk::Orientation::Horizontal,
                            //             //     set_spacing: 10,
                            //             //     set_halign: gtk::Align::End,

                            //             //     gtk::Button {
                            //             //         set_label: "Back to List",
                            //             //         connect_clicked => AppMsg::HideDetails,
                            //             //         add_css_class: "flat",
                            //             //     },

                            //             //     gtk::Button {
                            //             //         set_label: "Manage Apps",
                            //             //         connect_clicked => AppMsg::ShowAppManager(model.selected_prefix.unwrap_or(0)),
                            //             //         set_sensitive: model.selected_prefix.is_some(),
                            //             //         add_css_class: "suggested-action",
                            //             //     },
                            //             // },

                            //             // #[local_ref]
                            //             // prefix_details_widget -> gtk::Widget {},
                            //         }
                            // }
                            // ViewType::AppManager => {
                            //     #[name = "app_manager_view"]
                            //         gtk::Box {
                            //             set_orientation: gtk::Orientation::Vertical,
                            //             set_spacing: 10,
                            //             set_margin_all: 10,

                            //             gtk::Box {
                            //                 set_orientation: gtk::Orientation::Horizontal,
                            //                 set_spacing: 10,
                            //                 set_halign: gtk::Align::End,

                            //                 gtk::Button {
                            //                     set_label: "Back to Details",
                            //                     connect_clicked => AppMsg::ShowPrefixDetails(model.selected_prefix.unwrap_or(0)),
                            //                     set_sensitive: model.selected_prefix.is_some(),
                            //                     add_css_class: "flat",
                            //                 },

                            //                 gtk::Button {
                            //                     set_label: "Back to List",
                            //                     connect_clicked => AppMsg::HideDetails,
                            //                     add_css_class: "flat",
                            //                 },
                            //             },

                            //             #[local_ref]
                            //             app_manager_widget -> gtk::Widget {},
                            //         }
                            // }
                         
                    }       
                },

                // Status bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,
                    set_margin_top: 10,

                    gtk::Label {
                        set_label: &format!("{} prefixes loaded from {}",
                                          model.prefixes.len(),
                                          model.prefix_manager.wine_dir().display()),
                        add_css_class: "caption"
                    }
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let wine_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Wine");

        let prefix_manager = PrefixManager::new(wine_dir.clone());
        let prefixes = Self::scan_wine_prefixes(&prefix_manager);

        let prefix_list = PrefixListModel::builder()
            .launch((prefixes.clone(), None))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::prefix_list::PrefixListMsg::SelectPrefix(index) => AppMsg::SelectPrefix(index),
                crate::ui::prefix_list::PrefixListMsg::ShowPrefixDetails(index) => AppMsg::ShowPrefixDetails(index),
                // crate::ui::prefix_list::PrefixListMsg::ShowAppManager(index) => AppMsg::ShowAppManager(index),
            });

        let prefix_details = PrefixDetailsModel::builder()
            .launch((PathBuf::new(), prefix::config::PrefixConfig::new("".to_string(), "win64".to_string())))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config) // Use 0 as fallback index
                }
                // crate::ui::prefix_details::PrefixDetailsMsg::ShowAppManager => {
                //     AppMsg::ShowAppManager(0) // Use 0 as fallback index - will be updated when prefix is selected
                // }
                _ => AppMsg::RefreshPrefixes // Handle other messages
            });

        let app_manager = AppManagerModel::builder()
            .launch((PathBuf::new(), prefix::config::PrefixConfig::new("".to_string(), "win64".to_string())))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::app_manager::AppManagerMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config) // Use 0 as fallback index
                }
                _ => AppMsg::RefreshPrefixes // Handle other messages
            });

        let model = AppModel {
            prefixes,
            prefix_manager,
            selected_prefix: None,
            prefix_list,
            prefix_details,
            app_manager,
            current_view: ViewType::Empty,
        };

        // Set up local references for child components
        let prefix_list_widget = model.prefix_list.widget().clone().upcast::<gtk::Widget>();
        // let prefix_details_widget = model.prefix_details.widget().clone().upcast::<gtk::Widget>();
        // let app_manager_widget = model.app_manager.widget().clone().upcast::<gtk::Widget>();

        let widgets = view_output!();

        // #[cfg(not(target_os = "macos"))]
        {
            let header_bar = gtk::HeaderBar::new();
            // header_bar.set
            widgets.main_window.set_titlebar(Some(&header_bar));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ShowCreatePrefixDialog => {
                // Create a simple dialog for prefix creation
                let dialog = gtk::Dialog::builder()
                    .title("Create New Wine Prefix")
                    .modal(true)
                    .build();

                #[cfg(not(target_os = "macos"))]
                dialog.set_titlebar(&gtk::HeaderBar::new());

                dialog.add_button("Cancel", gtk::ResponseType::Cancel);
                dialog.add_button("Create", gtk::ResponseType::Ok);

                let content_area = dialog.content_area();
                let content_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(10)
                    .margin_top(10)
                    .margin_bottom(10)
                    .margin_start(10)
                    .margin_end(10)
                    .build();

                // Prefix name entry
                let name_label = gtk::Label::builder()
                    .label("Prefix Name:")
                    .halign(gtk::Align::Start)
                    .build();
                let name_entry = gtk::Entry::builder()
                    .placeholder_text("Enter prefix name")
                    .hexpand(true)
                    .width_chars(32)
                    .build();

                // Architecture selection
                let arch_label = gtk::Label::builder()
                    .label("Architecture:")
                    .halign(gtk::Align::Start)
                    .build();
                let arch_combo = gtk::ComboBoxText::builder()
                    .hexpand(true)
                    .build();
                arch_combo.append_text("win32");
                arch_combo.append_text("win64");
                arch_combo.set_active(Some(1)); // Default to win64

                content_box.append(&name_label);
                content_box.append(&name_entry);
                content_box.append(&arch_label);
                content_box.append(&arch_combo);

                content_area.append(&content_box);
                dialog.present();

                let sender_clone = sender.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Ok {
                        let name = name_entry.text().to_string();
                        let architecture = if let Some(active_text) = arch_combo.active_text() {
                            active_text.to_string()
                        } else {
                            "win64".to_string()
                        };

                        if !name.is_empty() {
                            sender_clone.input(AppMsg::CreatePrefixComplete(name, architecture));
                        } else {
                            eprintln!("Prefix name cannot be empty");
                            // TODO: Show error dialog
                        }
                    }
                    dialog.close();
                });
            }
            AppMsg::CreatePrefixComplete(prefix_name, architecture) => {
                if !prefix_name.is_empty() {
                    match self.prefix_manager.create_prefix(&prefix_name, &architecture) {
                        Ok(prefix_path) => {
                            println!("Created prefix: {} at {} with architecture {}",
                                prefix_name, prefix_path.display(), architecture);
                            // Refresh the prefix list
                            sender.input(AppMsg::RefreshPrefixes);
                        }
                        Err(e) => {
                            eprintln!("Failed to create prefix '{}': {}", prefix_name, e);
                            let dialog = gtk::Dialog::builder()
                                .title("Error")
                                .modal(true)
                                .build();

                            #[cfg(not(target_os = "macos"))]
                            dialog.set_titlebar(&gtk::HeaderBar::new());
                            
                            let content_area = dialog.content_area();
                            content_area.append(&gtk::Label::builder()
                                .label(&format!("Failed to create prefix '{}': {}", prefix_name, e))
                                .build());

                            dialog.add_button("OK", gtk::ResponseType::Ok);
                        }
                    }
                }
            }
            AppMsg::CreatePrefix => {
                // Legacy handler - now redirected to dialog
                sender.input(AppMsg::ShowCreatePrefixDialog);
            }
            AppMsg::DeletePrefix(index) => {
                if index < self.prefixes.len() {
                    let prefix_name = self.prefixes[index].name.clone();
                    let prefix_path = self.prefixes[index].path.clone();
                    
                    if let Err(e) = self.prefix_manager.delete_prefix(&prefix_path) {
                        eprintln!("Failed to delete prefix: {}", e);
                    } else {
                        self.prefixes.remove(index);
                        if self.selected_prefix == Some(index) {
                            self.selected_prefix = None;
                        } else if let Some(selected) = self.selected_prefix {
                            if selected > index {
                                self.selected_prefix = Some(selected - 1);
                            }
                        }
                        println!("Deleted prefix: {}", prefix_name);
                        sender.input(AppMsg::RefreshPrefixes);
                    }
                }
            }
            AppMsg::LaunchPrefix(index) => {
                if index < self.prefixes.len() {
                    let prefix_name = self.prefixes[index].name.clone();
                    let prefix_path = self.prefixes[index].path.clone();
                    
                    println!("Launching prefix: {} at {}", prefix_name, prefix_path.display());
                    
                    // Launch winecfg for the prefix
                    match self.prefix_manager.run_winecfg(&prefix_path) {
                        Ok(_) => {
                            println!("Successfully launched winecfg for prefix: {}", prefix_name);
                        }
                        Err(e) => {
                            eprintln!("Failed to launch winecfg for prefix {}: {}", prefix_name, e);
                            // TODO: Show error dialog to user
                        }
                    }
                }
            }
            AppMsg::LaunchExecutable(prefix_index, executable_index) => {
                if prefix_index < self.prefixes.len() {
                    let prefix_path = &self.prefixes[prefix_index].path;
                    let config = &self.prefixes[prefix_index].config;
                    
                    if executable_index < config.registered_executables.len() {
                        let executable = &config.registered_executables[executable_index];
                        if let Err(e) = self.prefix_manager.launch_executable(prefix_path, executable) {
                            eprintln!("Failed to launch executable: {}", e);
                        }
                    }
                }
            }
            AppMsg::RefreshPrefixes => {
                println!("Refreshing prefix list");
                self.prefixes = Self::scan_wine_prefixes(&self.prefix_manager);
                self.selected_prefix = None;
                self.current_view = ViewType::Empty;
                
                // Update the prefix list component
                self.prefix_list.emit(crate::ui::prefix_list::PrefixListMsg::SelectPrefix(0));
            }
            AppMsg::SelectPrefix(index) => {
                if index < self.prefixes.len() {
                    self.selected_prefix = Some(index);
                    println!("Selected prefix: {}", self.prefixes[index].name);
                    // Automatically show details when a prefix is selected
                    sender.input(AppMsg::ShowPrefixDetails(index));
                }
            }
            AppMsg::ShowPrefixDetails(index) => {
                if index < self.prefixes.len() {
                    self.selected_prefix = Some(index);
                    self.current_view = ViewType::Content;
                    
                    // Update the details component
                    let config = self.prefixes[index].config.clone();
                    self.prefix_details.emit(ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config.clone()));
                    self.app_manager.emit(ui::app_manager::AppManagerMsg::ConfigUpdated(config));
                    self.prefix_details.emit(ui::prefix_details::PrefixDetailsMsg::PrefixPathUpdated(self.prefixes[index].path.clone()));
                    self.app_manager.emit(ui::app_manager::AppManagerMsg::PrefixPathUpdated(self.prefixes[index].path.clone()));

                    println!("Showing details for prefix: {}", self.prefixes[index].name);
                }
            }
            AppMsg::HideDetails => {
                self.current_view = ViewType::Empty;
            }
            AppMsg::ConfigUpdated(index, config) => {
                if index < self.prefixes.len() {
                    let prefix_path = &self.prefixes[index].path;
                    if let Err(e) = self.prefix_manager.update_config(prefix_path, &config) {
                        eprintln!("Failed to update config: {}", e);
                    } else {
                        self.prefixes[index].config = config;
                    }
                }
            }
            AppMsg::ScanForApplications(index) => {
                if index < self.prefixes.len() {
                    let prefix_path = self.prefixes[index].path.clone();
                    let prefix_name = self.prefixes[index].name.clone();
                    
                    match self.prefix_manager.scan_for_applications(&prefix_path) {
                        Ok(executables) => {
                            println!("Found {} applications in prefix '{}'", executables.len(), prefix_name);
                            
                            // Get the current config and update it
                            let mut config = self.prefixes[index].config.clone();
                            let initial_count = config.registered_executables.len();
                            
                            for executable in executables {
                                config.add_executable(executable);
                            }
                            
                            let new_count = config.registered_executables.len();
                            let added_count = new_count - initial_count;
                            
                            // Save the updated config
                            if let Err(e) = self.prefix_manager.update_config(&prefix_path, &config) {
                                eprintln!("Failed to save updated config for prefix '{}': {}", prefix_name, e);
                            } else {
                                println!("Successfully updated prefix '{}' config with {} new executables (total: {})",
                                    prefix_name, added_count, new_count);
                                
                                // Update the local copy
                                self.prefixes[index].config = config;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to scan for applications in prefix '{}': {}", prefix_name, e);
                            // TODO: Show error dialog to user
                        }
                    }
                }
            }
        }
        
        // Update the view based on current state will be handled by Relm4 automatically
    }
}

fn main() {
    let app = RelmApp::new("com.github.tequila");
    app.run::<AppModel>(());
}
