use gtk::prelude::*;
use gtk4::gio;
use gtk::glib;
use adw::prelude::*;
use relm4::{ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent, Component, gtk, adw, component::AsyncComponentController};
use relm4::prelude::{AsyncController, AsyncComponent};
use std::path::PathBuf;
use std::sync::Arc;
use tracker;

use prefix::{Manager as PrefixManager, ProcessTracker, WinePrefix};
use prefix::runtime::RuntimeManager;
use super::{PrefixListModel, PrefixConfigModel, AppManagerModel, SettingsWindow};
use gtk::gdk;

#[tracker::track]
pub struct AppModel {
    pub prefixes: Vec<WinePrefix>,
    pub prefix_manager: PrefixManager,
    pub selected_prefix: Option<usize>,
    #[tracker::do_not_track]
    pub prefix_list: Controller<PrefixListModel>,
    #[tracker::do_not_track]
    pub prefix_config: Controller<PrefixConfigModel>,
    #[tracker::do_not_track]
    pub app_manager: AsyncController<AppManagerModel>,
    #[tracker::do_not_track]
    content_stack: adw::ViewStack,
    #[tracker::do_not_track]
    content_box: gtk::Stack,
    #[tracker::do_not_track]
    pub flap: adw::OverlaySplitView,
    #[tracker::do_not_track]
    pub switcher: adw::ViewSwitcher,
    #[tracker::do_not_track]
    pub prefix_store: Arc<prefix::PrefixStore>,
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
    settings: relm4::prelude::AsyncController<SettingsWindow>,
    #[tracker::do_not_track]
    create_prefix_dialog: Option<relm4::Controller<crate::prefix::create_dialog::CreatePrefixDialog>>,
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
    ConfigUpdated(usize, prefix::config::PrefixConfig),
    ScanForApplications(usize),
    ShowCreatePrefixDialog,
    SyncComplete(Vec<WinePrefix>),
    SyncPrefixes,
    ReloadPrefixes(Vec<WinePrefix>),
    SyncProgress(usize, usize),
    ToggleSidebar,
    ShowSettings,
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
        let _sender_clone = sender.clone();

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

        let back_btn = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back")
            .visible(false)
            .build();
        header_bar.pack_start(&back_btn);

        let new_prefix_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("New Prefix")
            .build();
        let np_sender = sender.clone();
        new_prefix_btn.connect_clicked(move |_| { np_sender.input(AppMsg::CreatePrefix); });
        header_bar.pack_end(&new_prefix_btn);

        let settings_btn = gtk::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Settings")
            .build();
        let st_sender = sender.clone();
        settings_btn.connect_clicked(move |_| { st_sender.input(AppMsg::ShowSettings); });
        header_bar.pack_end(&settings_btn);

        let switcher = adw::ViewSwitcher::builder()
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        switcher.set_sensitive(false);
        header_bar.set_title_widget(Some(&switcher));

        let wine_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Wine");

        let process_tracker = ProcessTracker::shared();

        let icon_cache = Arc::new(
            prefix::IconCache::open(
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
            prefix::PrefixStore::open(&state_path)
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
                crate::prefix_list::PrefixListOutput::SelectPrefix(index) => AppMsg::SelectPrefix(index),
                crate::prefix_list::PrefixListOutput::DeselectPrefix => AppMsg::HideDetails,
                crate::prefix_list::PrefixListOutput::DeletePrefix(index) => AppMsg::DeletePrefix(index),
            });

        let config_tab = PrefixConfigModel::builder()
            .launch((PathBuf::new(), prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()), Arc::clone(&prefix_store), Arc::clone(&process_tracker), back_btn))
            .forward(sender.input_sender(), |msg| match msg {
                crate::prefix_config::PrefixConfigOutput::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
            });

        let app_manager = AppManagerModel::builder()
            .launch((PathBuf::new(), prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()), Arc::clone(&icon_cache), Arc::clone(&prefix_store), Arc::clone(&process_tracker)))
            .forward(sender.input_sender(), |msg| match msg {
                crate::app_manager::AppManagerMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes
            });

        let settings = SettingsWindow::builder()
            .launch(prefix_manager.clone())
            .forward(sender.input_sender(), |msg| match msg {
                crate::settings::SettingsOutput::RuntimesUpdated(rm) => {
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
        content_stack.add_titled(config_tab.widget(), Some("config"), "Config")
            .set_icon_name(Some("document-properties-symbolic"));
        switcher.set_stack(Some(&content_stack));

        // Wrapper Stack: show either empty page or tabbed content
        let content_box = gtk::Stack::builder()
            .hexpand(true).vexpand(true)
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        content_box.add_named(&empty_page, Some("empty"));
        content_box.add_named(&content_stack, Some("tabs"));
        content_box.set_visible_child_name("empty");

        // Build sidebar using OverlaySplitView (replaces deprecated Flap)
        let flap = adw::OverlaySplitView::builder()
            .sidebar(&prefix_list_widget)
            .content(&content_box)
            .show_sidebar(true)
            .build();
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

        // macOS: prevent fullscreen by setting collectionBehavior on the native NSWindow.
        // Uses connect_realize instead of notify::surface because the latter
        // may not fire reliably during initial window setup.
        #[cfg(target_os = "macos")]
        root.connect_realize(move |window| {
            if let Some(surface) = window.surface() {
                if let Some(macos_surface) = surface.downcast_ref::<gdk4_macos::MacosSurface>() {
                    let ns_ptr = macos_surface.native();
                    let ns_window: &objc2_app_kit::NSWindow =
                        unsafe { &*(ns_ptr as *const objc2_app_kit::NSWindow) };
                    ns_window.setCollectionBehavior(
                        objc2_app_kit::NSWindowCollectionBehavior::FullScreenNone,
                    );
                }
            }
        });

        let overlay_widget = sync_overlay.clone().upcast::<gtk::Widget>();

        let model = AppModel {
            prefixes,
            prefix_manager,
            selected_prefix: None,
            prefix_list,
            prefix_config: config_tab,
            app_manager,
            settings,
            create_prefix_dialog: None,
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

        // Defer menu bar setup to ensure the application is fully initialized
        // (on macOS, root.application() may not be set during init)
        let root_clone = root.clone();
        let s = sender.clone();
        glib::idle_add_local(move || {
            let gtk_app = root_clone.application().or_else(|| {
                let app = gio::Application::default()?;
                app.downcast::<gtk::Application>().ok()
            });

            if let Some(app) = gtk_app {
                setup_menu_bar(app, s.clone());
            }
            glib::ControlFlow::Break
        });

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
                let dialog = crate::prefix::create_dialog::CreatePrefixDialog::builder()
                    .launch((self.prefix_manager.clone(), self.main_window.clone()))
                    .forward(sender.input_sender(), |msg| msg);
                self.create_prefix_dialog = Some(dialog);
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
                    self.prefix_config.emit(crate::prefix_config::PrefixConfigMsg::PrefixPathUpdated(prefix_path.clone()));
                    self.app_manager.emit(crate::app_manager::AppManagerMsg::PrefixPathUpdated(prefix_path));

                    // Resolve runtime display name
                    let runtime_display = config.wine_version.as_ref()
                        .and_then(|id| self.prefix_manager.runtime_manager().get(id))
                        .map(|r| format!("{} ({})", r.name, r.wine_version))
                        .unwrap_or_else(|| config.wine_version.as_deref().unwrap_or("Unknown").to_string());
                    self.prefix_config.emit(crate::prefix_config::PrefixConfigMsg::SetWineVersionDisplay(runtime_display));

                    self.prefix_config.emit(crate::prefix_config::PrefixConfigMsg::ConfigUpdated(config.clone()));
                    self.prefix_config.emit(crate::prefix_config::PrefixConfigMsg::SetPrefixIndex(index));
                    self.app_manager.emit(crate::app_manager::AppManagerMsg::ConfigUpdated(config.clone()));
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
                            self.prefix_config.emit(crate::prefix_config::PrefixConfigMsg::ConfigUpdated(config.clone()));
                            self.app_manager.emit(crate::app_manager::AppManagerMsg::ConfigUpdated(config.clone()));
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
                self.prefix_list.emit(crate::prefix_list::PrefixListMsg::SetPrefixes(fresh));
            }
            AppMsg::ReloadPrefixes(fresh) => {
                // Light reload: update the prefix list without app scanning or auto-select
                self.prefixes = fresh.clone();
                self.prefix_list.emit(crate::prefix_list::PrefixListMsg::SetPrefixes(fresh));
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
                self.flap.set_show_sidebar(visible);
            }
            AppMsg::ShowSettings => {
                self.settings.widget().present();
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

pub fn initialize_custom_resources() {
    gio::resources_register_include!("icons.gresource").unwrap();
    gio::resources_register_include!("css.gresource").unwrap();

    let display = gdk::Display::default().unwrap();
    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path("/com/anson2251/tequila/icons");

    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/com/anson2251/tequila/css/style.css");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Configure the application menu bar with platform-appropriate menus.
///
/// On macOS, creates a native NSMenu bar with AppKit APIs.
/// On Linux, uses GTK's gio::Menu with set_menubar.
pub fn setup_menu_bar(app: gtk::Application, sender: ComponentSender<AppModel>) {
    // Register shared actions so keyboard shortcuts work regardless of menu system
    register_menu_actions(&app, &sender);

    #[cfg(target_os = "macos")]
    setup_macos_native_menu(&app, sender);

    #[cfg(not(target_os = "macos"))]
    {
        use gtk::gio::Menu;

        let menubar = Menu::new();

        let file_menu = Menu::new();
        file_menu.append(Some("_New Prefix"), Some("app.new-prefix"));
        file_menu.append(Some("_Preferences"), Some("app.preferences"));
        file_menu.append(Some("_Quit"), Some("app.quit"));
        menubar.append_submenu(Some("_File"), &file_menu);

        let view_menu = Menu::new();
        view_menu.append(Some("Toggle _Sidebar"), Some("app.toggle-sidebar"));
        menubar.append_submenu(Some("_View"), &view_menu);

        app.set_menubar(Some(&menubar));
    }
}

/// Register GIO actions so keyboard shortcuts like Cmd+N, Cmd+, etc. work.
/// These actions are used by both the gio::Menu (Linux) and native NSMenu (macOS).
fn register_menu_actions(app: &gtk::Application, sender: &ComponentSender<AppModel>) {
    use gtk::gio::SimpleAction;

    let new_prefix_action = SimpleAction::new("new-prefix", None);
    let s = sender.clone();
    new_prefix_action.connect_activate(move |_, _| {
        s.input(AppMsg::ShowCreatePrefixDialog);
    });
    app.add_action(&new_prefix_action);
    app.set_accels_for_action("app.new-prefix", &["<primary>n"]);

    let preferences_action = SimpleAction::new("preferences", None);
    let s = sender.clone();
    preferences_action.connect_activate(move |_, _| {
        s.input(AppMsg::ShowSettings);
    });
    app.add_action(&preferences_action);
    app.set_accels_for_action("app.preferences", &["<primary>comma"]);

    let toggle_sidebar_action = SimpleAction::new("toggle-sidebar", None);
    let s = sender.clone();
    toggle_sidebar_action.connect_activate(move |_, _| {
        s.input(AppMsg::ToggleSidebar);
    });
    app.add_action(&toggle_sidebar_action);
    app.set_accels_for_action("app.toggle-sidebar", &["<primary>backslash"]);

    let app_quit = app.clone();
    let quit_action = SimpleAction::new("quit", None);
    quit_action.connect_activate(move |_, _| {
        app_quit.quit();
    });
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<primary>q"]);
}

// ── macOS native menu (NSMenu / NSMenuItem) ──────────────────────────────

#[cfg(target_os = "macos")]
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
static MENU_CALLBACK: OnceLock<Box<dyn Fn(AppMsg) + Send + Sync>> = OnceLock::new();

// Must be kept alive for the lifetime of the app — menu items hold a reference to this target.
#[cfg(target_os = "macos")]
static MENU_TARGET: OnceLock<objc2::rc::Retained<TequilaMenuHandler>> = OnceLock::new();

#[cfg(target_os = "macos")]
// Objective-C class that acts as the target for native NSMenuItem actions.
// Uses a global callback to dispatch AppMsg values to the component.
objc2::define_class!(
    #[unsafe(super(objc2::runtime::NSObject))]
    #[name = "TequilaMenuHandler"]
    struct TequilaMenuHandler;

    impl TequilaMenuHandler {
        #[unsafe(method(handleMenuAction:))]
        fn handle_menu_action(&self, sender: &objc2::runtime::NSObject) {
            use objc2::msg_send;
            let tag: isize = unsafe { msg_send![sender, tag] };
            match tag {
                1 => {
                    let about = adw::AboutDialog::new();
                    about.set_application_name("Tequila");
                    about.set_application_icon("com.github.anson2251.tequila");
                    about.set_version("0.1.0");
                    about.set_comments("Wine Prefix Manager");
                    about.set_developer_name("Anson2251");
                    let parent = gio::Application::default()
                        .and_then(|a| a.downcast::<gtk::Application>().ok())
                        .and_then(|app| app.active_window());
                    about.present(parent.as_ref());
                }
                2 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ShowCreatePrefixDialog);
                    }
                }
                3 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ShowSettings);
                    }
                }
                4 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ToggleSidebar);
                    }
                }
                5 => {
                    if let Some(gio_app) = gio::Application::default() {
                        if let Ok(gtk_app) = gio_app.downcast::<gtk::Application>() {
                            gtk_app.quit();
                        }
                    }
                }
                _ => {}
            }
        }
    }
);

#[cfg(target_os = "macos")]
impl TequilaMenuHandler {
    objc2::extern_methods!(
        #[unsafe(method(new))]
        fn new() -> objc2::rc::Retained<Self>;
    );
}

/// Create a native macOS menu bar using NSMenu / NSMenuItem.
#[cfg(target_os = "macos")]
fn setup_macos_native_menu(_app: &gtk::Application, sender: ComponentSender<AppModel>) {
    use objc2::runtime::NSObject;
    use objc2::{MainThreadMarker, sel};
    use objc2_foundation::NSString;
    use objc2_app_kit::{NSApp, NSMenu, NSMenuItem, NSEventModifierFlags};

    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    // Set up a channel: native menu callbacks send AppMsg through this,
    // and a glib timeout on the main thread polls it and forwards to the component.
    let s = sender.clone();
    let (tx, rx) = std::sync::mpsc::channel::<AppMsg>();
    MENU_CALLBACK
        .set(Box::new(move |msg| {
            let _ = tx.send(msg);
        }))
        .ok();

    // Poll the channel every 50ms on the GTK main loop
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(msg) = rx.try_recv() {
            s.input(msg);
        }
        glib::ControlFlow::Continue
    });

    // Store the target permanently — menu items hold references to it
    MENU_TARGET.set(TequilaMenuHandler::new()).ok();
    let target = MENU_TARGET.get().expect("Menu target should be set");

    unsafe {
        let main_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        main_menu.setTitle(&NSString::from_str("MainMenu"));

        // ── App Menu ──
        let app_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let app_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        app_menu.setTitle(&NSString::from_str("AppMenu"));
        app_menu_item.setSubmenu(Some(&app_menu));
        main_menu.addItem(&app_menu_item);

        // About
        let about_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        about_item.setTitle(&NSString::from_str("About Tequila"));
        about_item.setAction(Some(sel!(handleMenuAction:)));
        about_item.setTarget(Some(&*target as &NSObject));
        about_item.setTag(1);
        app_menu.addItem(&about_item);

        // Separator
        let sep1 = NSMenuItem::separatorItem(mtm);
        app_menu.addItem(&sep1);

        // Preferences
        let prefs_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        prefs_item.setTitle(&NSString::from_str("Preferences\u{2026}"));
        prefs_item.setAction(Some(sel!(handleMenuAction:)));
        prefs_item.setTarget(Some(&*target as &NSObject));
        prefs_item.setTag(3);
        prefs_item.setKeyEquivalent(&NSString::from_str(","));
        prefs_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        app_menu.addItem(&prefs_item);

        // Separator
        let sep2 = NSMenuItem::separatorItem(mtm);
        app_menu.addItem(&sep2);

        // Quit
        let quit_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        quit_item.setTitle(&NSString::from_str("Quit Tequila"));
        quit_item.setAction(Some(sel!(handleMenuAction:)));
        quit_item.setTarget(Some(&*target as &NSObject));
        quit_item.setTag(5);
        quit_item.setKeyEquivalent(&NSString::from_str("q"));
        quit_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        app_menu.addItem(&quit_item);

        // ── File Menu ──
        let file_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let file_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        file_menu.setTitle(&NSString::from_str("File"));
        file_menu_item.setSubmenu(Some(&file_menu));
        main_menu.addItem(&file_menu_item);

        let new_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        new_item.setTitle(&NSString::from_str("New Prefix"));
        new_item.setAction(Some(sel!(handleMenuAction:)));
        new_item.setTarget(Some(&*target as &NSObject));
        new_item.setTag(2);
        new_item.setKeyEquivalent(&NSString::from_str("n"));
        new_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        file_menu.addItem(&new_item);

        // ── View Menu ──
        let view_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let view_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        view_menu.setTitle(&NSString::from_str("View"));
        view_menu_item.setSubmenu(Some(&view_menu));
        main_menu.addItem(&view_menu_item);

        let sidebar_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        sidebar_item.setTitle(&NSString::from_str("Toggle Sidebar"));
        sidebar_item.setAction(Some(sel!(handleMenuAction:)));
        sidebar_item.setTarget(Some(&*target as &NSObject));
        sidebar_item.setTag(4);
        sidebar_item.setKeyEquivalent(&NSString::from_str("\\"));
        sidebar_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        );
        view_menu.addItem(&sidebar_item);

        // ── Edit Menu (responder-chain with nil target) ──
        let edit_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let edit_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        edit_menu.setTitle(&NSString::from_str("Edit"));
        edit_menu_item.setSubmenu(Some(&edit_menu));
        main_menu.addItem(&edit_menu_item);

        // Undo
        let undo_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        undo_item.setTitle(&NSString::from_str("Undo"));
        undo_item.setAction(Some(sel!(undo:)));
        undo_item.setTarget(None);
        undo_item.setKeyEquivalent(&NSString::from_str("z"));
        undo_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&undo_item);

        // Redo
        let redo_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        redo_item.setTitle(&NSString::from_str("Redo"));
        redo_item.setAction(Some(sel!(redo:)));
        redo_item.setTarget(None);
        redo_item.setKeyEquivalent(&NSString::from_str("z"));
        redo_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Shift);
        edit_menu.addItem(&redo_item);

        // Separator
        let edit_sep1 = NSMenuItem::separatorItem(mtm);
        edit_menu.addItem(&edit_sep1);

        // Cut
        let cut_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        cut_item.setTitle(&NSString::from_str("Cut"));
        cut_item.setAction(Some(sel!(cut:)));
        cut_item.setTarget(None);
        cut_item.setKeyEquivalent(&NSString::from_str("x"));
        cut_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&cut_item);

        // Copy
        let copy_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        copy_item.setTitle(&NSString::from_str("Copy"));
        copy_item.setAction(Some(sel!(copy:)));
        copy_item.setTarget(None);
        copy_item.setKeyEquivalent(&NSString::from_str("c"));
        copy_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&copy_item);

        // Paste
        let paste_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        paste_item.setTitle(&NSString::from_str("Paste"));
        paste_item.setAction(Some(sel!(paste:)));
        paste_item.setTarget(None);
        paste_item.setKeyEquivalent(&NSString::from_str("v"));
        paste_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&paste_item);

        // Separator
        let edit_sep2 = NSMenuItem::separatorItem(mtm);
        edit_menu.addItem(&edit_sep2);

        // Select All
        let select_all_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        select_all_item.setTitle(&NSString::from_str("Select All"));
        select_all_item.setAction(Some(sel!(selectAll:)));
        select_all_item.setTarget(None);
        select_all_item.setKeyEquivalent(&NSString::from_str("a"));
        select_all_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&select_all_item);

        // ── Window Menu ──
        let window_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let window_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        window_menu.setTitle(&NSString::from_str("Window"));
        window_menu_item.setSubmenu(Some(&window_menu));
        main_menu.addItem(&window_menu_item);

        // Minimize
        let minimize_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        minimize_item.setTitle(&NSString::from_str("Minimize"));
        minimize_item.setAction(Some(sel!(performMiniaturize:)));
        minimize_item.setTarget(None);
        minimize_item.setKeyEquivalent(&NSString::from_str("m"));
        minimize_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        window_menu.addItem(&minimize_item);

        // Zoom
        let zoom_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        zoom_item.setTitle(&NSString::from_str("Zoom"));
        zoom_item.setAction(Some(sel!(performZoom:)));
        zoom_item.setTarget(None);
        window_menu.addItem(&zoom_item);

        // Set as the NSApplication's main menu
        let nsapp = NSApp(mtm);
        nsapp.setMainMenu(Some(&main_menu));
    }
}
