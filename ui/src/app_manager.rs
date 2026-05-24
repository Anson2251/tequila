use relm4::{
    RelmWidgetExt, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender, AsyncController}, gtk, prelude::AsyncComponentController, view
};
use gtk::prelude::*;
use prefix::config::{RegisteredExecutable, PrefixConfig};
use prefix::IconCache;
use prefix::ProcessTracker;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracker;
use crate::{
    registered_apps_list::{RegisteredAppsListModel, RegisteredAppsListMsg, RegisteredAppsListOutput},
    app_actions::{AppActionsModel, AppActionsMsg, AppActionsOutput},
    add_app_popover::{AddAppPopoverModel, AddAppPopoverMsg, AddAppPopoverOutput},
    executable_info_dialog::{ExecutableInfoDialogModel, ExecutableInfoDialogMsg, ExecutableInfoDialogOutput},
};

#[tracker::track]
pub struct AppManagerModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    scanning: bool,
    selected_executable: Option<usize>,
    available_executables: Vec<RegisteredExecutable>,
    #[tracker::do_not_track]
    registered_apps_list: AsyncController<RegisteredAppsListModel>,
    #[tracker::do_not_track]
    app_actions: AsyncController<AppActionsModel>,
    #[tracker::do_not_track]
    add_app_popover: AsyncController<AddAppPopoverModel>,
    #[tracker::do_not_track]
    executable_info_dialog: AsyncController<ExecutableInfoDialogModel>,
    #[tracker::do_not_track]
    icon_cache: Arc<IconCache>,
    #[tracker::do_not_track]
    prefix_store: Arc<prefix::PrefixStore>,
    #[tracker::do_not_track]
    process_tracker: Arc<Mutex<ProcessTracker>>,
    running_paths: HashSet<PathBuf>,
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
    ExecutableInfoDialog(ExecutableInfoDialogOutput),
    PollProcesses,
}

#[relm4::component(pub, async)]
impl AsyncComponent for AppManagerModel {
    type Init = (PathBuf, PrefixConfig, Arc<IconCache>, Arc<prefix::PrefixStore>);
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
                #[local_ref]
                registered_apps_list_widget -> gtk::Widget {},
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
        let (prefix_path, config, icon_cache, prefix_store) = init;

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
            .launch(prefix_path.clone())
            .forward(sender.input_sender(), |output| AppManagerMsg::ExecutableInfoDialog(output));

        let process_tracker = ProcessTracker::shared();

        // Poll process status every 5 seconds
        let poll_sender = sender.clone();
        gtk::glib::timeout_add_seconds_local(5, move || {
            poll_sender.input(AppManagerMsg::PollProcesses);
            gtk::glib::ControlFlow::Continue
        });

        let model = AppManagerModel {
            prefix_path,
            config: config.clone(),
            scanning: false,
            selected_executable: None,
            available_executables: Vec::new(),
            registered_apps_list,
            app_actions,
            add_app_popover,
            executable_info_dialog,
            icon_cache,
            prefix_store,
            process_tracker,
            running_paths: HashSet::new(),
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
                if self.prefix_path.as_os_str().is_empty() {
                    println!("Skipping scan: no prefix path set");
                    return;
                }

                self.set_scanning(true);
                self.add_app_popover.emit(AddAppPopoverMsg::SetScanning(true));
                self.set_selected_executable(None);
                self.app_actions.emit(AppActionsMsg::SetSelection(false));

                println!("Scanning for applications... {}", &self.prefix_path.display());

                let prefix_manager = prefix::Manager::new(
                    self.prefix_path.parent().unwrap_or(&self.prefix_path).to_path_buf(),
                    Arc::clone(&self.icon_cache),
                );
                let prefix_path = self.prefix_path.clone();

                match prefix_manager.scan_for_applications_async(&prefix_path).await {
                    Ok(executables) => {
                        println!("Scanning complete, found {} executables", executables.len());
                        let _ = self.prefix_store.save_scanned_executables(&self.prefix_path.to_string_lossy(), &executables);
                        self.available_executables = executables;
                    }
                    Err(e) => {
                        eprintln!("Scan failed: {}", e);
                    }
                }
                self.set_scanning(false);
                self.add_app_popover.emit(AddAppPopoverMsg::SetScanning(false));
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
                    self.app_actions.emit(AppActionsMsg::SetSelection(false));

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
                    let pp = self.prefix_path.clone();
                    let exe_path = executable.executable_path.clone();
                    let env_vars = executable.env_vars.clone();
                    let cwd = executable.cwd.clone();
                    let tracker = Arc::clone(&self.process_tracker);

                    let mut cmd = std::process::Command::new("wine");
                    cmd.env("WINEPREFIX", pp.to_string_lossy().as_ref())
                        .arg(&exe_path);

                    // Apply per-executable environment variables
                    for (key, value) in &env_vars {
                        cmd.env(key, value);
                    }

                    // Apply per-executable working directory (fall back to prefix path)
                    if let Some(ref wd) = cwd {
                        cmd.current_dir(wd);
                    } else {
                        cmd.current_dir(&pp);
                    }

                    match cmd.spawn()
                    {
                        Ok(child) => {
                            println!("Successfully launched: {}", executable.name);
                            let mut t = tracker.lock().unwrap();
                            t.register(&exe_path, child);
                            drop(t);
                            // Update the running state immediately
                            sender.input(AppManagerMsg::PollProcesses);
                        }
                        Err(e) => {
                            eprintln!("Failed to launch executable '{}': {}", executable.name, e);
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

                // Load available executables from DB (populated during refresh/sync)
                if !self.prefix_path.as_os_str().is_empty() {
                    match self.prefix_store.list_scanned_executables(&self.prefix_path.to_string_lossy()) {
                        Ok(exes) => {
                            self.available_executables = exes;
                        }
                        Err(e) => {
                            eprintln!("Failed to load scanned executables: {}", e);
                        }
                    }
                }

                // Update the registered apps list from cached config
                self.registered_apps_list.emit(RegisteredAppsListMsg::UpdateExecutables(self.config.registered_executables.clone()));

                // Restore running highlight immediately
                let paths = {
                    let t = self.process_tracker.lock().unwrap();
                    t.running_paths().into_iter().collect::<HashSet<_>>()
                };
                self.set_running_paths(paths.clone());
                self.registered_apps_list.emit(RegisteredAppsListMsg::SetRunningPaths(paths));

                // Update selected running state
                if let Some(i) = self.selected_executable {
                    if let Some(exe) = self.config.registered_executables.get(i) {
                        let running = self.process_tracker.lock().unwrap().is_running(&exe.executable_path);
                        self.app_actions.emit(AppActionsMsg::SetSelectedRunning(running));
                    }
                }

                // Reset selection if the config has no executables or index is out of bounds
                if self.config.registered_executables.is_empty()
                    || self.selected_executable.map_or(false, |i| i >= self.config.registered_executables.len())
                {
                    self.set_selected_executable(None);
                    self.app_actions.emit(AppActionsMsg::SetSelection(false));
                }
            }
            AppManagerMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
                self.set_selected_executable(None);
                self.app_actions.emit(AppActionsMsg::SetSelection(false));
            }
            AppManagerMsg::ShowInfoDialog(index) => {
                if let Some(executable) = self.config.registered_executables.get(index) {
                    self.executable_info_dialog.emit(ExecutableInfoDialogMsg::ShowInfo(executable.clone(), self.prefix_path.clone()));
                }
            }
            // Handle messages from child components
            AppManagerMsg::ExecutableInfoDialog(output) => {
                match output {
                    ExecutableInfoDialogOutput::ExecutableUpdated(updated_exec) => {
                        // Find and update the executable in the config
                        if let Some(pos) = self.config.registered_executables.iter().position(|e| {
                            e.executable_path == updated_exec.executable_path
                        }) {
                            self.config.registered_executables[pos] = updated_exec;

                            // Persist config to disk
                            if let Err(e) = self.config.save_to_file(&self.prefix_path) {
                                eprintln!("Failed to save config after updating executable settings: {}", e);
                            }

                            // Update the registered apps list view
                            self.registered_apps_list.emit(RegisteredAppsListMsg::UpdateExecutables(
                                self.config.registered_executables.clone(),
                            ));

                            let _ = sender.output(AppManagerMsg::ConfigUpdated(self.config.clone()));
                        }
                    }
                }
            }
            AppManagerMsg::RegisteredAppsList(output) => {
                println!("DEBUG: Received RegisteredAppsList output: {:?}", output);
                match output {
                    RegisteredAppsListOutput::Selected(index) => {
                        println!("DEBUG: Setting selected executable to: {}", index);
                        self.set_selected_executable(Some(index));
                        self.app_actions.emit(AppActionsMsg::SetSelection(true));
                        // Check if the selected app is running
                        if let Some(exe) = self.config.registered_executables.get(index) {
                            let running = self.process_tracker.lock().unwrap().is_running(&exe.executable_path);
                            self.app_actions.emit(AppActionsMsg::SetSelectedRunning(running));
                        }
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
                    AppActionsOutput::Kill => {
                        if let Some(index) = self.selected_executable {
                            if let Some(exe) = self.config.registered_executables.get(index) {
                                let killed = self.process_tracker.lock().unwrap().kill(&exe.executable_path);
                                if killed {
                                    self.app_actions.emit(AppActionsMsg::SetSelectedRunning(false));
                                    // Refresh the list highlight
                                    let paths = {
                                        let t = self.process_tracker.lock().unwrap();
                                        t.running_paths().into_iter().collect::<HashSet<_>>()
                                    };
                                    self.set_running_paths(paths.clone());
                                    self.registered_apps_list.emit(RegisteredAppsListMsg::SetRunningPaths(paths));
                                }
                            }
                        }
                    }
                    AppActionsOutput::Add => {
                        // Show popover with available executables (loaded from DB during sync)
                        self.add_app_popover.emit(AddAppPopoverMsg::UpdateAvailableApps(self.available_executables.clone(), self.config.architecture.clone()));

                        let app_actions_widget = self.app_actions.widget();
                        if let Some(box_widget) = app_actions_widget.downcast_ref::<gtk::Box>() {
                            if let Some(first_child) = box_widget.first_child() {
                                if let Some(add_button) = first_child.downcast_ref::<gtk::Button>() {
                                    self.add_app_popover.widget().set_parent(add_button);
                                }
                            }
                        }

                        self.add_app_popover.emit(AddAppPopoverMsg::Show);
                    }
                    AppActionsOutput::Remove => {
                        if let Some(index) = self.selected_executable {
                            sender.input(AppManagerMsg::RemoveExecutable(index));
                        }
                    }
                    AppActionsOutput::ShowInfo => {
                        if let Some(index) = self.selected_executable {
                            sender.input(AppManagerMsg::ShowInfoDialog(index));
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
                    AddAppPopoverOutput::Scan => {
                        sender.input(AppManagerMsg::ScanForApplications);
                    }
                }
            }
            AppManagerMsg::PollProcesses => {
                {
                    let mut t = self.process_tracker.lock().unwrap();
                    t.poll_dead();
                }
                let paths = {
                    let t = self.process_tracker.lock().unwrap();
                    t.running_paths().into_iter().collect::<HashSet<_>>()
                };
                self.set_running_paths(paths.clone());
                self.registered_apps_list.emit(RegisteredAppsListMsg::SetRunningPaths(paths));
                // Update selected running state
                if let Some(i) = self.selected_executable {
                    if let Some(exe) = self.config.registered_executables.get(i) {
                        let running = self.process_tracker.lock().unwrap().is_running(&exe.executable_path);
                        self.app_actions.emit(AppActionsMsg::SetSelectedRunning(running));
                    }
                }
            }
        }
    }
}
