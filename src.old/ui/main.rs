use gtk::prelude::*;
use gtk4::gio;
use gtk::glib;
use relm4::view;
use relm4::{ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt, SimpleComponent, Component, gtk, adw, component::AsyncComponentController};
use relm4::prelude::{AsyncController, AsyncComponent};
use std::path::PathBuf;
use std::sync::Arc;
use tracker;

use crate::prefix::{Manager as PrefixManager, WinePrefix};
use crate::prefix::runtime::RuntimeManager;
use super::{PrefixListModel, PrefixDetailsModel, AppManagerModel, RegistryEditorModel, RuntimeManagerModel};
use gtk::gdk;

#[tracker::track]
pub struct AppModel {
    pub prefixes: Vec<WinePrefix>,
    pub prefix_manager: PrefixManager,
    pub selected_prefix: Option<usize>,
    #[tracker::do_not_track]
    pub prefix_list: Controller<PrefixListModel>,
    #[tracker::do_not_track]
    pub prefix_details: Controller<PrefixDetailsModel>,
    #[tracker::do_not_track]
    pub app_manager: AsyncController<AppManagerModel>,
    #[tracker::do_not_track]
    pub registry_editor: Controller<RegistryEditorModel>,
    #[tracker::do_not_track]
    content_stack: adw::ViewStack,
    #[tracker::do_not_track]
    content_box: gtk::Stack,
    #[tracker::do_not_track]
    pub flap: adw::Flap,
    #[tracker::do_not_track]
    pub switcher: adw::ViewSwitcherTitle,
    #[tracker::do_not_track]
    pub prefix_store: Arc<crate::prefix::PrefixStore>,
    pub syncing: bool,
    pub sidebar_visible: bool,
    #[tracker::do_not_track]
    main_window: gtk::ApplicationWindow,
    #[tracker::do_not_track]
    sync_overlay: gtk::CenterBox,
    #[tracker::do_not_track]
    sync_progress_bar: gtk::ProgressBar,
    #[tracker::do_not_track]
    sync_progress_label: gtk::Label,
    #[tracker::do_not_track]
    runtime_manager: relm4::prelude::AsyncController<RuntimeManagerModel>,
}

#[derive(Debug)]
pub enum AppMsg {
    CreatePrefix,
    DeletePrefix(usize),
    LaunchPrefix(usize),
    LaunchExecutable(usize, usize), // prefix index, executable index
    RefreshPrefixes,
    SelectPrefix(usize),
    ShowPrefixDetails(usize),
    // ShowAppManager(usize),
    HideDetails,
    ConfigUpdated(usize, crate::prefix::config::PrefixConfig),
    ScanForApplications(usize),
    ShowCreatePrefixDialog,
    SyncComplete(Vec<WinePrefix>),
    SyncPrefixes,
    ReloadPrefixes(Vec<WinePrefix>),
    SyncProgress(usize, usize),
    ToggleSidebar,
    ShowRuntimeManager,
    RuntimesUpdated(RuntimeManager),
}

impl AppModel {
    pub fn scan_wine_prefixes(prefix_manager: &PrefixManager) -> Vec<WinePrefix> {
        match prefix_manager.scan_prefixes() {
            Ok(prefixes) => prefixes,
            Err(e) => {
                eprintln!("Error scanning prefixes: {}", e);
                Vec::new()
            }
        }
    }
}

#[relm4::component(pub)]
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

            set_titlebar: Some(&header_bar),

            #[local_ref]
            overlay_widget -> gtk::Widget {}
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sender_clone = sender.clone();

        // Build header bar early
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        let sidebar_btn = gtk::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Show Sidebar")
            .build();
        let sb_sender = sender.clone();
        sidebar_btn.connect_clicked(move |_| { sb_sender.input(AppMsg::ToggleSidebar); });
        header_bar.pack_start(&sidebar_btn);

        let new_prefix_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("New Prefix")
            .build();
        let np_sender = sender.clone();
        new_prefix_btn.connect_clicked(move |_| { np_sender.input(AppMsg::CreatePrefix); });
        header_bar.pack_end(&new_prefix_btn);

        let settings_btn = gtk::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Runtime Settings")
            .build();
        let st_sender = sender.clone();
        settings_btn.connect_clicked(move |_| { st_sender.input(AppMsg::ShowRuntimeManager); });
        header_bar.pack_end(&settings_btn);

        let switcher = adw::ViewSwitcherTitle::new();
        switcher.set_title("Tequila");
        switcher.set_sensitive(false);
        header_bar.set_title_widget(Some(&switcher));

        let wine_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Wine");

        let icon_cache = Arc::new(
            crate::prefix::IconCache::open(
                dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("tequila/icons"),
            ).expect("Failed to open icon cache"),
        );

        // Persistent state store
        let state_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("tequila/state.db");
        let prefix_store = Arc::new(
            crate::prefix::PrefixStore::open(&state_path)
                .expect("Failed to open state store"),
        );

        let prefix_manager = PrefixManager::new(wine_dir.clone(), Arc::clone(&icon_cache));

        // Load prefixes from filesystem + JSON config files (fast, user-editable)
        let prefixes = AppModel::scan_wine_prefixes(&prefix_manager);
        // Trigger background scan if no cached scan results exist yet
        let needs_sync = !prefixes.is_empty() && prefixes.iter().all(|p| {
            !prefix_store.has_scanned_prefix(&p.path.to_string_lossy())
        });
        println!("Loaded {} prefixes", prefixes.len());

        let prefix_list = PrefixListModel::builder()
            .launch((prefixes.clone(), None))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::prefix_list::PrefixListOutput::SelectPrefix(index) => AppMsg::SelectPrefix(index),
                crate::ui::prefix_list::PrefixListOutput::DeselectPrefix => AppMsg::HideDetails,
                crate::ui::prefix_list::PrefixListOutput::DeletePrefix(index) => AppMsg::DeletePrefix(index),
            });

        let prefix_details = PrefixDetailsModel::builder()
            .launch((PathBuf::new(), crate::prefix::config::PrefixConfig::new("".to_string(), "win64".to_string())))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes
            });

        let app_manager = AppManagerModel::builder()
            .launch((PathBuf::new(), crate::prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()), Arc::clone(&icon_cache), Arc::clone(&prefix_store)))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::app_manager::AppManagerMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes
            });

        let registry_editor = RegistryEditorModel::builder()
            .launch((PathBuf::new(), crate::prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()), Arc::clone(&prefix_store)))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::registry_editor::RegistryEditorMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes
            });

        let runtime_manager = RuntimeManagerModel::builder()
            .launch(prefix_manager.clone())
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::runtime_manager::RuntimeManagerOutput::RuntimesUpdated(rm) => {
                    AppMsg::RuntimesUpdated(rm)
                }
            });

        let prefix_list_widget = prefix_list.widget().clone().upcast::<gtk::Widget>();

        // Empty state page
        let empty_page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center).valign(gtk::Align::Center).vexpand(true).build();
        empty_page.append(&gtk::Image::builder().pixel_size(72)
            .icon_name("brand-winehq-symbolic").css_classes(["dim-label"]).build());
        empty_page.append(&gtk::Label::builder().label("No prefix selected")
            .css_classes(["title-4", "dim-label"]).margin_top(10).build());

        // Tabbed content Stack
        let content_stack = adw::ViewStack::new();
        content_stack.add_titled(app_manager.widget(), Some("apps"), "Apps")
            .set_icon_name(Some("application-x-executable-symbolic"));
        content_stack.add_titled(prefix_details.widget(), Some("details"), "Details")
            .set_icon_name(Some("document-properties-symbolic"));
        content_stack.add_titled(registry_editor.widget(), Some("registry"), "Registry")
            .set_icon_name(Some("preferences-system-symbolic"));
        switcher.set_stack(Some(&content_stack));

        // Wrapper Stack: show either empty page or tabbed content
        let content_box = gtk::Stack::builder()
            .hexpand(true).vexpand(true)
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        content_box.add_named(&empty_page, Some("empty"));
        content_box.add_named(&content_stack, Some("tabs"));
        content_box.set_visible_child_name("empty");

        // Build Flap (native collapsible sidebar)
        let flap = adw::Flap::builder()
            .reveal_flap(true)
            .fold_policy(adw::FlapFoldPolicy::Never)
            .transition_type(adw::FlapTransitionType::Slide)
            .build();
        flap.set_flap(Some(&prefix_list_widget));
        flap.set_content(Some(&content_box));
        prefix_list_widget.set_width_request(240);

        let flap_widget = flap.clone().upcast::<gtk::Widget>();

        // Sync progress overlay
        let sync_progress_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .spacing(10)
            .css_classes(["sync-progress-box"])
            .build();
        let sync_spinner = gtk::Spinner::builder()
            .spinning(true)
            .width_request(36)
            .height_request(36)
            .build();
        sync_progress_box.append(&sync_spinner);
        let sync_progress_bar = gtk::ProgressBar::builder()
            .width_request(260)
            .build();
        sync_progress_box.append(&sync_progress_bar);
        let sync_progress_label = gtk::Label::builder()
            .css_classes(["caption", "dim-label"])
            .label("")
            .build();
        sync_progress_box.append(&sync_progress_label);

        let sync_overlay_box = gtk::CenterBox::builder()
            .hexpand(true)
            .vexpand(true)
            .css_classes(["sync-overlay-bg"])
            .visible(false)
            .build();
        sync_overlay_box.set_center_widget(Some(&sync_progress_box));

        let sync_overlay = gtk::Overlay::new();
        sync_overlay.set_child(Some(&flap_widget));
        sync_overlay.add_overlay(&sync_overlay_box);
        if needs_sync {
            sync_overlay_box.set_visible(true);
            sync_progress_label.set_label("Scanning...");
        }
        // Apply sync overlay CSS
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(".sync-overlay-bg { background: rgba(0, 0, 0, 0.45); } \
                                     .sync-progress-box { background: @view_bg_color; border: 1px solid @borders; border-radius: 12px; padding: 24px; } \
                                     .icon-bg { background: #eee; border-radius: 24px; padding: 12px; } \
                                     .desc-text { padding: 8px; } \
                                     .app-item { border: 2px solid transparent; border-radius: 8px; } \
                                     .app-item.running { border-color: @accent_color; }");
            gtk::style_context_add_provider_for_display(
                &gdk::Display::default().unwrap(),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        // macOS: remove rounded window corners, macOS would do that
        #[cfg(target_os = "macos")]
        {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(
                "window, .background, .titlebar, headerbar, .window-frame { border-radius: 0px; }",
            );
            gtk::style_context_add_provider_for_display(
                &gdk::Display::default().unwrap(),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        let overlay_widget = sync_overlay.clone().upcast::<gtk::Widget>();

        let model = AppModel {
            prefixes,
            prefix_manager,
            selected_prefix: None,
            prefix_list,
            prefix_details,
            app_manager,
            registry_editor,
            runtime_manager,
            content_stack,
            content_box,
            flap,
            switcher,
            prefix_store,
            syncing: false,
            sidebar_visible: true,
            main_window: root.clone(),
            sync_overlay: sync_overlay_box,
            sync_progress_bar,
            sync_progress_label,
            tracker: 0,
        };

        let widgets = view_output!();

        // Auto-select first prefix and trigger background scan if cold start
        if !model.prefixes.is_empty() {
            sender_clone.input(AppMsg::ShowPrefixDetails(0));
        }
        if needs_sync {
            let bg_sender = sender.clone();
            glib::spawn_future_local(async move {
                bg_sender.input(AppMsg::SyncPrefixes);
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ShowCreatePrefixDialog => {
                let dialog = gtk::Dialog::builder()
                    .title("Create New Wine Prefix")
                    .modal(true)
                    .build();

                #[cfg(not(target_os = "macos"))]
                dialog.set_titlebar(&gtk::HeaderBar::new());

                dialog.set_transient_for(Some(&self.main_window));
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

                let name_label = gtk::Label::builder()
                    .label("Prefix Name:")
                    .halign(gtk::Align::Start)
                    .build();
                let name_entry = gtk::Entry::builder()
                    .placeholder_text("Enter prefix name")
                    .hexpand(true)
                    .width_chars(32)
                    .build();

                let arch_label = gtk::Label::builder()
                    .label("Architecture:")
                    .halign(gtk::Align::Start)
                    .build();
                let arch_combo = gtk::ComboBoxText::builder()
                    .hexpand(true)
                    .build();
                arch_combo.append_text("win32");
                arch_combo.append_text("win64");
                arch_combo.set_active(Some(1));

                // Runtime selector
                let runtime_label = gtk::Label::builder()
                    .label("Wine Runtime:")
                    .halign(gtk::Align::Start)
                    .build();
                let runtime_combo = gtk::ComboBoxText::builder()
                    .hexpand(true)
                    .build();
                {
                    let rm = self.prefix_manager.runtime_manager();
                    let default_id = &rm.default_id;
                    let mut default_idx = 0u32;
                    for (i, rt) in rm.runtimes.iter().enumerate() {
                        runtime_combo.append_text(&format!("{} ({})", rt.name, rt.wine_version));
                        if &rt.id == default_id {
                            default_idx = i as u32;
                        }
                    }
                    if !rm.runtimes.is_empty() {
                        runtime_combo.set_active(Some(default_idx));
                    }
                }

                // Progress bar for prefix creation (hidden until Create is clicked)
                let progress_bar = gtk::ProgressBar::builder()
                    .visible(false)
                    .pulse_step(0.1)
                    .build();
                let progress_label = gtk::Label::builder()
                    .label("Creating Wine prefix...")
                    .visible(false)
                    .build();

                content_box.append(&name_label);
                content_box.append(&name_entry);
                content_box.append(&arch_label);
                content_box.append(&arch_combo);
                content_box.append(&runtime_label);
                content_box.append(&runtime_combo);
                content_box.append(&progress_label);
                content_box.append(&progress_bar);

                content_area.append(&content_box);
                dialog.present();

                let prefix_manager = self.prefix_manager.clone();
                let main_window = self.main_window.clone();
                let sender_clone = sender.clone();
                dialog.connect_response(move |dialog, response| {
                    if response != gtk::ResponseType::Ok {
                        dialog.close();
                        return;
                    }

                    let name = name_entry.text().to_string();
                    if name.is_empty() {
                        eprintln!("Prefix name cannot be empty");
                        return;
                    }

                    let architecture = arch_combo.active_text()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "win64".to_string());

                    // Get selected runtime id
                    let runtime_id = runtime_combo.active()
                        .and_then(|i| {
                            let rm = prefix_manager.runtime_manager();
                            rm.runtimes.get(i as usize).map(|r| r.id.clone())
                        })
                        .unwrap_or_else(|| prefix_manager.runtime_manager().default_id.clone());

                    // Show progress, disable inputs
                    name_entry.set_sensitive(false);
                    arch_combo.set_sensitive(false);
                    runtime_combo.set_sensitive(false);
                    progress_label.set_visible(true);
                    progress_bar.set_visible(true);
                    progress_bar.pulse();
                    dialog.set_response_sensitive(gtk::ResponseType::Ok, false);
                    dialog.set_response_sensitive(gtk::ResponseType::Cancel, false);

                    let pb = progress_bar.clone();
                    let pulse_id = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                        pb.pulse();
                        glib::ControlFlow::Continue
                    });

                    let prefix_name = name.clone();
                    let pm = prefix_manager.clone();
                    let sc = sender_clone.clone();
                    let dlg = dialog.clone();
                    let mw = main_window.clone();
                    let ctx = glib::MainContext::default();
                    ctx.spawn_local(async move {
                        let n = prefix_name.clone();
                        let a = architecture.clone();
                        let rid = runtime_id.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            pm.create_prefix_with_runtime(&n, &a, &rid)
                        }).await;

                        // Back on the main thread
                        pulse_id.remove();
                        dlg.close();
                        let err_msg: Option<String> = match result {
                            Ok(Ok(prefix_path)) => {
                                println!("Created prefix: {} at {}", prefix_name, prefix_path.display());
                                sc.input(AppMsg::RefreshPrefixes);
                                return;
                            }
                            Ok(Err(e)) => Some(format!("{}", e)),
                            Err(e) => Some(if e.is_panic() {
                                "panic in create_prefix".to_string()
                            } else {
                                format!("{}", e)
                            }),
                        };

                        if let Some(msg) = err_msg {
                            eprintln!("Failed to create prefix '{}': {}", prefix_name, msg);
                            let err_dlg = gtk::Dialog::builder()
                                .title("Error")
                                .modal(true)
                                .build();
                            err_dlg.set_transient_for(Some(&mw));
                            #[cfg(not(target_os = "macos"))]
                            err_dlg.set_titlebar(&gtk::HeaderBar::new());
                            err_dlg.content_area().append(&gtk::Label::builder()
                                .label(&format!("Failed to create prefix '{}': {}", prefix_name, msg))
                                .build());
                            err_dlg.add_button("OK", gtk::ResponseType::Ok);
                            err_dlg.connect_response(|d, _| d.close());
                            err_dlg.present();
                        }
                    });
                });
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
                let sm = self.prefix_manager.clone();
                let s = sender.clone();
                std::thread::spawn(move || {
                    let fresh = AppModel::scan_wine_prefixes(&sm);
                    s.input(AppMsg::ReloadPrefixes(fresh));
                });
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
                    self.switcher.set_sensitive(true);
                    self.content_box.set_visible_child_name("tabs");
                    self.content_stack.set_visible_child_name("apps");

                    // Update the details component
                    let config = self.prefixes[index].config.clone();
                    let prefix_path = self.prefixes[index].path.clone();

                    // Emit path first so ConfigUpdated handlers have the correct prefix path
                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::PrefixPathUpdated(prefix_path.clone()));
                    self.app_manager.emit(crate::ui::app_manager::AppManagerMsg::PrefixPathUpdated(prefix_path.clone()));
                    self.registry_editor.emit(crate::ui::registry_editor::RegistryEditorMsg::PrefixPathUpdated(prefix_path));

                    // Resolve runtime display name
                    let runtime_display = config.wine_version.as_ref()
                        .and_then(|id| self.prefix_manager.runtime_manager().get(id))
                        .map(|r| format!("{} ({})", r.name, r.wine_version))
                        .unwrap_or_else(|| config.wine_version.as_deref().unwrap_or("Unknown").to_string());
                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::SetWineVersionDisplay(runtime_display));

                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config.clone()));
                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::SetPrefixIndex(index));
                    self.app_manager.emit(crate::ui::app_manager::AppManagerMsg::ConfigUpdated(config.clone()));
                    self.registry_editor.emit(crate::ui::registry_editor::RegistryEditorMsg::ConfigUpdated(config.clone()));
                }
            }
            AppMsg::HideDetails => {
                self.switcher.set_sensitive(false);
                self.content_box.set_visible_child_name("empty");
            }
            AppMsg::ConfigUpdated(index, config) => {
                // Handle config updates from both app_manager and prefix_details
                if let Some(selected_index) = self.selected_prefix {
                    let actual_index = if index == 0 { selected_index } else { index };

                    if actual_index < self.prefixes.len() {
                        let prefix_path = &self.prefixes[actual_index].path;

                        // Save config to file and state store
                        if let Err(e) = self.prefix_manager.update_config(prefix_path, &config) {
                            eprintln!("Failed to update config: {}", e);
                        } else {
                            self.prefixes[actual_index].config = config.clone();

                            // Update other components with the new config but don't refresh the entire list
                            self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config.clone()));
                            self.app_manager.emit(crate::ui::app_manager::AppManagerMsg::ConfigUpdated(config.clone()));
                            self.registry_editor.emit(crate::ui::registry_editor::RegistryEditorMsg::ConfigUpdated(config.clone()));
                        }
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
            AppMsg::SyncComplete(fresh) => {
                self.set_syncing(false);
                self.sync_overlay.set_visible(false);
                self.prefixes = fresh.clone();
                self.prefix_list.emit(crate::ui::prefix_list::PrefixListMsg::SetPrefixes(fresh));
                if !self.prefixes.is_empty() {
                    sender.input(AppMsg::ShowPrefixDetails(0));
                }
            }
            AppMsg::ReloadPrefixes(fresh) => {
                // Light reload: update the prefix list without app scanning or auto-select
                self.prefixes = fresh.clone();
                self.prefix_list.emit(crate::ui::prefix_list::PrefixListMsg::SetPrefixes(fresh));
            }
            AppMsg::SyncPrefixes => {
                if !self.syncing {
                    self.set_syncing(true);
                    self.sync_overlay.set_visible(true);
                    self.sync_progress_bar.set_fraction(0.0);
                    self.sync_progress_label.set_label("Scanning...");
                    let ss = sender.clone();
                    let sp = sender.clone();
                    let sm = self.prefix_manager.clone();
                    let st = Arc::clone(&self.prefix_store);
                    std::thread::spawn(move || {
                        let mut fresh = AppModel::scan_wine_prefixes(&sm);
                        let total = fresh.len();
                        for (i, p) in fresh.iter_mut().enumerate() {
                            if let Ok(exes) = sm.scan_for_applications(&p.path) {
                                let _ = st.save_scanned_executables(&p.path.to_string_lossy(), &exes);
                            }
                            let changed = sm.enrich_executables(&mut p.config);
                            if changed {
                                let _ = sm.update_config(&p.path, &p.config);
                            }
                            let _ = sp.input(AppMsg::SyncProgress(i + 1, total));
                        }
                        let _ = ss.input(AppMsg::SyncComplete(fresh));
                    });
                }
            }
            AppMsg::SyncProgress(completed, total) => {
                self.sync_progress_bar.set_fraction(if total > 0 { completed as f64 / total as f64 } else { 0.0 });
                self.sync_progress_label.set_label(&format!("{} / {} prefixes", completed, total));
            }
            AppMsg::ToggleSidebar => {
                let visible = !self.sidebar_visible;
                self.set_sidebar_visible(visible);
                self.flap.set_reveal_flap(visible);
            }
            AppMsg::ShowRuntimeManager => {
                self.runtime_manager.widget().present();
            }
            AppMsg::RuntimesUpdated(rm) => {
                // Sync the updated RuntimeManager into our PrefixManager
                let pm_rm = self.prefix_manager.runtime_manager_mut();
                let _old = std::mem::replace(pm_rm, rm);
                self.prefix_manager.save_runtime_state();
            }
        }

        // Update the view based on current state will be handled by Relm4 automatically
    }
}

pub fn initialize_custom_icons() {
    gio::resources_register_include!("icons.gresource").unwrap();

    let display = gdk::Display::default().unwrap();
    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path("/com/anson2251/tequila/icons");
}
