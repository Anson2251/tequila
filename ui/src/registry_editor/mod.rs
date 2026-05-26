use gtk::prelude::*;
use notify::{RecursiveMode, Watcher, recommended_watcher};
use prefix::registry::cache::InMemoryRegistryCache;
use prefix::registry::keys::*;
use prefix::{
    PrefixError, ProcessTracker,
    config::PrefixConfig,
    registry::{RegEditor, RegistryEditor},
};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
    SimpleComponent, gtk,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use tokio::sync::{Mutex, oneshot};
use tracker;

pub mod general_tab;
pub mod graphics_tab;
pub mod platform_tab;

pub use general_tab::{GeneralSettings, GeneralTabModel};
pub use graphics_tab::{GraphicsSettings, GraphicsTabModel};
pub use platform_tab::{MacSettings, PlatformSettings, PlatformTabModel, X11Settings};

// ── Model ────────────────────────────────────────────────────────────────

#[derive(Debug)]
#[tracker::track]
pub struct RegistryEditorModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    editing: bool,
    loading: bool,
    #[tracker::do_not_track]
    pending_edit: bool,
    #[tracker::do_not_track]
    registry_editor: Option<Arc<Mutex<RegistryEditor>>>,
    // Tab component controllers
    #[tracker::do_not_track]
    pub general_ctrl: Controller<GeneralTabModel>,
    #[tracker::do_not_track]
    pub graphics_ctrl: Controller<GraphicsTabModel>,
    #[tracker::do_not_track]
    pub platform_ctrl: Controller<PlatformTabModel>,
    #[tracker::do_not_track]
    prefix_store: Arc<prefix::PrefixStore>,
    #[tracker::do_not_track]
    process_tracker: Arc<std::sync::Mutex<ProcessTracker>>,
    #[tracker::do_not_track]
    watch_kill: Option<mpsc::Sender<()>>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum RegistryEditorMsg {
    ToggleEdit,
    SaveRegistry,
    LoadRegistry,
    LoadForEdit,
    RegistryEditorLoaded(Arc<Mutex<RegistryEditor>>),
    LoadSettings(GeneralSettings, GraphicsSettings, PlatformSettings),
    RegistrySaveComplete,
    RegistrySaveError(String),
    ConfigUpdated(PrefixConfig),
    CancelEdit,
    RunWinecfg,
    RunRegedit,
    RefreshReg,
    PrefixPathUpdated(PathBuf),
    /// Unified handler: (section, setting) where setting is "key=value" or just "value"
    ApplySetting(String, String),
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub)]
impl SimpleComponent for RegistryEditorModel {
    type Init = (
        PathBuf,
        PrefixConfig,
        Arc<prefix::PrefixStore>,
        Arc<std::sync::Mutex<ProcessTracker>>,
    );
    type Input = RegistryEditorMsg;
    type Output = RegistryEditorMsg;
    type Widgets = RegistryEditorWidgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,
            set_hexpand: true,
            set_vexpand: true,

            #[transition = "Crossfade"]
            if !model.loading {
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 0,
                    set_hexpand: true,
                    set_vexpand: true,

                    #[name = "notebook"]
                    gtk::Notebook {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_show_border: false,

                        append_page: (
                            &model.general_ctrl.widget().clone(),
                            Some(&gtk::Label::builder().label("General").build())
                        ),

                        append_page: (
                            &model.graphics_ctrl.widget().clone(),
                            Some(&gtk::Label::builder().label("Graphics").build())
                        ),

                        append_page: (
                            &model.platform_ctrl.widget().clone(),
                            Some(&gtk::Label::builder().label("Platform").build())
                        ),
                    },

                    // Control buttons
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 10,
                        set_margin_all: 10,

                        gtk::Button {
                            set_icon_name: "applications-engineering-symbolic",
                            set_tooltip_text: Some("Launch Wine Configuration"),
                            connect_clicked => RegistryEditorMsg::RunWinecfg,
                        },

                        gtk::Button {
                            set_icon_name: "document-properties-symbolic",
                            set_tooltip_text: Some("Launch Wine Registry Editor"),
                            connect_clicked => RegistryEditorMsg::RunRegedit,
                        },

                        gtk::Box {
                            set_hexpand: true,
                        },

                        gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_tooltip_text: Some("Reload registry from disk"),
                            connect_clicked => RegistryEditorMsg::RefreshReg,
                        },

                        gtk::Separator {
                            set_orientation: gtk::Orientation::Vertical,
                        },

                        gtk::Button {
                            #[watch]
                            set_icon_name: if model.editing { "object-select-symbolic" } else { "document-edit-symbolic" },
                            #[watch]
                            set_tooltip_text: if model.editing { Some("Save") } else { Some("Edit") },
                            #[watch]
                            set_css_classes: if model.editing { &["suggested-action"] } else { &[] },
                            connect_clicked => RegistryEditorMsg::ToggleEdit,
                        },

                        gtk::Button {
                            set_icon_name: "edit-undo-symbolic",
                            set_tooltip_text: Some("Cancel"),
                            #[watch]
                            set_visible: model.editing,
                            connect_clicked[sender] => move |_| {
                                sender.input(RegistryEditorMsg::CancelEdit);
                            },
                        },
                    },
                }
            }
            else {
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    set_margin_top: 10,

                    gtk::Spinner {
                        set_halign: gtk::Align::Center,
                        set_margin_bottom: 10,
                    },

                    gtk::Label {
                        set_label: "Loading registry editor...",
                        set_halign: gtk::Align::Center,
                    },
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefix_path, config, prefix_store, process_tracker) = init;

        // ── Tab controllers ──
        let general_ctrl = GeneralTabModel::builder()
            .launch(GeneralSettings {
                windows_version: None,
                audio_driver: None,
                log_pixels: None,
                virtual_desktop_enabled: false,
                virtual_desktop_width: 1024,
                virtual_desktop_height: 768,
            })
            .forward(sender.input_sender(), |msg| match msg {
                general_tab::GeneralTabOutput::SettingChanged(k, v) => {
                    RegistryEditorMsg::ApplySetting(k, v)
                }
            });

        let graphics_ctrl = GraphicsTabModel::builder()
            .launch(GraphicsSettings {
                renderer: None,
                csmt: None,
                offscreen_mode: None,
                video_memory: None,
            })
            .forward(sender.input_sender(), |msg| match msg {
                graphics_tab::GraphicsTabOutput::SettingChanged(k, v) => {
                    RegistryEditorMsg::ApplySetting(k, v)
                }
            });

        let platform_ctrl = PlatformTabModel::builder()
            .launch(PlatformSettings::default())
            .forward(sender.input_sender(), |msg| match msg {
                platform_tab::PlatformTabOutput::SettingChanged(k, v) => {
                    RegistryEditorMsg::ApplySetting(k, v)
                }
            });

        let model = RegistryEditorModel {
            prefix_path: prefix_path.clone(),
            config: config.clone(),
            registry_editor: None,
            editing: false,
            loading: false,
            pending_edit: false,
            general_ctrl,
            graphics_ctrl,
            platform_ctrl,
            prefix_store,
            process_tracker,
            watch_kill: None,
            tracker: 0,
        };

        let widgets = view_output!();

        sender.input(RegistryEditorMsg::LoadRegistry);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            RegistryEditorMsg::ToggleEdit => {
                if self.editing {
                    sender.input(RegistryEditorMsg::SaveRegistry);
                } else if self.registry_editor.is_none() {
                    self.pending_edit = true;
                    sender.input(RegistryEditorMsg::LoadForEdit);
                } else {
                    self.set_editing(true);
                    self.general_ctrl
                        .emit(general_tab::GeneralTabInput::SetEditing(true));
                    self.graphics_ctrl
                        .emit(graphics_tab::GraphicsTabInput::SetEditing(true));
                    self.platform_ctrl
                        .emit(platform_tab::PlatformTabInput::SetEditing(true));
                }
            }

            RegistryEditorMsg::LoadRegistry => {
                if !self.prefix_path.as_os_str().is_empty() {
                    let prefix_path = self.prefix_path.clone();
                    let prefix_path_str = prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    let warm = store.has_registry_cache(&prefix_path_str);
                    if warm {
                        let load = |sec: &str, key: &str| -> Option<String> {
                            store.get_setting(&prefix_path_str, sec, key).ok().flatten()
                        };
                        let load_dword = |sec: &str, key: &str| -> Option<u32> {
                            load(sec, key).and_then(|v| v.parse().ok())
                        };
                        load_settings_from_cache(&load, &load_dword, &sender);
                    } else {
                        self.set_loading(true);
                        self.registry_editor = None;
                        spawn_registry_load(
                            prefix_path,
                            prefix_path_str,
                            Arc::clone(&self.prefix_store),
                            false,
                            sender.clone(),
                        );
                    }
                }
            }

            RegistryEditorMsg::LoadForEdit => {
                if !self.prefix_path.as_os_str().is_empty() {
                    self.set_loading(true);
                    let prefix_path = self.prefix_path.clone();
                    let prefix_path_str = prefix_path.to_string_lossy().to_string();
                    let warm = self.prefix_store.has_registry_cache(&prefix_path_str);
                    spawn_registry_load(
                        prefix_path,
                        prefix_path_str,
                        Arc::clone(&self.prefix_store),
                        warm,
                        sender.clone(),
                    );
                }
            }

            RegistryEditorMsg::LoadSettings(general, graphics, platform) => {
                self.general_ctrl
                    .emit(general_tab::GeneralTabInput::LoadSettings(general));
                self.graphics_ctrl
                    .emit(graphics_tab::GraphicsTabInput::LoadSettings(graphics));
                self.platform_ctrl
                    .emit(platform_tab::PlatformTabInput::LoadSettings(platform));
            }

            RegistryEditorMsg::RegistryEditorLoaded(editor) => {
                self.registry_editor = Some(editor);
                self.loading = false;
                if self.pending_edit {
                    self.pending_edit = false;
                    self.set_editing(true);
                    self.general_ctrl
                        .emit(general_tab::GeneralTabInput::SetEditing(true));
                    self.graphics_ctrl
                        .emit(graphics_tab::GraphicsTabInput::SetEditing(true));
                    self.platform_ctrl
                        .emit(platform_tab::PlatformTabInput::SetEditing(true));
                }
            }

            RegistryEditorMsg::ApplySetting(section, setting) => {
                self.handle_apply_setting(section, setting, &sender);
            }

            RegistryEditorMsg::SaveRegistry => {
                if let Some(editor_arc) = self.registry_editor.take() {
                    let pp = self.prefix_path.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    let ec = editor_arc.clone();
                    tokio::spawn(async move {
                        let editor = ec.lock().await;
                        let result = editor.save_registry(&pp).await;
                        let _ = tx.send(result);
                    });
                    if let Ok(result) = rx.recv() {
                        if let Err(e) = result {
                            eprintln!("Registry save failed: {}", e);
                        }
                    }
                    self.registry_editor = Some(editor_arc);
                }
                self.set_editing(false);
                set_editing_all_tabs(
                    &self.general_ctrl,
                    &self.graphics_ctrl,
                    &self.platform_ctrl,
                    false,
                );
            }

            RegistryEditorMsg::CancelEdit => {
                self.set_editing(false);
                set_editing_all_tabs(
                    &self.general_ctrl,
                    &self.graphics_ctrl,
                    &self.platform_ctrl,
                    false,
                );
                let pp = self.prefix_path.to_string_lossy().to_string();
                let _ = self.prefix_store.invalidate_registry_cache(&pp);
                sender.input(RegistryEditorMsg::LoadRegistry);
            }

            RegistryEditorMsg::RunWinecfg => {
                let pp = self.prefix_path.clone();
                let track_path = pp.join("__wine_winecfg__");
                match std::process::Command::new("winecfg")
                    .env("WINEPREFIX", pp.to_string_lossy().as_ref())
                    .spawn()
                {
                    Ok(child) => {
                        println!("Launched winecfg");
                        self.process_tracker
                            .lock()
                            .unwrap()
                            .register(&track_path, child);
                    }
                    Err(e) => eprintln!("Failed to launch winecfg: {}", e),
                }
            }

            RegistryEditorMsg::RunRegedit => {
                let pp = self.prefix_path.clone();
                let track_path = pp.join("__wine_regedit__");
                match std::process::Command::new("wine")
                    .env("WINEPREFIX", pp.to_string_lossy().as_ref())
                    .arg("regedit")
                    .spawn()
                {
                    Ok(child) => {
                        println!("Launched regedit");
                        self.process_tracker
                            .lock()
                            .unwrap()
                            .register(&track_path, child);
                    }
                    Err(e) => eprintln!("Failed to launch regedit: {}", e),
                }
            }

            RegistryEditorMsg::RefreshReg => {
                let pp = self.prefix_path.to_string_lossy().to_string();
                let _ = self.prefix_store.invalidate_registry_cache(&pp);
                self.registry_editor = None;
                self.set_editing(false);
                set_editing_all_tabs(
                    &self.general_ctrl,
                    &self.graphics_ctrl,
                    &self.platform_ctrl,
                    false,
                );
                sender.input(RegistryEditorMsg::LoadRegistry);
            }

            RegistryEditorMsg::ConfigUpdated(config) => {
                self.set_config(config);
                self.set_editing(false);
                sender.input(RegistryEditorMsg::LoadRegistry);
            }

            RegistryEditorMsg::PrefixPathUpdated(path) => {
                let pp = path.clone();
                self.set_prefix_path(path);
                self.registry_editor = None;
                self.watch_kill = None;

                let s = sender.clone();
                let (kill_tx, kill_rx) = mpsc::channel::<()>();
                self.watch_kill = Some(kill_tx);

                std::thread::spawn(move || {
                    let (tx, rx) = mpsc::channel();
                    let mut watcher = match recommended_watcher(move |_| {
                        let _ = tx.send(());
                    }) {
                        Ok(w) => w,
                        Err(e) => {
                            eprintln!("watch init: {}", e);
                            return;
                        }
                    };
                    let _ = watcher.watch(&pp.join("system.reg"), RecursiveMode::NonRecursive);
                    let _ = watcher.watch(&pp.join("user.reg"), RecursiveMode::NonRecursive);
                    let _ = watcher.watch(&pp.join("userdef.reg"), RecursiveMode::NonRecursive);
                    loop {
                        match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                            Ok(_) => {
                                while rx
                                    .recv_timeout(std::time::Duration::from_millis(200))
                                    .is_ok()
                                {}
                                std::thread::sleep(std::time::Duration::from_millis(500));
                                let _ = s.input(RegistryEditorMsg::RefreshReg);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if kill_rx.try_recv().is_ok() {
                                    break;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                });
            }

            RegistryEditorMsg::RegistrySaveComplete => {
                self.set_editing(false);
                set_editing_all_tabs(
                    &self.general_ctrl,
                    &self.graphics_ctrl,
                    &self.platform_ctrl,
                    false,
                );
            }

            RegistryEditorMsg::RegistrySaveError(error) => {
                eprintln!("Registry save error: {}", error);
                self.set_editing(false);
                set_editing_all_tabs(
                    &self.general_ctrl,
                    &self.graphics_ctrl,
                    &self.platform_ctrl,
                    false,
                );
            }
        }
    }
}

// ── ApplySetting handler ─────────────────────────────────────────────────

impl RegistryEditorModel {
    fn handle_apply_setting(
        &mut self,
        section: String,
        setting: String,
        _sender: &ComponentSender<Self>,
    ) {
        if let Some(editor_arc) = &self.registry_editor {
            let ec = editor_arc.clone();
            let pp = self.prefix_path.to_string_lossy().to_string();
            let store = Arc::clone(&self.prefix_store);
            let section_c = section.clone();
            let setting_c = setting.clone();

            tokio::spawn(async move {
                match section_c.as_str() {
                    // ── General: Windows Version ──
                    "Software\\Wine" => {
                        let version = setting_c.strip_prefix("Version=").unwrap_or(&setting_c);
                        let mut editor = ec.lock().await;
                        let version = if version.is_empty() {
                            None
                        } else {
                            Some(version.to_string())
                        };
                        if let Some(ref v) = version {
                            let _ = editor.set_windows_version(v).await;
                        } else {
                            let _ = editor
                                .registry
                                .delete_value("Software\\Wine", "Version")
                                .await;
                        }
                        let _ = store.save_setting(
                            &pp,
                            "Software\\Wine",
                            "Version",
                            version.as_deref(),
                        );
                    }

                    // ── General: Audio Driver ──
                    "Software\\Wine\\Drivers\\Audio" => {
                        let mut editor = ec.lock().await;
                        let _ = editor.set_audio_driver(&setting_c).await;
                        let _ = store.save_setting(
                            &pp,
                            "Software\\Wine\\Drivers\\Audio",
                            "",
                            Some(&setting_c),
                        );
                    }

                    // ── General: DPI ──
                    "Control Panel\\Desktop" => {
                        if let Some(log_pixels) = setting_c.strip_prefix("LogPixels=") {
                            if let Ok(v) = log_pixels.parse::<u32>() {
                                let mut editor = ec.lock().await;
                                let dpi = DpiSettings {
                                    log_pixels: Some(v),
                                };
                                let _ = editor.set_dpi_settings(&dpi).await;
                                let _ = store.save_setting(
                                    &pp,
                                    "Control Panel\\Desktop",
                                    "LogPixels",
                                    Some(log_pixels),
                                );
                            }
                        }
                    }

                    // ── General: Virtual Desktop enabled ──
                    "Software\\Wine\\Explorer" => {
                        let mut editor = ec.lock().await;
                        let current = editor.get_virtual_desktop().await.ok().flatten().unwrap_or(
                            VirtualDesktopSettings {
                                enabled: false,
                                width: 1024,
                                height: 768,
                            },
                        );
                        let enabled = !setting_c.is_empty();
                        let updated = VirtualDesktopSettings {
                            enabled,
                            width: current.width,
                            height: current.height,
                        };
                        let _ = editor.set_virtual_desktop(&updated).await;
                        let _ = store.save_setting(
                            &pp,
                            "Software\\Wine\\Explorer",
                            "Desktop",
                            Some(if enabled { "Default" } else { "" }),
                        );
                        if enabled {
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\Explorer\\Desktops",
                                "Default",
                                Some(&format!("{}x{}", current.width, current.height)),
                            );
                        }
                    }

                    // ── General: Virtual Desktop size ──
                    "Software\\Wine\\Explorer\\Desktops" => {
                        if let Some(size) = setting_c.strip_prefix("Default=") {
                            if let Some((w_str, h_str)) = size.split_once('x') {
                                if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>())
                                {
                                    let mut editor = ec.lock().await;
                                    let current = editor
                                        .get_virtual_desktop()
                                        .await
                                        .ok()
                                        .flatten()
                                        .unwrap_or(VirtualDesktopSettings {
                                            enabled: false,
                                            width: 1024,
                                            height: 768,
                                        });
                                    let updated = VirtualDesktopSettings {
                                        enabled: current.enabled,
                                        width: w,
                                        height: h,
                                    };
                                    let _ = editor.set_virtual_desktop(&updated).await;
                                    let _ = store.save_setting(
                                        &pp,
                                        "Software\\Wine\\Explorer\\Desktops",
                                        "Default",
                                        Some(size),
                                    );
                                }
                            }
                        }
                    }

                    // ── Graphics: D3D settings ──
                    "Software\\Wine\\Direct3D" => {
                        let mut editor = ec.lock().await;
                        if let Some(renderer) = setting_c.strip_prefix("renderer=") {
                            if renderer.is_empty() {
                                let _ = editor
                                    .registry
                                    .delete_value("Software\\Wine\\Direct3D", "renderer")
                                    .await;
                            } else {
                                let _ = editor.set_d3d_renderer(renderer).await;
                            }
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\Direct3D",
                                "renderer",
                                if renderer.is_empty() {
                                    None
                                } else {
                                    Some(renderer)
                                },
                            );
                        } else if let Some(csmt) = setting_c.strip_prefix("csmt=") {
                            let v = csmt != "0";
                            let _ = editor.set_d3d_csmt(v).await;
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\Direct3D",
                                "csmt",
                                Some(csmt),
                            );
                        } else if let Some(mode) = setting_c.strip_prefix("OffscreenRenderingMode=")
                        {
                            if mode.is_empty() {
                                let _ = editor
                                    .registry
                                    .delete_value(
                                        "Software\\Wine\\Direct3D",
                                        "OffscreenRenderingMode",
                                    )
                                    .await;
                            } else {
                                let _ = editor.set_offscreen_rendering_mode(mode).await;
                            }
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\Direct3D",
                                "OffscreenRenderingMode",
                                if mode.is_empty() { None } else { Some(mode) },
                            );
                        } else if let Some(size) = setting_c.strip_prefix("VideoMemorySize=") {
                            if let Ok(v) = size.parse::<u32>() {
                                let _ = editor.set_video_memory_size(v).await;
                                let _ = store.save_setting(
                                    &pp,
                                    "Software\\Wine\\Direct3D",
                                    "VideoMemorySize",
                                    Some(size),
                                );
                            }
                        }
                    }

                    // ── Platform: Mac Driver ──
                    "Software\\Wine\\Mac Driver" => {
                        let mut editor = ec.lock().await;
                        if let Some((key, val)) = setting_c.split_once('=') {
                            if let Some(current) =
                                editor.get_mac_driver_settings().await.ok().flatten()
                            {
                                let mut updated = current;
                                let v = val == "Y";
                                match key {
                                    "AllowVerticalSync" => updated.allow_vertical_sync = Some(v),
                                    "CaptureDisplaysForFullscreen" => {
                                        updated.capture_displays_for_fullscreen = Some(v)
                                    }
                                    "UsePreciseScrolling" => {
                                        updated.use_precise_scrolling = Some(v)
                                    }
                                    "RetinaMode" => updated.retina_mode = Some(v),
                                    "LeftOptionIsAlt" => updated.left_option_is_alt = Some(v),
                                    "RightOptionIsAlt" => updated.right_option_is_alt = Some(v),
                                    "LeftCommandIsCtrl" => updated.left_command_is_ctrl = Some(v),
                                    "RightCommandIsCtrl" => updated.right_command_is_ctrl = Some(v),
                                    _ => {}
                                }
                                let _ = editor.set_mac_driver_settings(&updated).await;
                            } else {
                                let mut settings = MacDriverSettings::new();
                                let v = val == "Y";
                                match key {
                                    "AllowVerticalSync" => settings.allow_vertical_sync = Some(v),
                                    "CaptureDisplaysForFullscreen" => {
                                        settings.capture_displays_for_fullscreen = Some(v)
                                    }
                                    "UsePreciseScrolling" => {
                                        settings.use_precise_scrolling = Some(v)
                                    }
                                    "RetinaMode" => settings.retina_mode = Some(v),
                                    "LeftOptionIsAlt" => settings.left_option_is_alt = Some(v),
                                    "RightOptionIsAlt" => settings.right_option_is_alt = Some(v),
                                    "LeftCommandIsCtrl" => settings.left_command_is_ctrl = Some(v),
                                    "RightCommandIsCtrl" => {
                                        settings.right_command_is_ctrl = Some(v)
                                    }
                                    _ => {}
                                }
                                let _ = editor.set_mac_driver_settings(&settings).await;
                            }
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\Mac Driver",
                                key,
                                Some(val),
                            );
                        }
                    }

                    // ── Platform: X11 Driver ──
                    "Software\\Wine\\X11 Driver" => {
                        let mut editor = ec.lock().await;
                        if let Some((key, val)) = setting_c.split_once('=') {
                            if let Some(current) =
                                editor.get_x11_driver_settings().await.ok().flatten()
                            {
                                let mut updated = current;
                                let v = val == "Y";
                                match key {
                                    "Decorated" => updated.decorated = Some(v),
                                    "ClientSideGraphics" => updated.client_side_graphics = Some(v),
                                    "ClientSideWithRender" => {
                                        updated.client_side_with_render = Some(v)
                                    }
                                    "ClientSideAntiAliasWithRender" => {
                                        updated.client_side_antialias_with_render = Some(v)
                                    }
                                    "ClientSideAntiAliasWithCore" => {
                                        updated.client_side_antialias_with_core = Some(v)
                                    }
                                    "GrabFullscreen" => updated.grab_fullscreen = Some(v),
                                    "GrabPointer" => updated.grab_pointer = Some(v),
                                    "Managed" => updated.managed = Some(v),
                                    "UseXRandR" => updated.use_xrandr = Some(v),
                                    "UseXVidMode" => updated.use_xvid_mode = Some(v),
                                    _ => {}
                                }
                                let _ = editor.set_x11_driver_settings(&updated).await;
                            } else {
                                let mut settings = X11DriverSettings::new();
                                let v = val == "Y";
                                match key {
                                    "Decorated" => settings.decorated = Some(v),
                                    "ClientSideGraphics" => settings.client_side_graphics = Some(v),
                                    "ClientSideWithRender" => {
                                        settings.client_side_with_render = Some(v)
                                    }
                                    "ClientSideAntiAliasWithRender" => {
                                        settings.client_side_antialias_with_render = Some(v)
                                    }
                                    "ClientSideAntiAliasWithCore" => {
                                        settings.client_side_antialias_with_core = Some(v)
                                    }
                                    "GrabFullscreen" => settings.grab_fullscreen = Some(v),
                                    "GrabPointer" => settings.grab_pointer = Some(v),
                                    "Managed" => settings.managed = Some(v),
                                    "UseXRandR" => settings.use_xrandr = Some(v),
                                    "UseXVidMode" => settings.use_xvid_mode = Some(v),
                                    _ => {}
                                }
                                let _ = editor.set_x11_driver_settings(&settings).await;
                            }
                            let _ = store.save_setting(
                                &pp,
                                "Software\\Wine\\X11 Driver",
                                key,
                                Some(val),
                            );
                        }
                    }

                    _ => {}
                }
            });
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn set_editing_all_tabs(
    general: &Controller<GeneralTabModel>,
    graphics: &Controller<GraphicsTabModel>,
    platform: &Controller<PlatformTabModel>,
    editing: bool,
) {
    general.emit(general_tab::GeneralTabInput::SetEditing(editing));
    graphics.emit(graphics_tab::GraphicsTabInput::SetEditing(editing));
    platform.emit(platform_tab::PlatformTabInput::SetEditing(editing));
}

/// Load settings from cache and send to tabs (cache warm path).
fn load_settings_from_cache(
    load: &dyn Fn(&str, &str) -> Option<String>,
    load_dword: &dyn Fn(&str, &str) -> Option<u32>,
    sender: &ComponentSender<RegistryEditorModel>,
) {
    let load_vd = || -> Option<VirtualDesktopSettings> {
        let e = load("Software\\Wine\\Explorer", "Desktop")?;
        let sz = load("Software\\Wine\\Explorer\\Desktops", "Default")?;
        let size = DesktopSize::from_string(&sz).unwrap_or_else(|| DesktopSize::new(1024, 768));
        Some(VirtualDesktopSettings {
            enabled: !e.is_empty(),
            width: size.width,
            height: size.height,
        })
    };

    let general = GeneralSettings {
        windows_version: load("Software\\Wine", "Version"),
        audio_driver: load("Software\\Wine\\Drivers\\Audio", ""),
        log_pixels: load_dword("Control Panel\\Desktop", "LogPixels"),
        virtual_desktop_enabled: load_vd().map(|vd| vd.enabled).unwrap_or(false),
        virtual_desktop_width: load_vd().map(|vd| vd.width).unwrap_or(1024),
        virtual_desktop_height: load_vd().map(|vd| vd.height).unwrap_or(768),
    };

    let graphics = GraphicsSettings {
        renderer: load("Software\\Wine\\Direct3D", "renderer"),
        csmt: load_dword("Software\\Wine\\Direct3D", "csmt"),
        offscreen_mode: load("Software\\Wine\\Direct3D", "OffscreenRenderingMode"),
        video_memory: load_dword("Software\\Wine\\Direct3D", "VideoMemorySize"),
    };

    let x11_present = load("Software\\Wine\\X11 Driver", "Decorated").is_some();
    let platform = PlatformSettings {
        mac: {
            #[cfg(target_os = "macos")]
            {
                let has = load("Software\\Wine\\Mac Driver", "AllowVerticalSync").is_some();
                if has {
                    Some(MacSettings {
                        allow_vertical_sync: load(
                            "Software\\Wine\\Mac Driver",
                            "AllowVerticalSync",
                        )
                        .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        capture_displays: load(
                            "Software\\Wine\\Mac Driver",
                            "CaptureDisplaysForFullscreen",
                        )
                        .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        precise_scrolling: load(
                            "Software\\Wine\\Mac Driver",
                            "UsePreciseScrolling",
                        )
                        .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        retina_mode: load("Software\\Wine\\Mac Driver", "RetinaMode")
                            .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        left_option_alt: load("Software\\Wine\\Mac Driver", "LeftOptionIsAlt")
                            .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        right_option_alt: load("Software\\Wine\\Mac Driver", "RightOptionIsAlt")
                            .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        left_command_ctrl: load("Software\\Wine\\Mac Driver", "LeftCommandIsCtrl")
                            .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                        right_command_ctrl: load(
                            "Software\\Wine\\Mac Driver",
                            "RightCommandIsCtrl",
                        )
                        .map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                    })
                } else {
                    None
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                None
            }
        },
        x11: if x11_present {
            Some(X11Settings {
                decorated: load("Software\\Wine\\X11 Driver", "Decorated").map(|v| v != "N"),
                client_side_graphics: load("Software\\Wine\\X11 Driver", "ClientSideGraphics")
                    .map(|v| v != "N"),
                client_side_with_render: load("Software\\Wine\\X11 Driver", "ClientSideWithRender")
                    .map(|v| v != "N"),
                client_side_antialias_with_render: load(
                    "Software\\Wine\\X11 Driver",
                    "ClientSideAntiAliasWithRender",
                )
                .map(|v| v != "N"),
                client_side_antialias_with_core: load(
                    "Software\\Wine\\X11 Driver",
                    "ClientSideAntiAliasWithCore",
                )
                .map(|v| v != "N"),
                grab_fullscreen: load("Software\\Wine\\X11 Driver", "GrabFullscreen")
                    .map(|v| v == "Y"),
                grab_pointer: load("Software\\Wine\\X11 Driver", "GrabPointer").map(|v| v != "N"),
                managed: load("Software\\Wine\\X11 Driver", "Managed").map(|v| v != "N"),
                use_xrandr: load("Software\\Wine\\X11 Driver", "UseXRandR").map(|v| v != "N"),
                use_xvid_mode: load("Software\\Wine\\X11 Driver", "UseXVidMode").map(|v| v == "Y"),
            })
        } else {
            None
        },
    };

    sender.input(RegistryEditorMsg::LoadSettings(general, graphics, platform));
}

/// Background registry load (cold path): reads .reg files, caches, sends to tabs.
fn spawn_registry_load(
    prefix_path: PathBuf,
    prefix_path_str: String,
    store: Arc<prefix::PrefixStore>,
    warm: bool,
    sender: ComponentSender<RegistryEditorModel>,
) {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let result = async {
            let editor = RegistryEditor::with_prefix(
                Arc::new(InMemoryRegistryCache::with_default_ttl()),
                &prefix_path,
            )
            .await?;
            let windows_version = editor.get_windows_version().await?;
            let d3d_renderer = editor.get_d3d_renderer().await?;
            let d3d_csmt = editor.get_d3d_csmt().await?;
            let offscreen_rendering_mode = editor.get_offscreen_rendering_mode().await?;
            let video_memory_size = editor.get_video_memory_size().await?;
            let audio_driver = editor.get_audio_driver().await?;
            let virtual_desktop = editor.get_virtual_desktop().await?;
            let dpi_settings = editor.get_dpi_settings().await?;
            let x11_driver_settings = editor.get_x11_driver_settings().await?;
            #[cfg(target_os = "macos")]
            let mac_driver_settings = editor.get_mac_driver_settings().await?;

            let general = GeneralSettings {
                windows_version: windows_version.clone(),
                audio_driver: audio_driver.clone(),
                log_pixels: dpi_settings.as_ref().and_then(|d| d.log_pixels),
                virtual_desktop_enabled: virtual_desktop
                    .as_ref()
                    .map(|vd| vd.enabled)
                    .unwrap_or(false),
                virtual_desktop_width: virtual_desktop.as_ref().map(|vd| vd.width).unwrap_or(1024),
                virtual_desktop_height: virtual_desktop.as_ref().map(|vd| vd.height).unwrap_or(768),
            };

            let graphics = GraphicsSettings {
                renderer: d3d_renderer.clone(),
                csmt: d3d_csmt,
                offscreen_mode: offscreen_rendering_mode.clone(),
                video_memory: video_memory_size,
            };

            let mac = {
                #[cfg(target_os = "macos")]
                {
                    mac_driver_settings.as_ref().map(|m| MacSettings {
                        allow_vertical_sync: m.allow_vertical_sync,
                        capture_displays: m.capture_displays_for_fullscreen,
                        precise_scrolling: m.use_precise_scrolling,
                        retina_mode: m.retina_mode,
                        left_option_alt: m.left_option_is_alt,
                        right_option_alt: m.right_option_is_alt,
                        left_command_ctrl: m.left_command_is_ctrl,
                        right_command_ctrl: m.right_command_is_ctrl,
                    })
                }
                #[cfg(not(target_os = "macos"))]
                {
                    None
                }
            };

            let x11 = x11_driver_settings.as_ref().map(|x| X11Settings {
                decorated: x.decorated,
                client_side_graphics: x.client_side_graphics,
                client_side_with_render: x.client_side_with_render,
                client_side_antialias_with_render: x.client_side_antialias_with_render,
                client_side_antialias_with_core: x.client_side_antialias_with_core,
                grab_fullscreen: x.grab_fullscreen,
                grab_pointer: x.grab_pointer,
                managed: x.managed,
                use_xrandr: x.use_xrandr,
                use_xvid_mode: x.use_xvid_mode,
            });

            let platform = PlatformSettings { mac, x11 };

            Ok::<_, PrefixError>((editor, general, graphics, platform))
        }
        .await;
        let _ = tx.send(result);
    });

    let pp2 = prefix_path_str;
    tokio::spawn(async move {
        match rx.await {
            Ok(Ok((editor, general, graphics, platform))) => {
                if !warm {
                    let pp = &pp2;
                    macro_rules! save {
                        ($sec:expr, $key:expr, $val:expr) => {
                            let _ = store.save_setting(pp, $sec, $key, $val);
                        };
                    }
                    save!(
                        "Software\\Wine",
                        "Version",
                        general.windows_version.as_deref()
                    );
                    save!(
                        "Software\\Wine\\Direct3D",
                        "renderer",
                        graphics.renderer.as_deref()
                    );
                    save!(
                        "Software\\Wine\\Direct3D",
                        "csmt",
                        graphics.csmt.map(|v| v.to_string()).as_deref()
                    );
                    save!(
                        "Software\\Wine\\Direct3D",
                        "OffscreenRenderingMode",
                        graphics.offscreen_mode.as_deref()
                    );
                    save!(
                        "Software\\Wine\\Direct3D",
                        "VideoMemorySize",
                        graphics.video_memory.map(|v| v.to_string()).as_deref()
                    );
                    save!(
                        "Software\\Wine\\Drivers\\Audio",
                        "",
                        general.audio_driver.as_deref()
                    );
                    save!(
                        "Control Panel\\Desktop",
                        "LogPixels",
                        general.log_pixels.map(|v| v.to_string()).as_deref()
                    );
                    if let Some(x11) = &platform.x11 {
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "Decorated",
                            x11.decorated.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "ClientSideGraphics",
                            x11.client_side_graphics.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "ClientSideWithRender",
                            x11.client_side_with_render
                                .map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "ClientSideAntiAliasWithRender",
                            x11.client_side_antialias_with_render.map(|v| if v {
                                "Y"
                            } else {
                                "N"
                            })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "ClientSideAntiAliasWithCore",
                            x11.client_side_antialias_with_core
                                .map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "GrabFullscreen",
                            x11.grab_fullscreen.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "GrabPointer",
                            x11.grab_pointer.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "Managed",
                            x11.managed.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "UseXRandR",
                            x11.use_xrandr.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\X11 Driver",
                            "UseXVidMode",
                            x11.use_xvid_mode.map(|v| if v { "Y" } else { "N" })
                        );
                    }
                    #[cfg(target_os = "macos")]
                    if let Some(mac) = &platform.mac {
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "AllowVerticalSync",
                            mac.allow_vertical_sync.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "CaptureDisplaysForFullscreen",
                            mac.capture_displays.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "UsePreciseScrolling",
                            mac.precise_scrolling.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "RetinaMode",
                            mac.retina_mode.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "LeftOptionIsAlt",
                            mac.left_option_alt.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "RightOptionIsAlt",
                            mac.right_option_alt.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "LeftCommandIsCtrl",
                            mac.left_command_ctrl.map(|v| if v { "Y" } else { "N" })
                        );
                        save!(
                            "Software\\Wine\\Mac Driver",
                            "RightCommandIsCtrl",
                            mac.right_command_ctrl.map(|v| if v { "Y" } else { "N" })
                        );
                    }
                }
                sender.input(RegistryEditorMsg::LoadSettings(general, graphics, platform));
                sender.input(RegistryEditorMsg::RegistryEditorLoaded(Arc::new(
                    Mutex::new(editor),
                )));
            }
            Ok(Err(e)) => {
                eprintln!("Failed to load registry: {}", e);
            }
            Err(_) => {
                eprintln!("Failed to receive registry load result");
            }
        }
    });
}
