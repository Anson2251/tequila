use relm4::{
    ComponentParts, ComponentSender, RelmWidgetExt, WidgetRef, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender, AsyncController}, gtk, prelude::{AsyncComponentController, FactoryVecDeque}, view
};
use gtk::prelude::*;
use crate::prefix::config::{RegisteredExecutable, PrefixConfig};
use std::{path::PathBuf, time::Duration};
use tracker;
use crate::ui::{
    registered_apps_list::{RegisteredAppsListModel, RegisteredAppsListMsg, RegisteredAppsListOutput},
    app_actions::{AppActionsModel, AppActionsMsg, AppActionsOutput},
    add_app_popover::{AddAppPopoverModel, AddAppPopoverMsg, AddAppPopoverOutput},
    executable_info_dialog::{ExecutableInfoDialogModel, ExecutableInfoDialogMsg},
};

#[derive(Debug)]
#[tracker::track]
pub struct AppManagerModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    scanning: bool,
    selected_executable: Option<usize>,
    available_executables: Vec<RegisteredExecutable>,
    #[tracker::do_not_track]
    show_popover_after_scan: bool,
    #[tracker::do_not_track]
    registered_apps_list: AsyncController<RegisteredAppsListModel>,
    #[tracker::do_not_track]
    app_actions: AsyncController<AppActionsModel>,
    #[tracker::do_not_track]
    add_app_popover: AsyncController<AddAppPopoverModel>,
    #[tracker::do_not_track]
    executable_info_dialog: AsyncController<ExecutableInfoDialogModel>,
}

#[derive(Debug)]
pub enum AppManagerMsg {
    ScanForApplications,
    AddExecutable(usize),
    AddExecutables(Vec<usize>),
    RemoveExecutable(usize),
    LaunchExecutable(usize),
    UpdateExecutableList(Vec<RegisteredExecutable>),
    SelectExecutable(usize),
    ConfigUpdated(PrefixConfig),
    PrefixPathUpdated(PathBuf),
    ShowInfoDialog(usize),
    // Messages from child components
    RegisteredAppsList(RegisteredAppsListOutput),
    AppActions(AppActionsOutput),
    AddAppPopover(AddAppPopoverOutput),
}

#[relm4::component(pub, async)]
impl AsyncComponent for AppManagerModel {
    type Init = (PathBuf, PrefixConfig);
    type Input = AppManagerMsg;
    type Output = AppManagerMsg;
    type CommandOutput = ();
    type Widgets = AppManagerWidgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 10,
            set_margin_all: 10,

            gtk::ScrolledWindow{
                gtk::Frame {
                    set_label: Some("Registered Applications"),
                    set_hexpand: true,

                    #[local_ref]
                    registered_apps_list_widget -> gtk::Widget {},
                },
            },

            // Action bar at bottom
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,
                set_margin_top: 10,

                #[local_ref]
                app_actions_widget -> gtk::Widget {},
            },
        }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<relm4::loading_widgets::LoadingWidgets> {
        view! {
            #[local]
            root {
                #[name(spinner)]
                gtk::Spinner {
                    start: (),
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_hexpand: true,
                    set_vexpand: true,
                }
            }
        }
        Some(relm4::loading_widgets::LoadingWidgets::new(root, spinner))
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (prefix_path, config) = init;
        
        // Initialize registered apps list component with the current registered executables
        let registered_apps_list = RegisteredAppsListModel::builder()
            .launch(config.registered_executables.clone())
            .forward(sender.input_sender(), |output| AppManagerMsg::RegisteredAppsList(output));

        // Initialize app actions component
        let app_actions = AppActionsModel::builder()
            .launch((false, false)) // (has_selection, is_scanning)
            .forward(sender.input_sender(), |output| AppManagerMsg::AppActions(output));

        // Initialize add app popover (hidden by default) - will be connected to the actual add button later
        let add_app_popover = AddAppPopoverModel::builder()
            .launch(gtk::Button::new())
            .forward(sender.input_sender(), |output| AppManagerMsg::AddAppPopover(output));

        // Initialize executable info dialog (hidden by default)
        let executable_info_dialog = ExecutableInfoDialogModel::builder()
            .launch(())
            .detach();

        let model = AppManagerModel {
            prefix_path,
            config: config.clone(),
            scanning: false,
            selected_executable: None,
            available_executables: Vec::new(),
            show_popover_after_scan: false,
            registered_apps_list,
            app_actions,
            add_app_popover,
            executable_info_dialog,
            tracker: 0
        };

        // Set up local references for child components
        let registered_apps_list_widget = model.registered_apps_list.widget().clone().upcast::<gtk::Widget>();
        let app_actions_widget = model.app_actions.widget().clone().upcast::<gtk::Widget>();

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.reset();
        match msg {
            AppManagerMsg::ScanForApplications => {
                self.set_scanning(true);
                self.set_selected_executable(None);

                println!("Scanning for applications... {}", &self.prefix_path.display());

                // Async scanning
                let prefix_manager = crate::prefix::Manager::new(self.prefix_path.parent().unwrap_or(&self.prefix_path).to_path_buf());
                let prefix_path = self.prefix_path.clone();
                
                match prefix_manager.scan_for_applications_async(&prefix_path).await {
                    Ok(executables) => {
                        println!("Scanning complete, found {} executables", executables.len());
                        self.available_executables = executables.clone();
                        self.set_selected_executable(None);
                        
                        // Check if we should show popover after scan (triggered by Add button)
                        if self.show_popover_after_scan && !executables.is_empty() {
                            // Update available apps in popover before showing
                            self.add_app_popover.emit(AddAppPopoverMsg::UpdateAvailableApps(self.available_executables.clone()));
                            
                            // Get the add button from the app_actions component
                            let app_actions_widget = self.app_actions.widget();
                            if let Some(box_widget) = app_actions_widget.downcast_ref::<gtk::Box>() {
                                if let Some(first_child) = box_widget.first_child() {
                                    if let Some(add_button) = first_child.downcast_ref::<gtk::Button>() {
                                        // Set the popover's parent to the add button directly
                                        self.add_app_popover.widget().set_parent(add_button);
                                    }
                                }
                            }
                            
                            self.add_app_popover.emit(AddAppPopoverMsg::Show);
                            
                            // Reset the flag
                            self.show_popover_after_scan = false;
                        }
                    }
                    Err(e) => {
                        eprintln!("Scan failed: {}", e);
                    }
                }
                self.set_scanning(false);
            }
            AppManagerMsg::AddExecutable(index) => {
                if let Some(executable) = self.available_executables.get(index) {
                    println!("Adding executable: {}", executable.name);
                    
                    self.config.add_executable(executable.clone());
                    
                    // Save config to file
                    if let Err(e) = self.config.save_to_file(&self.prefix_path) {
                        eprintln!("Failed to save config after adding executable: {}", e);
                    } else {
                        println!("Config saved successfully after adding executable");
                    }
                    
                    let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::AddExecutables(indices) => {
                println!("Adding {} executables: {:?}", indices.len(), indices);
                
                for &index in &indices {
                    if let Some(executable) = self.available_executables.get(index) {
                        println!("Adding executable: {}", executable.name);
                        self.config.add_executable(executable.clone());
                    }
                }
                
                // Save config to file
                if let Err(e) = self.config.save_to_file(&self.prefix_path) {
                    eprintln!("Failed to save config after adding executables: {}", e);
                } else {
                    println!("Config saved successfully after adding executables");
                }
                
                // Update the registered apps list with the new config's registered executables
                self.registered_apps_list.emit(RegisteredAppsListMsg::UpdateExecutables(self.config.registered_executables.clone()));
                
                let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
            }
            AppManagerMsg::RemoveExecutable(index) => {
                if index < self.config.registered_executables.len() {
                    self.config.remove_executable(index);
                    self.set_selected_executable(None);
                    
                    // Save config to file
                    if let Err(e) = self.config.save_to_file(&self.prefix_path) {
                        eprintln!("Failed to save config after removing executable: {}", e);
                    } else {
                        println!("Config saved successfully after removing executable");
                    }
                    
                    let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                }
            }
            AppManagerMsg::LaunchExecutable(index) => {
                if let Some(executable) = self.config.registered_executables.get(index) {
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
                self.available_executables = executables.clone();
                self.set_selected_executable(None);
                
                // Update the registered apps list with the current config's registered executables
                self.registered_apps_list.emit(RegisteredAppsListMsg::UpdateExecutables(self.config.registered_executables.clone()));
            }
            AppManagerMsg::SelectExecutable(index) => {
                println!("Selected executable: {}", index);
                self.set_selected_executable(Some(index));
            }
            AppManagerMsg::ConfigUpdated(config) => {
                self.set_config(config);
                
                // Update the registered apps list with the new config's registered executables
                self.registered_apps_list.emit(RegisteredAppsListMsg::UpdateExecutables(self.config.registered_executables.clone()));
                
                // Also update available executables by scanning
                sender.input(AppManagerMsg::ScanForApplications);
            }
            AppManagerMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
            }
            AppManagerMsg::ShowInfoDialog(index) => {
                if let Some(executable) = self.config.registered_executables.get(index) {
                    self.executable_info_dialog.emit(ExecutableInfoDialogMsg::ShowInfo(executable.clone()));
                }
            }
            // Handle messages from child components
            AppManagerMsg::RegisteredAppsList(output) => {
                println!("DEBUG: Received RegisteredAppsList output: {:?}", output);
                match output {
                    RegisteredAppsListOutput::Selected(index) => {
                        println!("DEBUG: Setting selected executable to: {}", index);
                        self.set_selected_executable(Some(index));
                    }
                    RegisteredAppsListOutput::Launch(index) => {
                        sender.input(AppManagerMsg::LaunchExecutable(index));
                    }
                    RegisteredAppsListOutput::Remove(index) => {
                        sender.input(AppManagerMsg::RemoveExecutable(index));
                    }
                    RegisteredAppsListOutput::ShowInfo(index) => {
                        sender.input(AppManagerMsg::ShowInfoDialog(index));
                    }
                }
            }
            AppManagerMsg::AppActions(output) => {
                println!("App actions: {:?}", output);
                match output {
                    AppActionsOutput::Launch => {
                        if let Some(index) = self.selected_executable {
                            sender.input(AppManagerMsg::LaunchExecutable(index));
                        }
                    }
                    AppActionsOutput::Add => {
                        // Set flag to show popover after scan completes
                        self.show_popover_after_scan = true;
                        
                        // First, scan for applications to populate available_executables
                        sender.input(AppManagerMsg::ScanForApplications);
                        
                        // We'll show the popover after the scan completes
                        // The scan will update available_executables, then we can show the popover
                    }
                    AppActionsOutput::Remove => {
                        if let Some(index) = self.selected_executable {
                            sender.input(AppManagerMsg::RemoveExecutable(index));
                        }
                    }
                }
            }
            AppManagerMsg::AddAppPopover(output) => {
                println!("DEBUG: Received AddAppPopover output: {:?}", output);
                match output {
                    AddAppPopoverOutput::AddApp(indices) => {
                        println!("Adding executables: {:?}", indices);
                        sender.input(AppManagerMsg::AddExecutables(indices));
                    }
                    AddAppPopoverOutput::Close => {
                        println!("DEBUG: Closing popover");
                        self.add_app_popover.widget().unparent();
                    }
                }
            }
        }
    }
}