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
use super::{PrefixListModel, PrefixDetailsModel, AppManagerModel, RegistryEditorModel};
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
    pub content_stack: adw::ViewStack,
    #[tracker::do_not_track]
    pub flap: adw::Flap,
    #[tracker::do_not_track]
    pub info_btn: gtk::Button,
    #[tracker::do_not_track]
    pub switcher: adw::ViewSwitcherTitle,
    #[tracker::do_not_track]
    pub prefix_store: Arc<crate::prefix::PrefixStore>,
    pub syncing: bool,
    pub sidebar_visible: bool,
    #[tracker::do_not_track]
    sync_overlay: gtk::CenterBox,
    #[tracker::do_not_track]
    sync_progress_bar: gtk::ProgressBar,
    #[tracker::do_not_track]
    sync_progress_label: gtk::Label,
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
    CreatePrefixComplete(String, String), // name, architecture
    ShowPrefixInfo,
    SyncComplete(Vec<WinePrefix>),
    SyncPrefixes,
    SyncProgress(usize, usize),
    ToggleSidebar,
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

        let refresh_btn = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Sync prefixes from disk")
            .build();
        let rf_sender = sender.clone();
        refresh_btn.connect_clicked(move |_| { rf_sender.input(AppMsg::SyncPrefixes); });
        header_bar.pack_start(&refresh_btn);

        let info_btn = gtk::Button::builder()
            .icon_name("dialog-information-symbolic")
            .tooltip_text("Prefix Info")
            .sensitive(false)
            .build();
        let info_sender = sender.clone();
        info_btn.connect_clicked(move |_| { info_sender.input(AppMsg::ShowPrefixInfo); });
        header_bar.pack_end(&info_btn);

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

        // Load prefixes from cache (instant)
        let cached: Vec<WinePrefix> = prefix_store.list_prefixes()
            .map_err(|e| eprintln!("Failed to load from cache: {}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|(path, config)| WinePrefix {
                name: config.name.clone(),
                path: PathBuf::from(path),
                config,
            })
            .collect();

        let needs_sync = cached.is_empty();
        let prefixes = cached;
        println!("Loaded {} prefixes from cache", prefixes.len());

        // If cache is empty, trigger background scan
        if needs_sync {
            let ss = sender.clone();
            let sp = sender.clone();
            let sm = prefix_manager.clone();
            let st = Arc::clone(&prefix_store);
            let ic = Arc::clone(&icon_cache);
            glib::spawn_future_local(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let mut fresh = AppModel::scan_wine_prefixes(&sm);
                    let total = fresh.len();
                    for (i, p) in fresh.iter_mut().enumerate() {
                        let _ = st.save_prefix(&p.path.to_string_lossy(), &p.config);
                        if let Ok(exes) = sm.scan_for_applications(&p.path) {
                            let _ = st.save_scanned_executables(&p.path.to_string_lossy(), &exes);
                        }
                        let mut changed = false;
                        for exe in &mut p.config.registered_executables {
                            if let Some(icon_path) = crate::prefix::scanner::extract_icon_for_exe(&exe.executable_path, &ic) {
                                if exe.icon_path.as_ref() != Some(&icon_path) {
                                    exe.icon_path = Some(icon_path);
                                    changed = true;
                                }
                            }
                            if exe.file_description.is_none() {
                                let meta = crate::prefix::scanner::extract_metadata_for_exe(&exe.executable_path);
                                if meta.file_version.is_some() || meta.file_description.is_some() {
                                    exe.file_version = meta.file_version;
                                    exe.product_version = meta.product_version;
                                    exe.company_name = meta.company_name;
                                    exe.file_description = meta.file_description;
                                    exe.product_name = meta.product_name;
                                    exe.imported_modules = meta.imported_modules;
                                    changed = true;
                                }
                            }
                        }
                        if changed {
                            let _ = st.save_prefix(&p.path.to_string_lossy(), &p.config);
                        }
                        let _ = sp.input(AppMsg::SyncProgress(i + 1, total));
                    }
                    fresh
                }).await;
                if let Ok(fresh) = result {
                    let _ = ss.input(AppMsg::SyncComplete(fresh));
                }
            });
        }

        let prefix_list = PrefixListModel::builder()
            .launch((prefixes.clone(), None))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::prefix_list::PrefixListOutput::SelectPrefix(index) => AppMsg::SelectPrefix(index),
                crate::ui::prefix_list::PrefixListOutput::ShowPrefixDetails(index) => AppMsg::ShowPrefixDetails(index),
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
            .launch((PathBuf::new(), crate::prefix::config::PrefixConfig::new("".to_string(), "win64".to_string())))
            .forward(sender.input_sender(), |msg| match msg {
                crate::ui::registry_editor::RegistryEditorMsg::ConfigUpdated(config) => {
                    AppMsg::ConfigUpdated(0, config)
                }
                _ => AppMsg::RefreshPrefixes
            });

        let prefix_list_widget = prefix_list.widget().clone().upcast::<gtk::Widget>();

        // Build content Stack
        let content_stack = adw::ViewStack::new();
        {
            let empty_page = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .halign(gtk::Align::Center).valign(gtk::Align::Center).vexpand(true).build();
            empty_page.append(&gtk::Image::builder().pixel_size(72)
                .icon_name("brand-winehq-symbolic").css_classes(["dim-label"]).build());
            empty_page.append(&gtk::Label::builder().label("No prefix selected")
                .css_classes(["title-4", "dim-label"]).margin_top(10).build());

            let sync_spinner = gtk::Spinner::new();
            sync_spinner.set_margin_top(20);
            empty_page.append(&sync_spinner);
            if needs_sync {
                sync_spinner.start();
            }
            let sync_label = gtk::Label::builder()
                .label(if needs_sync { "Scanning Wine prefixes..." } else { "Select a prefix from the list to view details" })
                .css_classes(["body", "dim-label"]).build();
            empty_page.append(&sync_label);
            content_stack.add(&empty_page);
        }
        content_stack.add_titled(app_manager.widget(), Some("apps"), "Apps")
            .set_icon_name(Some("application-x-executable"));
        content_stack.add_titled(prefix_details.widget(), Some("details"), "Details")
            .set_icon_name(Some("document-properties"));
        content_stack.add_titled(registry_editor.widget(), Some("registry"), "Registry")
            .set_icon_name(Some("preferences-system"));
        content_stack.set_visible_child_name("empty");
        switcher.set_stack(Some(&content_stack));

        // Content area
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical).hexpand(true).vexpand(true).build();
        content_stack.set_hexpand(true);
        content_stack.set_vexpand(true);
        content_box.append(&content_stack);

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
                                     .sync-progress-box { background: @view_bg_color; border: 1px solid @borders; border-radius: 12px; padding: 24px; }");
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
            content_stack,
            flap,
            info_btn,
            switcher,
            prefix_store,
            syncing: needs_sync,
            sidebar_visible: true,
            sync_overlay: sync_overlay_box,
            sync_progress_bar,
            sync_progress_label,
            tracker: 0,
        };

        let widgets = view_output!();

        // Auto-select first prefix
        if !model.prefixes.is_empty() {
            sender_clone.input(AppMsg::ShowPrefixDetails(0));
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
                sender.input(AppMsg::SyncPrefixes);
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
                    self.info_btn.set_sensitive(true);
                    self.switcher.set_sensitive(true);
                    self.content_stack.set_visible_child_name("apps");

                    // Update the details component
                    let config = self.prefixes[index].config.clone();
                    let prefix_path = self.prefixes[index].path.clone();

                    // Emit path first so ConfigUpdated handlers have the correct prefix path
                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::PrefixPathUpdated(prefix_path.clone()));
                    self.app_manager.emit(crate::ui::app_manager::AppManagerMsg::PrefixPathUpdated(prefix_path.clone()));
                    self.registry_editor.emit(crate::ui::registry_editor::RegistryEditorMsg::PrefixPathUpdated(prefix_path));

                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::ConfigUpdated(config.clone()));
                    self.prefix_details.emit(crate::ui::prefix_details::PrefixDetailsMsg::SetPrefixIndex(index));
                    self.app_manager.emit(crate::ui::app_manager::AppManagerMsg::ConfigUpdated(config.clone()));
                    self.registry_editor.emit(crate::ui::registry_editor::RegistryEditorMsg::ConfigUpdated(config.clone()));
                }
            }
            AppMsg::HideDetails => {
                self.switcher.set_sensitive(false);
                self.info_btn.set_sensitive(false);
                self.content_stack.set_visible_child_name("empty");
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
                            let _ = self.prefix_store.save_prefix(&prefix_path.to_string_lossy(), &config);
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
            AppMsg::ShowPrefixInfo => {
                if let Some(idx) = self.selected_prefix {
                    if idx < self.prefixes.len() {
                        let p = &self.prefixes[idx];
                        let info = format!(
                            "Path: {}\nArchitecture: {}\nWine: {}\nApps: {}\nCreated: {}\nModified: {}",
                            p.path.display(),
                            p.config.architecture,
                            p.config.wine_version.as_deref().unwrap_or("Unknown"),
                            p.config.registered_executables.len(),
                            p.config.creation_date.format("%Y-%m-%d %H:%M"),
                            p.config.last_modified.format("%Y-%m-%d %H:%M"),
                        );
                        let dialog = gtk::Dialog::builder()
                            .modal(true)
                            .build();
                        dialog.set_title(Some(&p.name));
                        let content = dialog.content_area();
                        let label = gtk::Label::builder()
                            .label(&info)
                            .halign(gtk::Align::Start)
                            .margin_start(16).margin_top(16).margin_bottom(16).margin_end(16)
                            .build();
                        content.append(&label);
                        dialog.add_button("OK", gtk::ResponseType::Ok);
                        dialog.connect_response(|d, _| d.close());
                        dialog.present();
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
                    glib::spawn_future_local(async move {
                        let result = tokio::task::spawn_blocking(move || {
                            let mut fresh = AppModel::scan_wine_prefixes(&sm);
                            let total = fresh.len();
                            let ic = sm.scanner().icon_cache();
                            for (i, p) in fresh.iter_mut().enumerate() {
                                let _ = st.save_prefix(&p.path.to_string_lossy(), &p.config);
                                if let Ok(exes) = sm.scan_for_applications(&p.path) {
                                    let _ = st.save_scanned_executables(&p.path.to_string_lossy(), &exes);
                                }
                                let mut changed = false;
                                for exe in &mut p.config.registered_executables {
                                    if let Some(icon_path) = crate::prefix::scanner::extract_icon_for_exe(&exe.executable_path, &ic) {
                                        if exe.icon_path.as_ref() != Some(&icon_path) {
                                            exe.icon_path = Some(icon_path);
                                            changed = true;
                                        }
                                    }
                                    if exe.file_description.is_none() {
                                        let meta = crate::prefix::scanner::extract_metadata_for_exe(&exe.executable_path);
                                        if meta.file_version.is_some() || meta.file_description.is_some() {
                                            exe.file_version = meta.file_version;
                                            exe.product_version = meta.product_version;
                                            exe.company_name = meta.company_name;
                                            exe.file_description = meta.file_description;
                                            exe.product_name = meta.product_name;
                                            exe.imported_modules = meta.imported_modules;
                                            changed = true;
                                        }
                                    }
                                }
                                if changed {
                                    let _ = st.save_prefix(&p.path.to_string_lossy(), &p.config);
                                }
                                let _ = sp.input(AppMsg::SyncProgress(i + 1, total));
                            }
                            fresh
                        }).await;
                        if let Ok(fresh) = result {
                            let _ = ss.input(AppMsg::SyncComplete(fresh));
                        }
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
