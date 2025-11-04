use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use crate::prefix::PrefixError;
use crate::prefix::config::PrefixConfig;
use crate::prefix::regeditor::{RegistryEditor, RegEditor};
use crate::prefix::regeditor::keys::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::prefix::regeditor::cache::InMemoryRegistryCache;
use tracker;

use super::{WindowsVersionModel, D3DModel, AudioModel, VirtualDesktopModel, MacDriverModel};

#[derive(Debug)]
#[tracker::track]
pub struct RegistryEditorModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    editing: bool,
    #[tracker::do_not_track]
    registry_editor: Option<Arc<Mutex<RegistryEditor>>>,
    // Tab component controllers
    #[tracker::do_not_track]
    pub windows_version_controller: Controller<WindowsVersionModel>,
    #[tracker::do_not_track]
    pub d3d_controller: Controller<D3DModel>,
    #[tracker::do_not_track]
    pub audio_controller: Controller<AudioModel>,
    #[tracker::do_not_track]
    pub virtual_desktop_controller: Controller<VirtualDesktopModel>,
    #[tracker::do_not_track]
    pub mac_driver_controller: Controller<MacDriverModel>,
}

#[derive(Debug)]
pub enum RegistryEditorMsg {
    ToggleEdit,
    SaveRegistry,
    LoadRegistry,
    ConfigUpdated(PrefixConfig),
    PrefixPathUpdated(PathBuf),
    // Messages from child components
    WindowsVersionUpdate(String),
    D3DUpdate {
        renderer: Option<String>,
        csmt: Option<bool>,
        offscreen_mode: Option<String>,
        video_memory: Option<String>,
    },
    AudioUpdate(String),
    VirtualDesktopUpdate {
        enabled: Option<bool>,
        width: Option<String>,
        height: Option<String>,
    },
    MacDriverUpdate {
        allow_vertical_sync: Option<bool>,
        capture_displays: Option<bool>,
        precise_scrolling: Option<bool>,
    },
}

#[relm4::component(pub)]
impl SimpleComponent for RegistryEditorModel {
    type Init = (PathBuf, PrefixConfig);
    type Input = RegistryEditorMsg;
    type Output = RegistryEditorMsg;
    type Widgets = RegistryEditorWidgets;

    view! {
        gtk::ScrolledWindow {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,

                #[name = "notebook"]
                gtk::Notebook {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_show_border: false,

                    // Windows Version tab
                    append_page: (
                        &{
                            model.windows_version_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("Windows Version").build())
                    ),

                    // Direct3D tab
                    append_page: (
                        &{
                            model.d3d_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("Direct3D").build())
                    ),

                    // Audio tab
                    append_page: (
                        &{
                            model.audio_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("Audio").build())
                    ),

                    // Virtual Desktop tab
                    append_page: (
                        &{
                            model.virtual_desktop_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("Virtual Desktop").build())
                    ),

                    // Mac Driver tab (only on macOS)
                    append_page: (
                        &{
                            model.mac_driver_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("Mac Driver").build())
                    ),
                },

                // Control buttons
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_halign: gtk::Align::End,
                    set_margin_top: 10,

                    gtk::Button {
                        #[track = "model.changed(RegistryEditorModel::editing())"]
                        set_label: if model.editing { "Save" } else { "Edit" },
                        #[track = "model.changed(RegistryEditorModel::editing())"]
                        add_css_class: if model.editing { "suggested-action" } else { "" },
                        connect_clicked => RegistryEditorMsg::ToggleEdit,
                    },

                    gtk::Button {
                        set_label: "Cancel",
                        #[track = "model.changed(RegistryEditorModel::editing())"]
                        set_visible: model.editing,
                        connect_clicked[sender, config = model.config.clone()] => move |_| {
                            sender.input(RegistryEditorMsg::ConfigUpdated(config.clone()));
                        },
                    },
                },
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefix_path, config) = init;
        
        // Create tab controllers
        let windows_version_controller = WindowsVersionModel::builder()
            .launch(None)
            .forward(sender.input_sender(), |msg| match msg {
                super::windows_version_tab::WindowsVersionMsg::UpdateWindowsVersion(version) => {
                    RegistryEditorMsg::WindowsVersionUpdate(version)
                }
                super::windows_version_tab::WindowsVersionMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::windows_version_tab::WindowsVersionMsg::SetWindowsVersion(_) => {
                    // Handle SetWindowsVersion message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let d3d_controller = D3DModel::builder()
            .launch((None, None, None, None))
            .forward(sender.input_sender(), |msg| match msg {
                super::d3d_tab::D3DMsg::UpdateD3DRenderer(renderer) => {
                    RegistryEditorMsg::D3DUpdate {
                        renderer: Some(renderer),
                        csmt: None,
                        offscreen_mode: None,
                        video_memory: None,
                    }
                }
                super::d3d_tab::D3DMsg::UpdateD3DCSMT(csmt) => {
                    RegistryEditorMsg::D3DUpdate {
                        renderer: None,
                        csmt: Some(csmt),
                        offscreen_mode: None,
                        video_memory: None,
                    }
                }
                super::d3d_tab::D3DMsg::UpdateOffscreenRenderingMode(mode) => {
                    RegistryEditorMsg::D3DUpdate {
                        renderer: None,
                        csmt: None,
                        offscreen_mode: Some(mode),
                        video_memory: None,
                    }
                }
                super::d3d_tab::D3DMsg::UpdateVideoMemorySize(size) => {
                    RegistryEditorMsg::D3DUpdate {
                        renderer: None,
                        csmt: None,
                        offscreen_mode: None,
                        video_memory: Some(size),
                    }
                }
                super::d3d_tab::D3DMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::d3d_tab::D3DMsg::SetD3DSettings { .. } => {
                    // Handle SetD3DSettings message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let audio_controller = AudioModel::builder()
            .launch(None)
            .forward(sender.input_sender(), |msg| match msg {
                super::audio_tab::AudioMsg::UpdateAudioDriver(driver) => {
                    RegistryEditorMsg::AudioUpdate(driver)
                }
                super::audio_tab::AudioMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::audio_tab::AudioMsg::SetAudioDriver(_) => {
                    // Handle SetAudioDriver message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let virtual_desktop_controller = VirtualDesktopModel::builder()
            .launch((false, 1024, 768))
            .forward(sender.input_sender(), |msg| match msg {
                super::virtual_desktop_tab::VirtualDesktopMsg::UpdateVirtualDesktop(enabled) => {
                    RegistryEditorMsg::VirtualDesktopUpdate {
                        enabled: Some(enabled),
                        width: None,
                        height: None,
                    }
                }
                super::virtual_desktop_tab::VirtualDesktopMsg::UpdateVirtualDesktopWidth(width) => {
                    RegistryEditorMsg::VirtualDesktopUpdate {
                        enabled: None,
                        width: Some(width),
                        height: None,
                    }
                }
                super::virtual_desktop_tab::VirtualDesktopMsg::UpdateVirtualDesktopHeight(height) => {
                    RegistryEditorMsg::VirtualDesktopUpdate {
                        enabled: None,
                        width: None,
                        height: Some(height),
                    }
                }
                super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::virtual_desktop_tab::VirtualDesktopMsg::SetVirtualDesktopSettings { .. } => {
                    // Handle SetVirtualDesktopSettings message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let mac_driver_controller = MacDriverModel::builder()
            .launch((None, None, None))
            .forward(sender.input_sender(), |msg| match msg {
                super::mac_driver_tab::MacDriverMsg::UpdateMacAllowVerticalSync(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: Some(enabled),
                        capture_displays: None,
                        precise_scrolling: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacCaptureDisplays(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: Some(enabled),
                        precise_scrolling: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacPreciseScrolling(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: Some(enabled),
                    }
                }
                super::mac_driver_tab::MacDriverMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::mac_driver_tab::MacDriverMsg::SetMacDriverSettings { .. } => {
                    // Handle SetMacDriverSettings message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let model = RegistryEditorModel {
            prefix_path: prefix_path.clone(),
            config: config.clone(),
            registry_editor: None,
            editing: false,
            windows_version_controller,
            d3d_controller,
            audio_controller,
            virtual_desktop_controller,
            mac_driver_controller,
            tracker: 0,
        };

        let widgets = view_output!();

        // Load initial registry values
        sender.input(RegistryEditorMsg::LoadRegistry);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            RegistryEditorMsg::ToggleEdit => {
                if self.editing {
                    println!("Saving editing");
                    // Save changes
                    sender.input(RegistryEditorMsg::SaveRegistry);
                } else {
                    self.set_editing(true);
                    println!("Setting to editing true");
                    
                    // Enable editing on all tab components
                    self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(true));
                    self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(true));
                    self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(true));
                    self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(true));
                    self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(true));
                }
            }
            RegistryEditorMsg::LoadRegistry => {
                // Always reload when LoadRegistry is called, especially when switching prefixes
                if !self.prefix_path.as_os_str().is_empty() {
                    // Reset registry editor to force reload
                    self.registry_editor = None;
                    
                    // Create new cache and editor each time to ensure fresh state
                    let cache = Arc::new(InMemoryRegistryCache::with_default_ttl());
                    let prefix_path = self.prefix_path.clone();
                    
                    // Use relm4's spawn_blocking to avoid blocking UI
                    let (tx, rx) = std::sync::mpsc::channel();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let result = rt.block_on(async {
                            println!("Loading registry values for prefix: {:?}", prefix_path);
                            let mut editor = RegistryEditor::with_prefix(cache, &prefix_path).await?;
                            
                            // Load all values in one go
                            let windows_version = editor.get_windows_version().await?;
                            let d3d_renderer = editor.get_d3d_renderer().await?;
                            let d3d_csmt = editor.get_d3d_csmt().await?;
                            let offscreen_rendering_mode = editor.get_offscreen_rendering_mode().await?;
                            let video_memory_size = editor.get_video_memory_size().await?;
                            let audio_driver = editor.get_audio_driver().await?;
                            let virtual_desktop = editor.get_virtual_desktop().await?;
                            
                            #[cfg(target_os = "macos")]
                            let mac_driver_settings = editor.get_mac_driver_settings().await?;
                            
                            let data = (
                                windows_version,
                                d3d_renderer,
                                d3d_csmt,
                                offscreen_rendering_mode,
                                video_memory_size,
                                audio_driver,
                                virtual_desktop,
                                #[cfg(target_os = "macos")]
                                mac_driver_settings,
                                #[cfg(not(target_os = "macos"))]
                                None,
                            );
                            
                            Ok::<(RegistryEditor, (Option<String>, Option<String>, Option<u32>, Option<String>, Option<u32>, Option<String>, Option<VirtualDesktopSettings>, Option<MacDriverSettings>)), PrefixError>((editor, data))
                        });
                        
                        let _ = tx.send(result);
                    });
                    
                    #[cfg(target_os = "macos")]
                    if let Ok(Ok((editor, (windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode, video_memory_size, audio_driver, virtual_desktop, mac_driver_settings)))) = rx.recv() {
                        // Update Windows Version tab
                        self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetWindowsVersion(windows_version));
                        
                        // Update D3D tab
                        self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetD3DSettings {
                            renderer: d3d_renderer,
                            csmt: d3d_csmt,
                            offscreen_mode: offscreen_rendering_mode,
                            video_memory: video_memory_size,
                        });
                        
                        // Update Audio tab
                        self.audio_controller.emit(super::audio_tab::AudioMsg::SetAudioDriver(audio_driver));
                        
                        // Update Virtual Desktop tab
                        if let Some(virtual_settings) = virtual_desktop {
                            self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetVirtualDesktopSettings {
                                enabled: virtual_settings.enabled,
                                width: virtual_settings.width,
                                height: virtual_settings.height,
                            });
                        }
                        
                        // Update Mac Driver tab
                        if let Some(mac_settings) = mac_driver_settings {
                            self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetMacDriverSettings {
                                allow_vertical_sync: mac_settings.allow_vertical_sync,
                                capture_displays: mac_settings.capture_displays_for_fullscreen,
                                precise_scrolling: mac_settings.use_precise_scrolling,
                            });
                        }
                        
                        // Wrap editor in Arc<Mutex<>> for thread-safe access
                        self.registry_editor = Some(Arc::new(Mutex::new(editor)));
                    }
                    
                    #[cfg(not(target_os = "macos"))]
                    if let Ok(Ok((editor, (windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode, video_memory_size, audio_driver, virtual_desktop, _mac_driver_settings)))) = rx.recv() {
                        // Update Windows Version tab
                        self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetWindowsVersion(windows_version));
                        
                        // Update D3D tab
                        self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetD3DSettings {
                            renderer: d3d_renderer,
                            csmt: d3d_csmt,
                            offscreen_mode: offscreen_rendering_mode,
                            video_memory: video_memory_size,
                        });
                        
                        // Update Audio tab
                        self.audio_controller.emit(super::audio_tab::AudioMsg::SetAudioDriver(audio_driver));
                        
                        // Update Virtual Desktop tab
                        if let Some(virtual_settings) = virtual_desktop {
                            self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetVirtualDesktopSettings {
                                enabled: virtual_settings.enabled,
                                width: virtual_settings.width,
                                height: virtual_settings.height,
                            });
                        }
                        
                        // Wrap editor in Arc<Mutex<>> for thread-safe access
                        self.registry_editor = Some(Arc::new(Mutex::new(editor)));
                    }
                }
            }
            RegistryEditorMsg::SaveRegistry => {
                println!("Saving registry values");
                // Save changes using Arc<Mutex<>> approach
                if let Some(editor_arc) = self.registry_editor.take() {
                    let prefix_path = self.prefix_path.clone();
                    
                    // Clone all the data we need to save
                    let (tx, rx) = std::sync::mpsc::channel();
                    
                    // Clone Arc before moving it into closure
                    let editor_arc_clone = editor_arc.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let result = rt.block_on(async {
                            // Lock the editor for the duration of the save operation
                            let mut editor = editor_arc_clone.lock().unwrap();
                            
                            // Save registry to file
                            if let Err(e) = editor.save_registry(&prefix_path).await {
                                eprintln!("Failed to save registry: {}", e);
                            }
                            
                            Ok::<(), PrefixError>(())
                        });
                        
                        let _ = tx.send(result);
                    });
                    
                    // Wait for save to complete
                    if let Ok(result) = rx.recv() {
                        if let Err(e) = result {
                            eprintln!("Registry save failed: {}", e);
                            // TODO: Show error dialog to user
                        } else {
                            println!("Registry saved successfully");
                        }
                    }
                    
                    // Put the editor back
                    self.registry_editor = Some(editor_arc);
                }
                
                self.set_editing(false);
                
                // Disable editing on all tab components
                self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(false));
                self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(false));
                self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(false));
                self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(false));
                self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(false));
            }
            RegistryEditorMsg::ConfigUpdated(config) => {
                self.set_config(config);
                self.set_editing(false);
                // Reload values
                sender.input(RegistryEditorMsg::LoadRegistry);
            }
            RegistryEditorMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path);
                self.registry_editor = None; // Reset to reload with new path
                sender.input(RegistryEditorMsg::LoadRegistry);
            }
            // Handle updates from child components
            RegistryEditorMsg::WindowsVersionUpdate(version) => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let version = version.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(async {
                            let mut editor = editor_arc_clone.lock().unwrap();
                            if let Err(e) = editor.set_windows_version(&version).await {
                                eprintln!("Failed to save Windows version: {}", e);
                            }
                        });
                    });
                }
            }
            RegistryEditorMsg::D3DUpdate { renderer, csmt, offscreen_mode, video_memory } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(async {
                            let mut editor = editor_arc_clone.lock().unwrap();
                            
                            // Save D3D renderer
                            if let Some(ref renderer) = renderer {
                                if let Err(e) = editor.set_d3d_renderer(renderer).await {
                                    eprintln!("Failed to save D3D renderer: {}", e);
                                }
                            }
                            
                            // Save CSMT
                            if let Some(csmt) = csmt {
                                if let Err(e) = editor.set_d3d_csmt(csmt).await {
                                    eprintln!("Failed to save CSMT: {}", e);
                                }
                            }
                            
                            // Save offscreen rendering mode
                            if let Some(ref mode) = offscreen_mode {
                                if let Err(e) = editor.set_offscreen_rendering_mode(mode).await {
                                    eprintln!("Failed to save offscreen rendering mode: {}", e);
                                }
                            }
                            
                            // Save video memory size
                            if let Some(size) = video_memory {
                                if let Ok(size) = size.parse::<u32>() {
                                    if let Err(e) = editor.set_video_memory_size(size).await {
                                        eprintln!("Failed to save video memory size: {}", e);
                                    }
                                }
                            }
                        });
                    });
                }
            }
            RegistryEditorMsg::AudioUpdate(driver) => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let driver = driver.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(async {
                            let mut editor = editor_arc_clone.lock().unwrap();
                            if let Err(e) = editor.set_audio_driver(&driver).await {
                                eprintln!("Failed to save audio driver: {}", e);
                            }
                        });
                    });
                }
            }
            RegistryEditorMsg::VirtualDesktopUpdate { enabled, width, height } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(async {
                            let mut editor = editor_arc_clone.lock().unwrap();
                            
                            // Save virtual desktop
                            let mut virtual_settings = VirtualDesktopSettings {
                                enabled: false,
                                width: 1024,
                                height: 768,
                            };
                            
                            if let Some(enabled) = enabled {
                                virtual_settings.enabled = enabled;
                            }
                            
                            if let Some(width) = width {
                                if let Ok(width) = width.parse::<u32>() {
                                    virtual_settings.width = width;
                                }
                            }
                            
                            if let Some(height) = height {
                                if let Ok(height) = height.parse::<u32>() {
                                    virtual_settings.height = height;
                                }
                            }
                            
                            if let Err(e) = editor.set_virtual_desktop(&virtual_settings).await {
                                eprintln!("Failed to save virtual desktop: {}", e);
                            }
                        });
                    });
                }
            }
            RegistryEditorMsg::MacDriverUpdate { allow_vertical_sync, capture_displays, precise_scrolling } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    
                    relm4::spawn_blocking(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(async {
                            let mut editor = editor_arc_clone.lock().unwrap();
                            
                            // Save Mac driver settings
                            let mut mac_settings = MacDriverSettings::new();
                            
                            if let Some(allow_vertical_sync) = allow_vertical_sync {
                                mac_settings.allow_vertical_sync = Some(allow_vertical_sync);
                            }
                            
                            if let Some(capture_displays) = capture_displays {
                                mac_settings.capture_displays_for_fullscreen = Some(capture_displays);
                            }
                            
                            if let Some(precise_scrolling) = precise_scrolling {
                                mac_settings.use_precise_scrolling = Some(precise_scrolling);
                            }
                            
                            if let Err(e) = editor.set_mac_driver_settings(&mac_settings).await {
                                eprintln!("Failed to save Mac driver settings: {}", e);
                            }
                        });
                    });
                }
            }
        }
    }
}