use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use crate::prefix::PrefixError;
use crate::prefix::config::PrefixConfig;
use crate::prefix::regeditor::{RegistryEditor, RegEditor};
use crate::prefix::regeditor::keys::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use crate::prefix::regeditor::cache::InMemoryRegistryCache;
use tracker;

use super::{WindowsVersionModel, D3DModel, AudioModel, VirtualDesktopModel, MacDriverModel, DpiModel, X11DriverModel};

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
    #[tracker::do_not_track]
    pub dpi_controller: Controller<DpiModel>,
    #[tracker::do_not_track]
    pub x11_driver_controller: Controller<X11DriverModel>,
}

#[derive(Debug)]
pub enum RegistryEditorMsg {
    ToggleEdit,
    SaveRegistry,
    LoadRegistry,
    RegistryEditorLoaded(Arc<Mutex<RegistryEditor>>),
    RegistryEditorUpdateTabs(Option<String>, Option<String>, Option<u32>, Option<String>, Option<u32>, Option<String>, Option<VirtualDesktopSettings>, Option<DpiSettings>, Option<X11DriverSettings>, Option<MacDriverSettings>),
    RegistrySaveComplete,
    RegistrySaveError(String),
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
        retina_mode: Option<bool>,
    },
    DpiUpdate {
        log_pixels: Option<u32>,
    },
    X11DriverUpdate {
        decorated: Option<bool>,
        client_side_graphics: Option<bool>,
        client_side_with_render: Option<bool>,
        client_side_antialias_with_render: Option<bool>,
        client_side_antialias_with_core: Option<bool>,
        grab_fullscreen: Option<bool>,
        grab_pointer: Option<bool>,
        managed: Option<bool>,
        use_xrandr: Option<bool>,
        use_xvid_mode: Option<bool>,
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

                    // DPI tab
                    append_page: (
                        &{
                            model.dpi_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("DPI").build())
                    ),

                    // X11 Driver tab
                    append_page: (
                        &{
                            model.x11_driver_controller.widget().clone().upcast::<gtk::Widget>()
                        },
                        Some(&gtk::Label::builder().label("X11 Driver").build())
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
            .launch((None, None, None, None))
            .forward(sender.input_sender(), |msg| match msg {
                super::mac_driver_tab::MacDriverMsg::UpdateMacAllowVerticalSync(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: Some(enabled),
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacCaptureDisplays(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: Some(enabled),
                        precise_scrolling: None,
                        retina_mode: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacPreciseScrolling(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: Some(enabled),
                        retina_mode: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacRetinaMode(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: Some(enabled),
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

        let dpi_controller = DpiModel::builder()
            .launch(None)
            .forward(sender.input_sender(), |msg| match msg {
                super::dpi_tab::DpiMsg::UpdateLogPixels(value_str) => {
                    if let Ok(value) = value_str.parse::<u32>() {
                        RegistryEditorMsg::DpiUpdate {
                            log_pixels: Some(value),
                        }
                    } else {
                        RegistryEditorMsg::ToggleEdit // Dummy message
                    }
                }
                super::dpi_tab::DpiMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::dpi_tab::DpiMsg::SetDpiSettings { log_pixels } => {
                    // Handle SetDpiSettings message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
            });

        let x11_driver_controller = X11DriverModel::builder()
            .launch((None, None, None, None, None, None, None, None, None, None, None))
            .forward(sender.input_sender(), |msg| match msg {
                super::x11_driver_tab::X11DriverMsg::UpdateDecorated(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: Some(enabled),
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateClientSideGraphics(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: Some(enabled),
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateClientSideWithRender(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: Some(enabled),
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateClientSideAntialiasWithRender(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: Some(enabled),
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateClientSideAntialiasWithCore(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: Some(enabled),
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateGrabFullscreen(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: Some(enabled),
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateGrabPointer(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: Some(enabled),
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateManaged(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: Some(enabled),
                        use_xrandr: None,
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateUseXRandR(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: Some(enabled),
                        use_xvid_mode: None,
                    }
                }
                super::x11_driver_tab::X11DriverMsg::UpdateUseXVidMode(enabled) => {
                    RegistryEditorMsg::X11DriverUpdate {
                        decorated: None,
                        client_side_graphics: None,
                        client_side_with_render: None,
                        client_side_antialias_with_render: None,
                        client_side_antialias_with_core: None,
                        grab_fullscreen: None,
                        grab_pointer: None,
                        managed: None,
                        use_xrandr: None,
                        use_xvid_mode: Some(enabled),
                    }
                }
                super::x11_driver_tab::X11DriverMsg::SetEditing(_) => {
                    // Handle SetEditing message but don't forward to parent
                    RegistryEditorMsg::ToggleEdit // Dummy message to satisfy compiler
                }
                super::x11_driver_tab::X11DriverMsg::SetX11DriverSettings { .. } => {
                    // Handle SetX11DriverSettings message but don't forward to parent
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
            dpi_controller,
            x11_driver_controller,
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
                    self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(true));
                    self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(true));
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
                    
                    // Use tokio::sync::oneshot channel for async communication
                    let (tx, rx) = oneshot::channel();
                    
                    tokio::spawn(async move {
                        let result = async move {
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
                            let dpi_settings = editor.get_dpi_settings().await?;
                            let x11_driver_settings = editor.get_x11_driver_settings().await?;
                            
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
                                dpi_settings,
                                x11_driver_settings,
                                #[cfg(target_os = "macos")]
                                mac_driver_settings,
                                #[cfg(not(target_os = "macos"))]
                                None,
                            );
                            
                            Ok::<(RegistryEditor, (Option<String>, Option<String>, Option<u32>, Option<String>, Option<u32>, Option<String>, Option<VirtualDesktopSettings>, Option<DpiSettings>, Option<X11DriverSettings>, Option<MacDriverSettings>)), PrefixError>((editor, data))
                        }.await;

                        let _ = tx.send(result);
                    });
                    
                    // Handle the result asynchronously without blocking UI
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok((editor, (windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode, video_memory_size, audio_driver, virtual_desktop, dpi_settings, x11_driver_settings, mac_driver_settings)))) => {
                                // Send message to update all tabs at once
                                sender_clone.input(RegistryEditorMsg::RegistryEditorUpdateTabs(
                                    windows_version,
                                    d3d_renderer,
                                    d3d_csmt,
                                    offscreen_rendering_mode,
                                    video_memory_size,
                                    audio_driver,
                                    virtual_desktop,
                                    dpi_settings,
                                    x11_driver_settings,
                                    mac_driver_settings,
                                ));
                                
                                // Send message to update the registry editor in the main thread
                                sender_clone.input(RegistryEditorMsg::RegistryEditorLoaded(Arc::new(Mutex::new(editor))));
                            }
                            Ok(Err(e)) => {
                                eprintln!("Failed to load registry: {}", e);
                                // TODO: Show error dialog to user
                            }
                            Err(_) => {
                                eprintln!("Failed to receive registry load result");
                                // TODO: Show error dialog to user
                            }
                        }
                    });
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
                    
                    tokio::spawn(async move {
                        let result = async move {
                            // Lock the editor for the duration of the save operation
                            let editor = editor_arc_clone.lock().await;
                            
                            // Save registry to file
                            editor.save_registry(&prefix_path).await
                        }.await;

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
                self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(false));
                self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(false));
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
            RegistryEditorMsg::RegistryEditorLoaded(editor) => {
                self.registry_editor = Some(editor);
            }
            RegistryEditorMsg::RegistryEditorUpdateTabs(windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode, video_memory_size, audio_driver, virtual_desktop, dpi_settings, x11_driver_settings, mac_driver_settings) => {
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
                
                // Update DPI tab
                if let Some(dpi) = dpi_settings {
                    self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetDpiSettings {
                        log_pixels: dpi.log_pixels,
                    });
                }
                
                // Update X11 Driver tab
                if let Some(x11) = x11_driver_settings {
                    self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetX11DriverSettings {
                        decorated: x11.decorated,
                        client_side_graphics: x11.client_side_graphics,
                        client_side_with_render: x11.client_side_with_render,
                        client_side_antialias_with_render: x11.client_side_antialias_with_render,
                        client_side_antialias_with_core: x11.client_side_antialias_with_core,
                        grab_fullscreen: x11.grab_fullscreen,
                        grab_pointer: x11.grab_pointer,
                        managed: x11.managed,
                        use_xrandr: x11.use_xrandr,
                        use_xvid_mode: x11.use_xvid_mode,
                    });
                }
                
                // Update Mac Driver tab
                if let Some(mac_settings) = mac_driver_settings {
                    self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetMacDriverSettings {
                        allow_vertical_sync: mac_settings.allow_vertical_sync,
                        capture_displays: mac_settings.capture_displays_for_fullscreen,
                        precise_scrolling: mac_settings.use_precise_scrolling,
                        retina_mode: mac_settings.retina_mode,
                    });
                }
            }
            RegistryEditorMsg::RegistrySaveComplete => {
                self.set_editing(false);
                
                // Disable editing on all tab components
                self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(false));
                self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(false));
                self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(false));
                self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(false));
                self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(false));
                self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(false));
                self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(false));
            }
            RegistryEditorMsg::RegistrySaveError(error) => {
                eprintln!("Registry save error: {}", error);
                // TODO: Show error dialog to user
                // For now, just disable editing mode
                self.set_editing(false);
                
                // Disable editing on all tab components
                self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(false));
                self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(false));
                self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(false));
                self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(false));
                self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(false));
                self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(false));
                self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(false));
            }
            // Handle updates from child components
            RegistryEditorMsg::WindowsVersionUpdate(version) => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let version = version.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            editor.set_windows_version(&version).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("Windows version updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save Windows version: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save Windows version: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::D3DUpdate { renderer, csmt, offscreen_mode, video_memory } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            
                            // Save D3D renderer
                            if let Some(ref renderer) = renderer {
                                editor.set_d3d_renderer(renderer).await?;
                            }
                            
                            // Save CSMT
                            if let Some(csmt) = csmt {
                                editor.set_d3d_csmt(csmt).await?;
                            }
                            
                            // Save offscreen rendering mode
                            if let Some(ref mode) = offscreen_mode {
                                editor.set_offscreen_rendering_mode(mode).await?;
                            }
                            
                            // Save video memory size
                            if let Some(size) = video_memory {
                                if let Ok(size) = size.parse::<u32>() {
                                    editor.set_video_memory_size(size).await?;
                                }
                            }
                            
                            Ok::<(), PrefixError>(())
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("D3D settings updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save D3D settings: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save D3D settings: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::AudioUpdate(driver) => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let driver = driver.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            editor.set_audio_driver(&driver).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("Audio driver updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save audio driver: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save audio driver: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::VirtualDesktopUpdate { enabled, width, height } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            
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
                            
                            editor.set_virtual_desktop(&virtual_settings).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("Virtual desktop updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save virtual desktop: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save virtual desktop: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::MacDriverUpdate { allow_vertical_sync, capture_displays, precise_scrolling, retina_mode } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            
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
                            
                            if let Some(retina_mode) = retina_mode {
                                mac_settings.retina_mode = Some(retina_mode);
                            }
                            
                            editor.set_mac_driver_settings(&mac_settings).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("Mac driver settings updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save Mac driver settings: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save Mac driver settings: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::DpiUpdate { log_pixels } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            
                            // Get current DPI settings and update
                            let mut dpi_settings = if let Ok(Some(settings)) = editor.get_dpi_settings().await {
                                settings
                            } else {
                                DpiSettings { log_pixels: None }
                            };
                            
                            if let Some(pixels) = log_pixels {
                                dpi_settings.log_pixels = Some(pixels);
                            }
                            
                            // Save updated DPI settings
                            editor.set_dpi_settings(&dpi_settings).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("DPI settings updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save DPI settings: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save DPI settings: {}", e)));
                            }
                        }
                    });
                }
            }
            RegistryEditorMsg::X11DriverUpdate { decorated, client_side_graphics, client_side_with_render, client_side_antialias_with_render, client_side_antialias_with_core, grab_fullscreen, grab_pointer, managed, use_xrandr, use_xvid_mode } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    
                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            
                            // Get current X11 driver settings and update incrementally
                            let mut x11_settings = if let Ok(Some(settings)) = editor.get_x11_driver_settings().await {
                                settings
                            } else {
                                X11DriverSettings::new()
                            };
                            
                            // Update only the fields that were changed
                            if let Some(val) = decorated {
                                x11_settings.decorated = Some(val);
                            }
                            if let Some(val) = client_side_graphics {
                                x11_settings.client_side_graphics = Some(val);
                            }
                            if let Some(val) = client_side_with_render {
                                x11_settings.client_side_with_render = Some(val);
                            }
                            if let Some(val) = client_side_antialias_with_render {
                                x11_settings.client_side_antialias_with_render = Some(val);
                            }
                            if let Some(val) = client_side_antialias_with_core {
                                x11_settings.client_side_antialias_with_core = Some(val);
                            }
                            if let Some(val) = grab_fullscreen {
                                x11_settings.grab_fullscreen = Some(val);
                            }
                            if let Some(val) = grab_pointer {
                                x11_settings.grab_pointer = Some(val);
                            }
                            if let Some(val) = managed {
                                x11_settings.managed = Some(val);
                            }
                            if let Some(val) = use_xrandr {
                                x11_settings.use_xrandr = Some(val);
                            }
                            if let Some(val) = use_xvid_mode {
                                x11_settings.use_xvid_mode = Some(val);
                            }
                            
                            // Save updated X11 driver settings
                            editor.set_x11_driver_settings(&x11_settings).await
                        }.await;
                        
                        match result {
                            Ok(()) => {
                                println!("X11 driver settings updated successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to save X11 driver settings: {}", e);
                                sender_clone.input(RegistryEditorMsg::RegistrySaveError(format!("Failed to save X11 driver settings: {}", e)));
                            }
                        }
                    });
                }
            }
        }
    }
}