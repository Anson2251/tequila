pub mod menu;
pub mod resources;
pub use resources::initialize_custom_resources;

use adw::prelude::*;
use gtk::glib;
use gtk4::gio;
use log::{error, info};
use relm4::prelude::{AsyncComponent, AsyncController};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
    adw, component::AsyncComponentController, gtk,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tracker;

use crate::apps::AppManagerModel;
use crate::prefix::config::PrefixConfigModel;
use crate::prefix::list::PrefixListModel;
use crate::settings::SettingsWindow;
use menu::setup_menu_bar;
use prefix::runtime::RuntimeManager;
use prefix::{Manager as PrefixManager, ProcessTracker, WinePrefix};

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
    create_prefix_dialog:
        Option<relm4::Controller<crate::prefix::create_dialog::CreatePrefixDialog>>,
    #[tracker::do_not_track]
    export_dialog: Option<relm4::Controller<crate::prefix::export_dialog::ExportDialogModel>>,
    #[tracker::do_not_track]
    import_dialog: Option<relm4::Controller<crate::prefix::import_dialog::ImportDialogModel>>,
    #[tracker::do_not_track]
    sidebar_btn: gtk::Button,
}

#[derive(Debug)]
pub enum AppMsg {
    CreatePrefix,
    DeletePrefix(usize),
    ExportPrefix(usize),
    ImportPrefix,
    ShowError(String),
    ShowImportDialog {
        name: String,
        archive_wine: Option<String>,
        path: PathBuf,
    },
    OpenInFileManager(usize),
    OpenInTerminal(usize),
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
    ReinitComplete(usize, std::result::Result<(), String>),
}

impl AppModel {
    pub fn scan_wine_prefixes(prefix_manager: &PrefixManager) -> Vec<WinePrefix> {
        match prefix_manager.scan_prefixes() {
            Ok(prefixes) => prefixes,
            Err(e) => {
                error!("[app] error scanning prefixes: {}", e);
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
        sidebar_btn.connect_clicked(move |_| {
            sb_sender.input(AppMsg::ToggleSidebar);
        });
        header_bar.pack_start(&sidebar_btn);

        let back_btn = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back")
            .visible(false)
            .build();
        header_bar.pack_start(&back_btn);

        let import_btn = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Import Prefix")
            .build();
        let im_sender = sender.clone();
        import_btn.connect_clicked(move |_| {
            im_sender.input(AppMsg::ImportPrefix);
        });
        header_bar.pack_end(&import_btn);

        let new_prefix_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("New Prefix")
            .build();
        let np_sender = sender.clone();
        new_prefix_btn.connect_clicked(move |_| {
            np_sender.input(AppMsg::CreatePrefix);
        });
        header_bar.pack_end(&new_prefix_btn);

        let settings_btn = gtk::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Settings")
            .build();
        let st_sender = sender.clone();
        settings_btn.connect_clicked(move |_| {
            st_sender.input(AppMsg::ShowSettings);
        });
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
            )
            .expect("Failed to open icon cache"),
        );

        // Persistent state store
        let state_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("tequila/state.db");
        let prefix_store =
            Arc::new(prefix::PrefixStore::open(&state_path).expect("Failed to open state store"));

        let prefix_manager = PrefixManager::new(wine_dir.clone(), Arc::clone(&icon_cache));

        // Load prefixes from filesystem + JSON config files (fast, user-editable)
        let prefixes = AppModel::scan_wine_prefixes(&prefix_manager);
        // Trigger background scan if no cached scan results exist yet
        let needs_sync = !prefixes.is_empty()
            && prefixes
                .iter()
                .all(|p| !prefix_store.has_scanned_prefix(&p.path.to_string_lossy()));
        info!("[app] loaded {} prefixes", prefixes.len());

        let prefix_list = PrefixListModel::builder()
            .launch((prefixes.clone(), None))
            .forward(sender.input_sender(), |msg| match msg {
                crate::prefix::list::PrefixListOutput::SelectPrefix(index) => {
                    AppMsg::SelectPrefix(index)
                }
                crate::prefix::list::PrefixListOutput::DeselectPrefix => AppMsg::HideDetails,
                crate::prefix::list::PrefixListOutput::DeletePrefix(index) => {
                    AppMsg::DeletePrefix(index)
                }
                crate::prefix::list::PrefixListOutput::ExportPrefix(index) => {
                    AppMsg::ExportPrefix(index)
                }
                crate::prefix::list::PrefixListOutput::OpenInFileManager(index) => {
                    AppMsg::OpenInFileManager(index)
                }
                crate::prefix::list::PrefixListOutput::OpenInTerminal(index) => {
                    AppMsg::OpenInTerminal(index)
                }
            });

        let config_tab = PrefixConfigModel::builder()
            .launch((
                PathBuf::new(),
                prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()),
                Arc::clone(&prefix_store),
                Arc::clone(&process_tracker),
                back_btn,
                prefix_manager.runtime_manager().clone(),
                root.clone().upcast::<gtk::Window>(),
                prefix_manager.clone(),
            ))
            .forward(sender.input_sender(), |msg| match msg {
                crate::prefix::config::PrefixConfigOutput::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
            });

        let app_manager = AppManagerModel::builder()
            .launch((
                PathBuf::new(),
                prefix::config::PrefixConfig::new("".to_string(), "win64".to_string()),
                prefix_manager.clone(),
                Arc::clone(&icon_cache),
                Arc::clone(&prefix_store),
                Arc::clone(&process_tracker),
                root.clone().upcast::<gtk::Window>(),
            ))
            .forward(sender.input_sender(), |msg| match msg {
                crate::apps::AppManagerMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes,
            });

        let settings = SettingsWindow::builder()
            .launch(prefix_manager.clone())
            .forward(sender.input_sender(), |msg| match msg {
                crate::settings::SettingsOutput::RuntimesUpdated(rm) => AppMsg::RuntimesUpdated(rm),
            });

        let prefix_list_widget = prefix_list.widget().clone().upcast::<gtk::Widget>();

        // Empty state page
        let empty_page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .build();
        empty_page.append(
            &gtk::Image::builder()
                .pixel_size(72)
                .icon_name("brand-winehq-symbolic")
                .css_classes(["dim-label"])
                .build(),
        );
        empty_page.append(
            &gtk::Label::builder()
                .label("No prefix selected")
                .css_classes(["title-4", "dim-label"])
                .margin_top(10)
                .build(),
        );

        // Tabbed content Stack
        let content_stack = adw::ViewStack::new();
        content_stack
            .add_titled(app_manager.widget(), Some("apps"), "Apps")
            .set_icon_name(Some("application-x-executable-symbolic"));
        content_stack
            .add_titled(config_tab.widget(), Some("config"), "Config")
            .set_icon_name(Some("document-properties-symbolic"));
        switcher.set_stack(Some(&content_stack));

        // Wrapper Stack: show either empty page or tabbed content
        let content_box = gtk::Stack::builder()
            .hexpand(true)
            .vexpand(true)
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
        let sync_progress_bar = gtk::ProgressBar::builder().width_request(260).build();
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

        let has_prefixes = !prefixes.is_empty();

        let model = AppModel {
            prefixes,
            prefix_manager,
            selected_prefix: None,
            prefix_list,
            prefix_config: config_tab,
            app_manager,
            settings,
            create_prefix_dialog: None,
            export_dialog: None,
            import_dialog: None,
            sidebar_btn: sidebar_btn.clone(),
            content_stack,
            content_box,
            flap,
            switcher,
            prefix_store,
            syncing: false,
            sidebar_visible: has_prefixes,
            main_window: root.clone(),
            sync_overlay: sync_overlay_box,
            sync_progress_bar,
            sync_progress_label,
            tracker: 0,
        };

        // If there are no prefixes, hide the sidebar
        if !has_prefixes {
            model.flap.set_show_sidebar(false);
            model.sidebar_btn.set_sensitive(false);
        }

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
                        error!("[app] failed to delete prefix: {}", e);
                    } else {
                        self.prefixes.remove(index);
                        if self.selected_prefix == Some(index) {
                            self.selected_prefix = None;
                        } else if let Some(selected) = self.selected_prefix {
                            if selected > index {
                                self.selected_prefix = Some(selected - 1);
                            }
                        }
                        info!("[app] deleted prefix: {}", prefix_name);
                        if self.prefixes.is_empty() {
                            sender.input(AppMsg::HideDetails);
                        }
                        sender.input(AppMsg::RefreshPrefixes);
                    }
                }
            }
            AppMsg::LaunchPrefix(index) => {
                if index < self.prefixes.len() {
                    let prefix_name = self.prefixes[index].name.clone();
                    let prefix_path = self.prefixes[index].path.clone();

                    info!(
                        "[app] launching prefix: {} at {}",
                        prefix_name,
                        prefix_path.display()
                    );

                    // Launch winecfg for the prefix
                    match self.prefix_manager.run_winecfg(&prefix_path) {
                        Ok(_) => {
                            info!(
                                "[app] successfully launched winecfg for prefix: {}",
                                prefix_name
                            );
                        }
                        Err(e) => {
                            error!(
                                "[app] failed to launch winecfg for prefix {}: {}",
                                prefix_name, e
                            );
                            let alert = adw::AlertDialog::new(
                                Some("Launch Failed"),
                                Some(&format!("Failed to launch winecfg:\n\n{}", e)),
                            );
                            alert.add_response("ok", "OK");
                            alert.set_default_response(Some("ok"));
                            alert.set_close_response("ok");
                            alert.choose(
                                Some(&self.main_window.clone().upcast::<gtk::Window>()),
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
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
                        if let Err(e) = self
                            .prefix_manager
                            .launch_executable(prefix_path, executable)
                        {
                            error!("[app] failed to launch executable: {}", e);
                            let alert = adw::AlertDialog::new(
                                Some("Launch Failed"),
                                Some(&format!("Failed to launch '{}':\n\n{}", executable.name, e)),
                            );
                            alert.add_response("ok", "OK");
                            alert.set_default_response(Some("ok"));
                            alert.set_close_response("ok");
                            alert.choose(
                                Some(&self.main_window.clone().upcast::<gtk::Window>()),
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                        }
                    }
                }
            }
            AppMsg::ExportPrefix(index) => {
                if index >= self.prefixes.len() {
                    return;
                }
                let prefix_path = self.prefixes[index].path.clone();
                let prefix_name = self.prefixes[index].name.clone();

                let dialog = crate::prefix::export_dialog::ExportDialogModel::builder()
                    .launch((
                        self.prefix_manager.clone(),
                        prefix_path,
                        prefix_name,
                        self.main_window.clone(),
                    ))
                    .forward(sender.input_sender(), |msg| msg);
                self.export_dialog = Some(dialog);
            }
            AppMsg::ImportPrefix => {
                let parent: gtk::Window = self.main_window.clone().upcast();
                let pm = self.prefix_manager.clone();
                let s = sender.clone();

                let exts = [&format!("zst.{}", prefix::TQL_EXTENSION)[..]];
                crate::dialogs::pick_file(&parent, "Import Prefix", &exts, move |path| {
                    if let Some(path) = path {
                        let p = PathBuf::from(&path);
                        let pm = pm.clone();
                        let s = s.clone();
                        std::thread::spawn(move || {
                            let (name, archive_wine) = match pm.inspect_archive(&p) {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = s.input(AppMsg::ShowError(format!(
                                        "Failed to read archive:\n\n{}",
                                        e
                                    )));
                                    return;
                                }
                            };
                            let _ = s.input(AppMsg::ShowImportDialog {
                                name,
                                archive_wine,
                                path: p,
                            });
                        });
                    }
                });
            }
            AppMsg::ShowError(msg) => {
                let alert = adw::AlertDialog::new(Some("Error"), Some(&msg));
                alert.add_response("ok", "OK");
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.choose(
                    Some(&self.main_window.clone().upcast::<gtk::Window>()),
                    None::<&gio::Cancellable>,
                    |_| {},
                );
            }
            AppMsg::ShowImportDialog { name, path, .. } => {
                let dialog = crate::prefix::import_dialog::ImportDialogModel::builder()
                    .launch((
                        self.prefix_manager.clone(),
                        path,
                        name,
                        self.main_window.clone(),
                    ))
                    .forward(sender.input_sender(), |msg| msg);
                self.import_dialog = Some(dialog);
            }
            AppMsg::OpenInFileManager(index) => {
                if let Some(prefix) = self.prefixes.get(index) {
                    let path = prefix.path.to_string_lossy().to_string();
                    std::thread::spawn(move || {
                        #[cfg(target_os = "macos")]
                        let _ = std::process::Command::new("open").arg(&path).status();
                        #[cfg(not(target_os = "macos"))]
                        let _ = std::process::Command::new("xdg-open").arg(&path).status();
                    });
                }
            }
            AppMsg::OpenInTerminal(index) => {
                if let Some(prefix) = self.prefixes.get(index) {
                    let prefix_path = prefix.path.clone();
                    let pm = self.prefix_manager.clone();
                    std::thread::spawn(move || match pm.generate_terminal_script(&prefix_path) {
                        Ok(script) => {
                            let tmp = std::env::temp_dir().join("tequila-terminal.sh");
                            if let Err(e) = std::fs::write(&tmp, &script) {
                                error!("[term] failed to write script: {}", e);
                                return;
                            }
                            if let Err(e) = std::fs::set_permissions(
                                &tmp,
                                std::fs::Permissions::from_mode(0o755),
                            ) {
                                error!("[term] failed to chmod script: {}", e);
                            }
                            open_terminal_with_script(&tmp);
                        }
                        Err(e) => error!("[term] failed to generate script: {}", e),
                    });
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
                    info!("[app] selected prefix: {}", self.prefixes[index].name);
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
                    self.prefix_config.emit(
                        crate::prefix::config::PrefixConfigMsg::PrefixPathUpdated(
                            prefix_path.clone(),
                        ),
                    );
                    self.app_manager
                        .emit(crate::apps::AppManagerMsg::PrefixPathUpdated(prefix_path));

                    // Resolve runtime display name
                    let runtime_display = config
                        .wine_version
                        .as_ref()
                        .and_then(|id| self.prefix_manager.runtime_manager().get(id))
                        .map(|r| format!("{} ({})", r.name, r.wine_version))
                        .unwrap_or_else(|| {
                            config
                                .wine_version
                                .as_deref()
                                .unwrap_or("Unknown")
                                .to_string()
                        });
                    self.prefix_config.emit(
                        crate::prefix::config::PrefixConfigMsg::SetWineVersionDisplay(
                            runtime_display,
                        ),
                    );

                    self.prefix_config
                        .emit(crate::prefix::config::PrefixConfigMsg::ConfigUpdated(
                            config.clone(),
                        ));
                    self.prefix_config.emit(
                        crate::prefix::config::PrefixConfigMsg::SetPrefixIndex(index),
                    );
                    self.app_manager
                        .emit(crate::apps::AppManagerMsg::ConfigUpdated(config.clone()));
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
                        let prefix_path = self.prefixes[actual_index].path.clone();
                        let old_graphics = self.prefixes[actual_index].config.graphics.clone();
                        let new_graphics = config.graphics.clone();
                        let old_wine_version =
                            self.prefixes[actual_index].config.wine_version.clone();
                        let new_wine_version = config.wine_version.clone();

                        // Detect if the graphics backend changed
                        let graphics_changed = match (&old_graphics, &new_graphics) {
                            (None, None) => false,
                            (Some(a), Some(b)) => a.backend != b.backend || a.version != b.version,
                            _ => true, // None <-> Some
                        };

                        if graphics_changed {
                            // Save the full pre-change config so we can roll
                            // back on disk + notify UIs if anything fails.
                            let rollback_config = self.prefixes[actual_index].config.clone();

                            // Update in-memory state + notify UIs immediately.
                            // This gives instant visual feedback; if the async
                            // operation fails below we restore the old state.
                            self.prefixes[actual_index].config = config.clone();
                            self.prefix_config.emit(
                                crate::prefix::config::PrefixConfigMsg::ConfigUpdated(
                                    config.clone(),
                                ),
                            );
                            self.app_manager
                                .emit(crate::apps::AppManagerMsg::ConfigUpdated(config.clone()));

                            // Async: deactivate old backend, then activate new one
                            let pm = self.prefix_manager.clone();
                            let pp = prefix_path.clone();
                            let window = self.main_window.clone().upcast::<gtk::Window>();
                            let s = sender.clone();
                            glib::MainContext::default().spawn_local(async move {
                                let mut failed = false;

                                if let Some(ref old_gfx) = old_graphics {
                                    info!("[config] deactivating old graphics backend");
                                    if let Err(e) = pm
                                        .deactivate_graphics_backend(&pp, Some(old_gfx.clone()))
                                        .await
                                    {
                                        error!("[app] failed to deactivate old graphics: {}", e);
                                        failed = true;
                                        show_error_dialog(
                                            &window,
                                            "Failed to Deactivate Graphics Backend",
                                            &e,
                                        );
                                    }
                                }

                                if !failed {
                                    if let Some(ref gfx) = new_graphics {
                                        if let Some(backend) = gfx.to_backend() {
                                            info!(
                                                "[config] activating {} graphics backend",
                                                backend.display_name()
                                            );
                                            if let Err(e) =
                                                pm.activate_graphics_backend(&backend, &pp).await
                                            {
                                                error!("[app] failed to activate graphics: {}", e);
                                                failed = true;
                                                show_error_dialog(
                                                    &window,
                                                    "Failed to Activate Graphics Backend",
                                                    &e,
                                                );
                                            }
                                        }
                                    }
                                }

                                // On failure: restore the old config on disk
                                // and emit updates so all UIs revert.
                                if failed {
                                    let _ = pm.update_config(&pp, &rollback_config);
                                    // Emit the rollback config so the UI reverts
                                    s.input(AppMsg::ConfigUpdated(0, rollback_config));
                                }
                            });
                        } else {
                            // No backend change — normal config save
                            if let Err(e) = self.prefix_manager.update_config(&prefix_path, &config)
                            {
                                error!("[app] failed to update config: {}", e)
                            } else {
                                self.prefixes[actual_index].config = config.clone();
                                self.prefix_config.emit(
                                    crate::prefix::config::PrefixConfigMsg::ConfigUpdated(
                                        config.clone(),
                                    ),
                                );
                                self.app_manager
                                    .emit(crate::apps::AppManagerMsg::ConfigUpdated(
                                        config.clone(),
                                    ));
                            }
                        }

                        // Detect if wine version changed — reinitialize prefix on a
                        // background thread if it did.
                        if old_wine_version != new_wine_version {
                            let pm = self.prefix_manager.clone();
                            let pp = prefix_path.clone();
                            let cfg = config.clone();
                            let s = sender.clone();
                            let prefix_index = actual_index;

                            std::thread::spawn(move || {
                                let result = pm.reinitialize_prefix(&pp, &cfg);
                                let msg = AppMsg::ReinitComplete(
                                    prefix_index,
                                    result.map_err(|e| e.to_string()),
                                );
                                // Send back to main thread via component channel
                                let _ = s.input(msg);
                            });
                        }
                    }
                }
            }
            AppMsg::ReinitComplete(_index, result) => {
                // Forward to prefix config
                self.prefix_config
                    .emit(crate::prefix::config::PrefixConfigMsg::ReinitComplete(
                        result,
                    ));
            }
            AppMsg::ScanForApplications(index) => {
                if index < self.prefixes.len() {
                    let prefix_path = self.prefixes[index].path.clone();
                    let prefix_name = self.prefixes[index].name.clone();

                    match self.prefix_manager.scan_for_applications(&prefix_path) {
                        Ok(executables) => {
                            info!(
                                "[app] found {} applications in prefix '{}'",
                                executables.len(),
                                prefix_name
                            );

                            // Get the current config and update it
                            let mut config = self.prefixes[index].config.clone();
                            let initial_count = config.registered_executables.len();

                            for executable in executables {
                                config.add_executable(executable);
                            }

                            let new_count = config.registered_executables.len();
                            let added_count = new_count - initial_count;

                            // Save the updated config
                            if let Err(e) = self.prefix_manager.update_config(&prefix_path, &config)
                            {
                                error!(
                                    "[app] failed to save updated config for prefix '{}': {}",
                                    prefix_name, e
                                );
                            } else {
                                info!(
                                    "[app] successfully updated prefix '{}' config with {} new executables (total: {})",
                                    prefix_name, added_count, new_count
                                );

                                // Update the local copy
                                self.prefixes[index].config = config;
                            }
                        }
                        Err(e) => {
                            error!(
                                "[app] failed to scan for applications in prefix '{}': {}",
                                prefix_name, e
                            );
                            // TODO: Show error dialog to user
                        }
                    }
                }
            }
            AppMsg::SyncComplete(fresh) => {
                self.set_syncing(false);
                self.sync_overlay.set_visible(false);
                self.prefixes = fresh.clone();

                // Close sidebar if all prefixes were removed
                if fresh.is_empty() {
                    self.flap.set_show_sidebar(false);
                    self.set_sidebar_visible(false);
                    self.sidebar_btn.set_sensitive(false);
                } else {
                    self.sidebar_btn.set_sensitive(true);
                }

                self.prefix_list
                    .emit(crate::prefix::list::PrefixListMsg::SetPrefixes(fresh));
            }
            AppMsg::ReloadPrefixes(fresh) => {
                // Light reload: update the prefix list without app scanning or auto-select
                self.prefixes = fresh.clone();

                // Close sidebar if all prefixes were removed
                if fresh.is_empty() {
                    self.flap.set_show_sidebar(false);
                    self.set_sidebar_visible(false);
                    self.sidebar_btn.set_sensitive(false);
                } else {
                    self.sidebar_btn.set_sensitive(true);
                }

                self.prefix_list
                    .emit(crate::prefix::list::PrefixListMsg::SetPrefixes(fresh));
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
                                let _ =
                                    st.save_scanned_executables(&p.path.to_string_lossy(), &exes);
                            }
                            let changed = sm.enrich_executables(&p.path, &mut p.config);
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
                self.sync_progress_bar.set_fraction(if total > 0 {
                    completed as f64 / total as f64
                } else {
                    0.0
                });
                self.sync_progress_label
                    .set_label(&format!("{} / {} prefixes", completed, total));
            }
            AppMsg::ToggleSidebar => {
                if self.prefixes.is_empty() {
                    return;
                }
                let visible = !self.sidebar_visible;
                self.set_sidebar_visible(visible);
                self.flap.set_show_sidebar(visible);
            }
            AppMsg::ShowSettings => {
                self.settings
                    .widget()
                    .set_transient_for(Some(&self.main_window));
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

// ── Helpers ─────────────────────────────────────────────────────────────

/// Try to open a terminal emulator that runs the given shell script.
///
/// The script should be executable and self-contained (it sets up the Wine
/// environment and starts an interactive shell). On macOS we use
/// Terminal.app via AppleScript; on Linux we try common terminals.
pub fn open_terminal_with_script(script_path: &std::path::Path) {
    let path_str = script_path.to_string_lossy();

    #[cfg(target_os = "macos")]
    {
        let template = include_str!("../../../../scripts/tequila-terminal.applescript");
        let src = template.replace("__TEQUILA_SCRIPT_PATH__", &path_str.replace('"', "\\\""));
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&src)
            .status();
        let _ = std::process::Command::new("open")
            .args(["-a", "Terminal"])
            .status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Try terminals that support -e with a script path.
        // We run `bash /path/to/script` so the script's shebang and trap work.
        let cmds: &[&[&str]] = &[
            &["x-terminal-emulator", "-e", "bash", &path_str],
            &["gnome-terminal", "--", "bash", &path_str],
            &["xfce4-terminal", "-e", "bash", &path_str],
            &["konsole", "-e", "bash", &path_str],
            &["lxterminal", "-e", "bash", &path_str],
            &["xterm", "-e", "bash", &path_str],
            &["kgx", "-e", "bash", &path_str],
        ];
        for args in cmds {
            let cmd = args[0];
            let rest = &args[1..];
            if std::process::Command::new(cmd).args(rest).spawn().is_ok() {
                return;
            }
        }
    }
}

/// Show a simple error dialog with an OK button.
fn show_error_dialog(parent: &gtk::Window, title: &str, msg: &dyn std::fmt::Display) {
    let alert = adw::AlertDialog::new(Some(title), Some(&msg.to_string()));
    alert.add_response("ok", "OK");
    alert.set_default_response(Some("ok"));
    alert.set_close_response("ok");
    alert.choose(Some(parent), None::<&gio::Cancellable>, |_| {});
}
