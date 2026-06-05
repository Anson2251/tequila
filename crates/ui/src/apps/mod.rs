pub mod actions;
pub mod add_popover;
pub mod info_dialog;
pub mod list;

use crate::{
    apps::actions::{AppActionsModel, AppActionsMsg, AppActionsOutput},
    apps::add_popover::{AddAppPopoverModel, AddAppPopoverMsg, AddAppPopoverOutput},
    apps::info_dialog::{
        ExecutableInfoDialogModel, ExecutableInfoDialogMsg, ExecutableInfoDialogOutput,
    },
    apps::list::{RegisteredAppsListModel, RegisteredAppsListMsg, RegisteredAppsListOutput},
};
use adw::prelude::*;
use log::{debug, error, info};
use prefix::config::{PrefixConfig, RegisteredExecutable};
use relm4::adw;
use relm4::{
    RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender, AsyncController},
    gtk,
    prelude::AsyncComponentController,
    view,
};
use service::AppService;
use std::collections::HashSet;
use std::path::PathBuf;
use tracker;

#[tracker::track]
pub struct AppManagerModel {
    prefix: prefix::Prefix,
    scanning: bool,
    selected_executable: Option<usize>,
    #[tracker::do_not_track]
    available_executables: Vec<RegisteredExecutable>,
    #[tracker::do_not_track]
    registered_apps_list: AsyncController<RegisteredAppsListModel>,
    #[tracker::do_not_track]
    app_actions: AsyncController<AppActionsModel>,
    #[tracker::do_not_track]
    add_app_popover: AsyncController<AddAppPopoverModel>,
    #[tracker::do_not_track]
    executable_info_dialog: AsyncController<ExecutableInfoDialogModel>,
    running_paths: HashSet<PathBuf>,
    #[tracker::do_not_track]
    uninstaller_track_path: Option<PathBuf>,
    #[tracker::do_not_track]
    external_running: HashSet<PathBuf>,
}

#[derive(Debug)]
pub enum AppManagerMsg {
    ScanForApplications,
    AddExecutable(usize),
    AddExecutables(Vec<usize>),
    RemoveExecutable(usize),
    LaunchExecutable(usize),
    LaunchDirectExe(PathBuf),
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
    type Init = (prefix::Prefix, gtk::Window);
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
        let (prefix, main_window) = init;

        let icon_cache = AppService::global()
            .prefix_manager()
            .scanner()
            .icon_cache()
            .clone();

        // Initialize registered apps list component with the current registered executables
        let registered_apps_list = RegisteredAppsListModel::builder()
            .launch((
                prefix.config().registered_executables.clone(),
                prefix.path().to_path_buf(),
                icon_cache.clone(),
            ))
            .forward(sender.input_sender(), |output| {
                AppManagerMsg::RegisteredAppsList(output)
            });

        // Initialize app actions component
        let has_prefix = !prefix.path().as_os_str().is_empty();
        let app_actions = AppActionsModel::builder()
            .launch((false, false, has_prefix)) // (has_selection, is_scanning, prefix_set)
            .forward(sender.input_sender(), |output| {
                AppManagerMsg::AppActions(output)
            });

        // Initialize add app popover (hidden by default) - will be connected to the actual add button later
        let add_app_popover = AddAppPopoverModel::builder()
            .launch((gtk::Button::new(), prefix.path().to_path_buf(), icon_cache.clone()))
            .forward(sender.input_sender(), |output| {
                AppManagerMsg::AddAppPopover(output)
            });

        // Initialize executable info dialog (hidden by default)
        let executable_info_dialog = ExecutableInfoDialogModel::builder()
            .launch((prefix.path().to_path_buf(), main_window.clone(), icon_cache.clone()))
            .forward(sender.input_sender(), |output| {
                AppManagerMsg::ExecutableInfoDialog(output)
            });

        // Poll process status every 5 seconds
        let poll_sender = sender.clone();
        gtk::glib::timeout_add_seconds_local(5, move || {
            poll_sender.input(AppManagerMsg::PollProcesses);
            gtk::glib::ControlFlow::Continue
        });

        let model = AppManagerModel {
            prefix,
            scanning: false,
            selected_executable: None,
            available_executables: Vec::new(),
            registered_apps_list,
            app_actions,
            add_app_popover,
            executable_info_dialog,
            running_paths: HashSet::new(),
            uninstaller_track_path: None,
            external_running: HashSet::new(),
            tracker: 0,
        };

        // Set up local references for child components
        let registered_apps_list_widget = model
            .registered_apps_list
            .widget()
            .clone()
            .upcast::<gtk::Widget>();
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
                if self.prefix.path().as_os_str().is_empty() {
                    info!("[apps] skipping scan: no prefix path set");
                    return;
                }

                self.set_scanning(true);
                self.add_app_popover
                    .emit(AddAppPopoverMsg::SetScanning(true));
                self.set_selected_executable(None);
                self.app_actions.emit(AppActionsMsg::SetSelection(false));

                info!(
                    "[apps] scanning for applications... {}",
                    self.prefix.path().display()
                );

                match self.prefix.scan_applications_async().await {
                    Ok(all) => {
                        info!("[apps] scanning complete, found {} executables", all.len());
                        let _ = AppService::global()
                            .prefix_store()
                            .save_scanned_executables(&self.prefix.path().to_string_lossy(), &all);
                        self.available_executables = all.clone();
                        // Refresh popover list with scanned results
                        self.add_app_popover
                            .emit(AddAppPopoverMsg::UpdateAvailableApps(
                                all.clone(),
                                self.prefix.config().architecture.clone(),
                            ));
                    }
                    Err(e) => {
                        error!("[apps] scan failed: {}", e);
                    }
                }
                self.set_scanning(false);
                self.add_app_popover
                    .emit(AddAppPopoverMsg::SetScanning(false));
            }
            AppManagerMsg::AddExecutable(index) => {
                if let Some(executable) = self.available_executables.get(index) {
                    info!("[apps] adding executable: {}", executable.name);
                    let path = self.prefix.path().to_path_buf();
                    if service::config_ops::add_executable(
                        &AppService::global(),
                        &path,
                        self.prefix.config_mut(),
                        executable.clone(),
                    ) {
                        let _ = sender.output(AppManagerMsg::ConfigUpdated(self.prefix.config().clone()));
                    }
                }
            }
            AppManagerMsg::AddExecutables(indices) => {
                info!("[apps] adding {} executables: {:?}", indices.len(), indices);

                let exes: Vec<_> = indices
                    .iter()
                    .filter_map(|&i| self.available_executables.get(i).cloned())
                    .collect();

                let path = self.prefix.path().to_path_buf();
                if service::config_ops::add_executables(
                    &AppService::global(),
                    &path,
                    self.prefix.config_mut(),
                    &exes,
                ) {
                    self.registered_apps_list
                        .emit(RegisteredAppsListMsg::UpdateExecutables(
                            self.prefix.config().registered_executables.clone(),
                        ));
                    let _ = sender.output(AppManagerMsg::ConfigUpdated(self.prefix.config().clone()));
                }
            }
            AppManagerMsg::RemoveExecutable(index) => {
                if index < self.prefix.config().registered_executables.len() {
                    self.set_selected_executable(None);
                    self.app_actions.emit(AppActionsMsg::SetSelection(false));
                    self.app_actions
                        .emit(AppActionsMsg::SetDesktopExists(false));

                    let path = self.prefix.path().to_path_buf();
                    if service::config_ops::remove_executable(
                        &AppService::global(),
                        &path,
                        self.prefix.config_mut(),
                        index,
                    ) {
                        let _ = sender.output(AppManagerMsg::ConfigUpdated(self.prefix.config().clone()));
                    }
                }
            }
            AppManagerMsg::LaunchExecutable(index) => {
                if let Some(executable) = self.prefix.config().registered_executables.get(index) {
                    match service::launch::launch_executable(
                        &AppService::global(),
                        self.prefix.path(),
                        executable,
                    ) {
                        Ok(_pid) => {
                            sender.input(AppManagerMsg::PollProcesses);
                        }
                        Err(e) => {
                            let parent_window = _root
                                .ancestor(gtk::Window::static_type())
                                .and_then(|w| w.downcast::<gtk::Window>().ok());
                            let alert = adw::AlertDialog::new(
                                Some("Launch Failed"),
                                Some(&format!("Failed to launch '{}':\n\n{}", executable.name, e)),
                            );
                            alert.add_response("ok", "OK");
                            alert.set_default_response(Some("ok"));
                            alert.set_close_response("ok");
                            alert.choose(
                                parent_window.as_ref(),
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                        }
                    }
                }
            }
            AppManagerMsg::UpdateExecutableList(executables) => {
                self.available_executables = executables.clone();
                self.set_selected_executable(None);

                // Update the registered apps list with the current config's registered executables
                self.registered_apps_list
                    .emit(RegisteredAppsListMsg::UpdateExecutables(
                        self.prefix.config().registered_executables.clone(),
                    ));
            }
            AppManagerMsg::SelectExecutable(index) => {
                info!("[apps] selected executable: {}", index);
                self.set_selected_executable(Some(index));
            }
            AppManagerMsg::ConfigUpdated(config) => {
                self.prefix.set_config(config);
                self.set_prefix(self.prefix.clone());

                // Load available executables from DB (populated during refresh/sync)
                if !self.prefix.path().as_os_str().is_empty() {
                    match AppService::global()
                        .prefix_store()
                        .list_scanned_executables(&self.prefix.path().to_string_lossy())
                    {
                        Ok(exes) => {
                            self.available_executables = exes;
                        }
                        Err(e) => {
                            error!("[apps] failed to load scanned executables: {}", e);
                        }
                    }
                }

                // Update the registered apps list from cached config
                self.registered_apps_list
                    .emit(RegisteredAppsListMsg::UpdateExecutables(
                        self.prefix.config().registered_executables.clone(),
                    ));

                // Restore running highlight immediately
                let paths = service::launch::poll_dead_processes(&AppService::global());
                self.set_running_paths(paths.clone());
                self.registered_apps_list
                    .emit(RegisteredAppsListMsg::SetRunningPaths(paths));

                // Update selected running state
                if let Some(i) = self.selected_executable {
                    if let Some(exe) = self.prefix.config().registered_executables.get(i) {
                        let running = service::launch::is_process_running(
                            &AppService::global(),
                            &exe.executable_path,
                        );
                        self.app_actions
                            .emit(AppActionsMsg::SetSelectedRunning(running));
                    }
                }

                // Reset selection if the config has no executables or index is out of bounds
                if self.prefix.config().registered_executables.is_empty()
                    || self
                        .selected_executable
                        .map_or(false, |i| i >= self.prefix.config().registered_executables.len())
                {
                    self.set_selected_executable(None);
                    self.app_actions.emit(AppActionsMsg::SetSelection(false));
                }
            }
            AppManagerMsg::PrefixPathUpdated(path) => {
                let has_prefix = !path.as_os_str().is_empty();
                self.app_actions
                    .emit(AppActionsMsg::SetPrefixSet(has_prefix));
                self.prefix.set_path(path.clone());
                self.set_prefix(self.prefix.clone());
                self.registered_apps_list
                    .emit(RegisteredAppsListMsg::PrefixPathUpdated(path.clone()));
                self.add_app_popover
                    .emit(AddAppPopoverMsg::PrefixPathUpdated(path));
                self.set_selected_executable(None);
                self.app_actions.emit(AppActionsMsg::SetSelection(false));
                self.app_actions
                    .emit(AppActionsMsg::SetDesktopExists(false));
            }
            AppManagerMsg::ShowInfoDialog(index) => {
                if let Some(executable) = self.prefix.config().registered_executables.get(index) {
                    self.executable_info_dialog
                        .emit(ExecutableInfoDialogMsg::ShowInfo(
                            executable.clone(),
                            self.prefix.path().to_path_buf(),
                        ));
                }
            }
            // Handle messages from child components
            AppManagerMsg::ExecutableInfoDialog(output) => match output {
                ExecutableInfoDialogOutput::ExecutableUpdated(updated_exec) => {
                    let exe_path = updated_exec.executable_path.clone();
                    let path = self.prefix.path().to_path_buf();
                    if service::config_ops::update_executable(
                        &AppService::global(),
                        &path,
                        self.prefix.config_mut(),
                        updated_exec.clone(),
                    ) {
                        // Update desktop launcher if one exists (name/icon may have changed)
                        if prefix::desktop::desktop_launcher_exists(self.prefix.path(), &exe_path) {
                            let icon_cache = AppService::global()
                                .prefix_manager()
                                .scanner()
                                .icon_cache()
                                .clone();
                            let resolved_icon = prefix::resolve_or_extract_icon(
                                &updated_exec,
                                self.prefix.path(),
                                &icon_cache,
                            );
                            let prefix_name = self.prefix.name().to_string();
                            if let Err(e) = prefix::desktop::create_desktop_launcher(
                                self.prefix.path(),
                                &prefix_name,
                                &updated_exec.name,
                                &exe_path,
                                resolved_icon.as_deref(),
                            ) {
                                error!("[apps] failed to update desktop launcher: {}", e);
                            } else {
                                info!(
                                    "[apps] updated desktop launcher for '{}'",
                                    updated_exec.name
                                );
                                self.app_actions
                                    .emit(AppActionsMsg::SetDesktopExists(true));
                            }
                        }

                        self.registered_apps_list
                            .emit(RegisteredAppsListMsg::UpdateExecutables(
                                self.prefix.config().registered_executables.clone(),
                            ));
                        let _ = sender.output(AppManagerMsg::ConfigUpdated(self.prefix.config().clone()));
                    }
                }
            },
            AppManagerMsg::RegisteredAppsList(output) => {
                debug!("[apps] received RegisteredAppsList output: {:?}", output);
                match output {
                    RegisteredAppsListOutput::Selected(index) => {
                        debug!("[apps] setting selected executable to: {}", index);
                        self.set_selected_executable(Some(index));
                        self.app_actions.emit(AppActionsMsg::SetSelection(true));
                        // Check if the selected app is running
                        if let Some(exe) = self.prefix.config().registered_executables.get(index) {
                            let running = service::launch::is_process_running(
                                &AppService::global(),
                                &exe.executable_path,
                            );
                            self.app_actions
                                .emit(AppActionsMsg::SetSelectedRunning(running));
                            // Check desktop launcher state
                            let has_desktop = prefix::desktop::desktop_launcher_exists(
                                self.prefix.path(),
                                &exe.executable_path,
                            );
                            self.app_actions
                                .emit(AppActionsMsg::SetDesktopExists(has_desktop));
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
                info!("[apps] app actions: {:?}", output);
                match output {
                    AppActionsOutput::Launch => {
                        if let Some(index) = self.selected_executable {
                            sender.input(AppManagerMsg::LaunchExecutable(index));
                        }
                    }
                    AppActionsOutput::Kill => {
                        if let Some(index) = self.selected_executable {
                            if let Some(exe) = self.prefix.config().registered_executables.get(index) {
                                let killed = service::launch::kill_process(
                                    &AppService::global(),
                                    &exe.executable_path,
                                );
                                if killed {
                                    self.app_actions
                                        .emit(AppActionsMsg::SetSelectedRunning(false));
                                    // Refresh the list highlight
                                    let paths =
                                        service::launch::poll_dead_processes(&AppService::global());
                                    self.set_running_paths(paths.clone());
                                    self.registered_apps_list
                                        .emit(RegisteredAppsListMsg::SetRunningPaths(paths));
                                }
                            }
                        }
                    }
                    AppActionsOutput::Add => {
                        // Show popover with available executables (loaded from DB during sync)
                        self.add_app_popover
                            .emit(AddAppPopoverMsg::UpdateAvailableApps(
                                self.available_executables.clone(),
                                self.prefix.config().architecture.clone(),
                            ));

                        // Auto-scan if no cached executables
                        if self.available_executables.is_empty() {
                            info!("[apps] no cached executables found, starting auto-scan");
                            sender.input(AppManagerMsg::ScanForApplications);
                        }

                        let app_actions_widget = self.app_actions.widget();
                        if let Some(box_widget) = app_actions_widget.downcast_ref::<gtk::Box>() {
                            if let Some(first_child) = box_widget.first_child() {
                                if let Some(add_button) = first_child.downcast_ref::<gtk::Button>()
                                {
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
                    AppActionsOutput::RunUninstaller => {
                        match service::launch::launch_uninstaller(
                            &AppService::global(),
                            self.prefix.path(),
                            self.prefix.config(),
                        ) {
                            Ok(track_path) => {
                                self.uninstaller_track_path = Some(track_path);
                                self.app_actions
                                    .emit(AppActionsMsg::SetUninstallerRunning(true));
                                sender.input(AppManagerMsg::PollProcesses);
                            }
                            Err(e) => error!("[apps] failed to launch uninstaller: {}", e),
                        }
                    }
                    AppActionsOutput::RunExe => {
                        let sender_clone = sender.clone();
                        let parent_window = _root
                            .ancestor(gtk::Window::static_type())
                            .and_then(|w| w.downcast::<gtk::Window>().ok());
                        if let Some(window) = parent_window {
                            crate::dialogs::pick_file(
                                &window,
                                "Select Windows Executable",
                                &["exe"],
                                move |path| {
                                    if let Some(path) = path {
                                        sender_clone.input(AppManagerMsg::LaunchDirectExe(
                                            PathBuf::from(path),
                                        ));
                                    }
                                },
                            );
                        }
                    }
                    AppActionsOutput::CreateDesktop => {
                        if let Some(index) = self.selected_executable {
                            if let Some(exe) = self.prefix.config().registered_executables.get(index) {
                                let prefix_path = self.prefix.path().to_path_buf();
                                let prefix_name = self.prefix.name().to_string();
                                let exe_name = exe.name.clone();
                                let exe_path = exe.executable_path.clone();
                                let parent_window = _root
                                    .ancestor(gtk::Window::static_type())
                                    .and_then(|w| w.downcast::<gtk::Window>().ok());

                                // Toggle: if launcher exists, remove it; otherwise create it
                                if prefix::desktop::desktop_launcher_exists(&prefix_path, &exe_path)
                                {
                                    match prefix::desktop::remove_desktop_launcher(
                                        &prefix_path,
                                        &exe_path,
                                    ) {
                                        Ok(()) => {
                                            info!(
                                                "[apps] removed desktop launcher for '{}'",
                                                exe_name
                                            );
                                            self.app_actions
                                                .emit(AppActionsMsg::SetDesktopExists(false));
                                        }
                                        Err(e) => {
                                            error!(
                                                "[apps] failed to remove desktop launcher: {}",
                                                e
                                            );
                                        }
                                    }
                                } else {
                                    let icon_cache = AppService::global()
                                        .prefix_manager()
                                        .scanner()
                                        .icon_cache()
                                        .clone();
                                    let resolved_icon = prefix::resolve_or_extract_icon(
                                        exe,
                                        &prefix_path,
                                        &icon_cache,
                                    );

                                    match prefix::desktop::create_desktop_launcher(
                                        &prefix_path,
                                        &prefix_name,
                                        &exe_name,
                                        &exe_path,
                                        resolved_icon.as_deref(),
                                    ) {
                                        Ok(path) => {
                                            info!(
                                                "[apps] created desktop launcher: {}",
                                                path.display()
                                            );
                                            self.app_actions
                                                .emit(AppActionsMsg::SetDesktopExists(true));
                                        }
                                        Err(e) => {
                                            error!(
                                                "[apps] failed to create desktop launcher: {}",
                                                e
                                            );
                                            let alert = adw::AlertDialog::new(
                                                Some("Failed to Create Desktop Launcher"),
                                                Some(&format!("{}", e)),
                                            );
                                            alert.add_response("ok", "OK");
                                            alert.set_default_response(Some("ok"));
                                            alert.set_close_response("ok");
                                            alert.choose(
                                                parent_window.as_ref(),
                                                None::<&gtk::gio::Cancellable>,
                                                |_| {},
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            AppManagerMsg::AddAppPopover(output) => {
                debug!("[apps] received AddAppPopover output: {:?}", output);
                match output {
                    AddAppPopoverOutput::AddApp(indices) => {
                        info!("[apps] adding executables: {:?}", indices);
                        sender.input(AppManagerMsg::AddExecutables(indices));
                    }
                    AddAppPopoverOutput::Close => {
                        debug!("[apps] closing popover");
                        self.add_app_popover.widget().unparent();
                    }
                    AddAppPopoverOutput::Scan => {
                        sender.input(AppManagerMsg::ScanForApplications);
                    }
                }
            }
            AppManagerMsg::LaunchDirectExe(exe_path) => {
                match service::launch::launch_direct_exe(
                    &AppService::global(),
                    &exe_path,
                    self.prefix.path(),
                    self.prefix.config(),
                ) {
                    Ok(()) => {
                        self.external_running.insert(exe_path);
                        self.app_actions.emit(AppActionsMsg::SetExeRunning(true));
                        sender.input(AppManagerMsg::PollProcesses);
                    }
                    Err(e) => error!("[apps] failed to launch exe: {}", e),
                }
            }
            AppManagerMsg::PollProcesses => {
                let paths = service::launch::poll_dead_processes(&AppService::global());
                self.set_running_paths(paths.clone());
                self.registered_apps_list
                    .emit(RegisteredAppsListMsg::SetRunningPaths(paths));
                // Update selected running state
                if let Some(i) = self.selected_executable {
                    if let Some(exe) = self.prefix.config().registered_executables.get(i) {
                        let running = service::launch::is_process_running(
                            &AppService::global(),
                            &exe.executable_path,
                        );
                        self.app_actions
                            .emit(AppActionsMsg::SetSelectedRunning(running));
                    }
                }
                // Update uninstaller running state
                let uninstaller_still_running = self
                    .uninstaller_track_path
                    .as_ref()
                    .map(|p| service::launch::is_process_running(&AppService::global(), p))
                    .unwrap_or(false);
                if !uninstaller_still_running {
                    self.uninstaller_track_path = None;
                }
                self.app_actions.emit(AppActionsMsg::SetUninstallerRunning(
                    uninstaller_still_running,
                ));
                // Update external (directly-run) exe running state
                self.external_running.retain(|path| {
                    service::launch::is_process_running(&AppService::global(), path)
                });
                self.app_actions.emit(AppActionsMsg::SetExeRunning(
                    !self.external_running.is_empty(),
                ));
            }
        }
    }
}
