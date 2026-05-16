use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use crate::prefix::PrefixError;
use crate::prefix::config::PrefixConfig;
use crate::prefix::regeditor::{RegistryEditor, RegEditor};
use crate::prefix::regeditor::keys::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use tokio::sync::{Mutex, oneshot};
use crate::prefix::regeditor::cache::InMemoryRegistryCache;
use notify::{Watcher, RecursiveMode, recommended_watcher};
use tracker;

use super::{WindowsVersionModel, D3DModel, AudioModel, VirtualDesktopModel, MacDriverModel, DpiModel, X11DriverModel};

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
    #[tracker::do_not_track]
    prefix_store: Arc<crate::prefix::PrefixStore>,
    #[tracker::do_not_track]
    watch_kill: Option<mpsc::Sender<()>>,
}

#[derive(Debug)]
pub enum RegistryEditorMsg {
    ToggleEdit,
    SaveRegistry,
    LoadRegistry,
    LoadForEdit, // load .reg for editing (skip cache)
    RegistryEditorLoaded(Arc<Mutex<RegistryEditor>>),
    RegistryEditorUpdateTabs(Option<String>, Option<String>, Option<u32>, Option<String>, Option<u32>, Option<String>, Option<VirtualDesktopSettings>, Option<DpiSettings>, Option<X11DriverSettings>, Option<MacDriverSettings>),
    RegistrySaveComplete,
    RegistrySaveError(String),
    ConfigUpdated(PrefixConfig),
    CancelEdit,
    RunWinecfg,
    RunRegedit,
    RefreshReg,
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
        left_option_alt: Option<bool>,
        right_option_alt: Option<bool>,
        left_command_ctrl: Option<bool>,
        right_command_ctrl: Option<bool>,
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
    type Init = (PathBuf, PrefixConfig, Arc<crate::prefix::PrefixStore>);
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
                                &{
                                    let s = gtk::ScrolledWindow::builder()
                                        .vexpand(true).hexpand(true).build();
                                    s.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                                    let b = gtk::Box::builder()
                                        .orientation(gtk::Orientation::Vertical)
                                        .spacing(0)
                                        .hexpand(true)
                                        .build();
                                    b.append(&model.windows_version_controller.widget().clone());
                                    b.append(&model.audio_controller.widget().clone());
                                    b.append(&model.dpi_controller.widget().clone());
                                    s.set_child(Some(&b));
                                    s.upcast::<gtk::Widget>()
                                },
                                Some(&gtk::Label::builder().label("General").build())
                            ),

                            append_page: (
                                &{
                                    let s = gtk::ScrolledWindow::builder()
                                        .vexpand(true).hexpand(true).build();
                                    s.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                                    let b = gtk::Box::builder()
                                        .orientation(gtk::Orientation::Vertical)
                                        .spacing(0)
                                        .hexpand(true)
                                        .build();
                                    b.append(&model.d3d_controller.widget().clone());
                                    #[cfg(not(target_os = "macos"))]
                                    b.append(&model.virtual_desktop_controller.widget().clone());
                                    s.set_child(Some(&b));
                                    s.upcast::<gtk::Widget>()
                                },
                                Some(&gtk::Label::builder().label("Graphics").build())
                            ),

                            append_page: (
                                &{
                                    let s = gtk::ScrolledWindow::builder()
                                        .vexpand(true).hexpand(true).build();
                                    s.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                                    let b = gtk::Box::builder()
                                        .orientation(gtk::Orientation::Vertical)
                                        .spacing(0)
                                        .hexpand(true)
                                        .build();
                                    b.append(&model.mac_driver_controller.widget().clone());
                                    b.append(&model.x11_driver_controller.widget().clone());
                                    s.set_child(Some(&b));
                                    s.upcast::<gtk::Widget>()
                                },
                                Some(&gtk::Label::builder().label("Platform").build())
                            ),
                        },

                    // Control buttons — fixed at bottom
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 10,
                        set_margin_all: 10,

                        gtk::Button {
                            set_label: "winecfg",
                            set_tooltip_text: Some("Launch Wine Configuration for this prefix"),
                            connect_clicked => RegistryEditorMsg::RunWinecfg,
                        },

                        gtk::Button {
                            set_label: "regedit",
                            set_tooltip_text: Some("Launch Wine Registry Editor for this prefix"),
                            connect_clicked => RegistryEditorMsg::RunRegedit,
                        },

                        gtk::Button {
                            set_label: "Refresh",
                            set_tooltip_text: Some("Reload registry from disk"),
                            connect_clicked => RegistryEditorMsg::RefreshReg,
                        },

                        gtk::Box {
                            set_hexpand: true,
                        },

                        gtk::Button {
                            #[watch]
                            set_label: if model.editing { "Save" } else { "Edit" },
                            #[watch]
                            add_css_class: if model.editing { "suggested-action" } else { "" },
                            connect_clicked => RegistryEditorMsg::ToggleEdit,
                        },

                        gtk::Button {
                            set_label: "Cancel",
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
        let (prefix_path, config, prefix_store) = init;
        
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
            .launch((None, None, None, None, None, None, None, None))
            .forward(sender.input_sender(), |msg| match msg {
                super::mac_driver_tab::MacDriverMsg::UpdateMacAllowVerticalSync(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: Some(enabled),
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacCaptureDisplays(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: Some(enabled),
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacPreciseScrolling(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: Some(enabled),
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacRetinaMode(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: Some(enabled),
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacLeftOptionAlt(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: Some(enabled),
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacRightOptionAlt(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: Some(enabled),
                        left_command_ctrl: None,
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacLeftCommandCtrl(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: Some(enabled),
                        right_command_ctrl: None,
                    }
                }
                super::mac_driver_tab::MacDriverMsg::UpdateMacRightCommandCtrl(enabled) => {
                    RegistryEditorMsg::MacDriverUpdate {
                        allow_vertical_sync: None,
                        capture_displays: None,
                        precise_scrolling: None,
                        retina_mode: None,
                        left_option_alt: None,
                        right_option_alt: None,
                        left_command_ctrl: None,
                        right_command_ctrl: Some(enabled),
                    }
                }
                super::mac_driver_tab::MacDriverMsg::SetEditing(_) => {
                    RegistryEditorMsg::ToggleEdit
                }
                super::mac_driver_tab::MacDriverMsg::SetMacDriverSettings { .. } => {
                    RegistryEditorMsg::ToggleEdit
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
            loading: false,
            pending_edit: false,
            windows_version_controller,
            d3d_controller,
            audio_controller,
            virtual_desktop_controller,
            mac_driver_controller,
            dpi_controller,
            x11_driver_controller,
            prefix_store,
            watch_kill: None,
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
                    sender.input(RegistryEditorMsg::SaveRegistry);
                } else if self.registry_editor.is_none() {
                    // Load .reg first, then enter edit mode
                    println!("Need to load registry for editing...");
                    self.pending_edit = true;
                    sender.input(RegistryEditorMsg::LoadForEdit);
                } else {
                    self.set_editing(true);
                    println!("Setting to editing true");
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
                if !self.prefix_path.as_os_str().is_empty() {
                    let prefix_path = self.prefix_path.clone();
                    let prefix_path_str = prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    // Fast path: try SQLite cache first for instant tab population
                    let warm = store.has_registry_cache(&prefix_path_str);
                    if warm {
                        let load_from_cache = |section: &str, key: &str| -> Option<String> {
                            store.get_setting(&prefix_path_str, section, key).ok().flatten()
                        };
                        let load_dword = |section: &str, key: &str| -> Option<u32> {
                            load_from_cache(section, key).and_then(|v| v.parse().ok())
                        };
                        let load_vd = || -> Option<VirtualDesktopSettings> {
                            let e = load_from_cache("Software\\Wine\\Explorer", "Desktop")?;
                            let sz = load_from_cache("Software\\Wine\\Explorer\\Desktops", "Default")?;
                            let size = crate::prefix::regeditor::keys::DesktopSize::from_string(&sz)
                                .unwrap_or_else(|| crate::prefix::regeditor::keys::DesktopSize::new(1024, 768));
                            Some(VirtualDesktopSettings { enabled: !e.is_empty(), width: size.width, height: size.height })
                        };
                        sender.input(RegistryEditorMsg::RegistryEditorUpdateTabs(
                            load_from_cache("Software\\Wine", "Version"),
                            load_from_cache("Software\\Wine\\Direct3D", "renderer"),
                            load_dword("Software\\Wine\\Direct3D", "csmt"),
                            load_from_cache("Software\\Wine\\Direct3D", "OffscreenRenderingMode"),
                            load_dword("Software\\Wine\\Direct3D", "VideoMemorySize"),
                            load_from_cache("Software\\Wine\\Drivers\\Audio", ""),
                            load_vd(),
                            Some(DpiSettings { log_pixels: load_dword("Control Panel\\Desktop", "LogPixels") }),
                            {
                                let has = store.get_setting(&prefix_path_str, "Software\\Wine\\X11 Driver", "Decorated").ok().flatten();
                                has.map(|_| X11DriverSettings {
                                    decorated: load_from_cache("Software\\Wine\\X11 Driver", "Decorated").map(|v| v != "N"),
                                    client_side_graphics: load_from_cache("Software\\Wine\\X11 Driver", "ClientSideGraphics").map(|v| v != "N"),
                                    client_side_with_render: load_from_cache("Software\\Wine\\X11 Driver", "ClientSideWithRender").map(|v| v != "N"),
                                    client_side_antialias_with_render: load_from_cache("Software\\Wine\\X11 Driver", "ClientSideAntiAliasWithRender").map(|v| v != "N"),
                                    client_side_antialias_with_core: load_from_cache("Software\\Wine\\X11 Driver", "ClientSideAntiAliasWithCore").map(|v| v != "N"),
                                    grab_fullscreen: load_from_cache("Software\\Wine\\X11 Driver", "GrabFullscreen").map(|v| v == "Y"),
                                    grab_pointer: load_from_cache("Software\\Wine\\X11 Driver", "GrabPointer").map(|v| v != "N"),
                                    managed: load_from_cache("Software\\Wine\\X11 Driver", "Managed").map(|v| v != "N"),
                                    use_xrandr: load_from_cache("Software\\Wine\\X11 Driver", "UseXRandR").map(|v| v != "N"),
                                    use_xvid_mode: load_from_cache("Software\\Wine\\X11 Driver", "UseXVidMode").map(|v| v == "Y"),
                                })
                            },
                            {
                                #[cfg(target_os = "macos")] {
                                    let has = store.get_setting(&prefix_path_str, "Software\\Wine\\Mac Driver", "AllowVerticalSync").ok().flatten();
                                    has.map(|_| MacDriverSettings {
                                        allow_vertical_sync: load_from_cache("Software\\Wine\\Mac Driver", "AllowVerticalSync").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        capture_displays_for_fullscreen: load_from_cache("Software\\Wine\\Mac Driver", "CaptureDisplaysForFullscreen").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        use_precise_scrolling: load_from_cache("Software\\Wine\\Mac Driver", "UsePreciseScrolling").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        retina_mode: load_from_cache("Software\\Wine\\Mac Driver", "RetinaMode").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        windows_float_when_inactive: None,
                                        left_option_is_alt: load_from_cache("Software\\Wine\\Mac Driver", "LeftOptionIsAlt").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        right_option_is_alt: load_from_cache("Software\\Wine\\Mac Driver", "RightOptionIsAlt").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        left_command_is_ctrl: load_from_cache("Software\\Wine\\Mac Driver", "LeftCommandIsCtrl").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                        right_command_is_ctrl: load_from_cache("Software\\Wine\\Mac Driver", "RightCommandIsCtrl").map(|v| matches!(v.as_str(), "Y" | "y" | "T" | "t" | "1")),
                                    })
                                }
                                #[cfg(not(target_os = "macos"))] { None }
                            },
                        ));
                    } else {
                        self.set_loading(true);
                    }

                    if !warm {
                        // Cold: load .reg in background
                        self.registry_editor = None;
                        spawn_registry_load(prefix_path, prefix_path_str, Arc::clone(&self.prefix_store), warm, sender.clone());
                    }
                }
            }
            RegistryEditorMsg::LoadForEdit => {
                if !self.prefix_path.as_os_str().is_empty() {
                    self.set_loading(true);
                    let prefix_path = self.prefix_path.clone();
                    let prefix_path_str = prefix_path.to_string_lossy().to_string();
                    let warm = self.prefix_store.has_registry_cache(&prefix_path_str);
                    spawn_registry_load(prefix_path, prefix_path_str, Arc::clone(&self.prefix_store), warm, sender.clone());
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
            RegistryEditorMsg::CancelEdit => {
                self.set_editing(false);
                self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(false));
                self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(false));
                self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(false));
                self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(false));
                self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(false));
                self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(false));
                self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(false));
                // Invalidate cache so LoadRegistry reads fresh from .reg
                let pp = self.prefix_path.to_string_lossy().to_string();
                let _ = self.prefix_store.invalidate_registry_cache(&pp);
                sender.input(RegistryEditorMsg::LoadRegistry);
            }
            RegistryEditorMsg::RunWinecfg => {
                let pp = self.prefix_path.clone();
                let _ = std::process::Command::new("winecfg")
                    .env("WINEPREFIX", pp.to_string_lossy().as_ref())
                    .spawn()
                    .map_err(|e| eprintln!("Failed to launch winecfg: {}", e));
            }
            RegistryEditorMsg::RunRegedit => {
                let pp = self.prefix_path.clone();
                let _ = std::process::Command::new("wine")
                    .env("WINEPREFIX", pp.to_string_lossy().as_ref())
                    .arg("regedit")
                    .spawn()
                    .map_err(|e| eprintln!("Failed to launch regedit: {}", e));
            }
            RegistryEditorMsg::RefreshReg => {
                let pp = self.prefix_path.to_string_lossy().to_string();
                let _ = self.prefix_store.invalidate_registry_cache(&pp);
                self.registry_editor = None;
                self.set_editing(false);
                self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(false));
                self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(false));
                self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(false));
                self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(false));
                self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(false));
                self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(false));
                self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(false));
                sender.input(RegistryEditorMsg::LoadRegistry);
            }
            RegistryEditorMsg::ConfigUpdated(config) => {
                self.set_config(config);
                self.set_editing(false);
                // Reload values
                sender.input(RegistryEditorMsg::LoadRegistry);
            }
            RegistryEditorMsg::PrefixPathUpdated(path) => {
                let pp = path.clone();
                self.set_prefix_path(path);
                self.registry_editor = None; // Reset, ConfigUpdated will trigger the reload

                // Stop old file watcher
                self.watch_kill = None;

                // Start watching registry files for the new prefix
                let s = sender.clone();
                let (kill_tx, kill_rx) = mpsc::channel::<()>();
                self.watch_kill = Some(kill_tx);

                std::thread::spawn(move || {
                    let (tx, rx) = mpsc::channel();
                    let mut watcher = match recommended_watcher(move |_| {
                        let _ = tx.send(());
                    }) {
                        Ok(w) => w,
                        Err(e) => { eprintln!("watch init: {}", e); return; }
                    };

                    let _ = watcher.watch(&pp.join("system.reg"), RecursiveMode::NonRecursive);
                    let _ = watcher.watch(&pp.join("user.reg"), RecursiveMode::NonRecursive);
                    let _ = watcher.watch(&pp.join("userdef.reg"), RecursiveMode::NonRecursive);

                    loop {
                        match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                            Ok(_) => {
                                // drain pending events
                                while rx.recv_timeout(std::time::Duration::from_millis(200)).is_ok() {}
                                // wait for files to be fully flushed before refresh
                                std::thread::sleep(std::time::Duration::from_millis(500));
                                let _ = s.input(RegistryEditorMsg::RefreshReg);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if kill_rx.try_recv().is_ok() { break; }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    // watcher dropped here — unwatches
                });
            }
            RegistryEditorMsg::RegistryEditorLoaded(editor) => {
                self.registry_editor = Some(editor);
                self.loading = false;
                if self.pending_edit {
                    self.pending_edit = false;
                    self.set_editing(true);
                    self.windows_version_controller.emit(super::windows_version_tab::WindowsVersionMsg::SetEditing(true));
                    self.d3d_controller.emit(super::d3d_tab::D3DMsg::SetEditing(true));
                    self.audio_controller.emit(super::audio_tab::AudioMsg::SetEditing(true));
                    self.virtual_desktop_controller.emit(super::virtual_desktop_tab::VirtualDesktopMsg::SetEditing(true));
                    self.mac_driver_controller.emit(super::mac_driver_tab::MacDriverMsg::SetEditing(true));
                    self.dpi_controller.emit(super::dpi_tab::DpiMsg::SetEditing(true));
                    self.x11_driver_controller.emit(super::x11_driver_tab::X11DriverMsg::SetEditing(true));
                }
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
                #[cfg(not(target_os = "macos"))]
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
                        left_option_alt: mac_settings.left_option_is_alt,
                        right_option_alt: mac_settings.right_option_is_alt,
                        left_command_ctrl: mac_settings.left_command_is_ctrl,
                        right_command_ctrl: mac_settings.right_command_is_ctrl,
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result: Result<(), PrefixError> = if version.is_empty() {
                            async {
                                let editor = editor_arc_clone.lock().await;
                                editor.registry.delete_value("Software\\Wine", "Version").await
                            }.await
                        } else {
                            async {
                                let mut editor = editor_arc_clone.lock().await;
                                editor.set_windows_version(&version).await
                            }.await
                        };

                        match result {
                            Ok(()) => {
                                let val = if version.is_empty() { None } else { Some(version.as_str()) };
                                let _ = store.save_setting(&pp, "Software\\Wine", "Version", val);
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            if let Some(ref renderer) = renderer {
                                if renderer.is_empty() {
                                    editor.registry.delete_value("Software\\Wine\\Direct3D", "renderer").await?;
                                } else {
                                    editor.set_d3d_renderer(renderer).await?;
                                }
                            }
                            if let Some(csmt) = csmt { editor.set_d3d_csmt(csmt).await?; }
                            if let Some(ref mode) = offscreen_mode {
                                if mode.is_empty() {
                                    editor.registry.delete_value("Software\\Wine\\Direct3D", "OffscreenRenderingMode").await?;
                                } else {
                                    editor.set_offscreen_rendering_mode(mode).await?;
                                }
                            }
                            if let Some(ref size) = video_memory {
                                if let Ok(size) = size.parse::<u32>() { editor.set_video_memory_size(size).await?; }
                            }
                            Ok::<(), PrefixError>(())
                        }.await;

                        match result {
                            Ok(()) => {
                                let r_val = renderer.as_deref().filter(|v| !v.is_empty());
                                let o_val = offscreen_mode.as_deref().filter(|v| !v.is_empty());
                                let _ = store.save_setting(&pp, "Software\\Wine\\Direct3D", "renderer", r_val);
                                let _ = store.save_setting(&pp, "Software\\Wine\\Direct3D", "csmt", csmt.map(|v| v.to_string()).as_deref());
                                let _ = store.save_setting(&pp, "Software\\Wine\\Direct3D", "OffscreenRenderingMode", o_val);
                                let _ = store.save_setting(&pp, "Software\\Wine\\Direct3D", "VideoMemorySize", video_memory.as_deref());
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            editor.set_audio_driver(&driver).await
                        }.await;

                        match result {
                            Ok(()) => {
                                let _ = store.save_setting(&pp, "Software\\Wine\\Drivers\\Audio", "", Some(&driver));
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);
                    let width_c = width.clone();
                    let height_c = height.clone();
                    let enabled_c = enabled;

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            let mut virtual_settings = VirtualDesktopSettings { enabled: false, width: 1024, height: 768 };
                            if let Some(e) = enabled_c { virtual_settings.enabled = e; }
                            if let Some(w) = &width_c { if let Ok(w) = w.parse::<u32>() { virtual_settings.width = w; } }
                            if let Some(h) = &height_c { if let Ok(h) = h.parse::<u32>() { virtual_settings.height = h; } }
                            editor.set_virtual_desktop(&virtual_settings).await
                        }.await;

                        match result {
                            Ok(()) => {
                                let w = width_c.unwrap_or_else(|| "1024".to_string());
                                let h = height_c.unwrap_or_else(|| "768".to_string());
                                let en = enabled_c.unwrap_or(false);
                                let _ = store.save_setting(&pp, "Software\\Wine\\Explorer", "Desktop", Some(if en { "Default" } else { "" }));
                                if en {
                                    let _ = store.save_setting(&pp, "Software\\Wine\\Explorer\\Desktops", "Default", Some(&format!("{}x{}", w, h)));
                                } else {
                                    let _ = store.save_setting(&pp, "Software\\Wine\\Explorer\\Desktops", "Default", None);
                                }
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
            RegistryEditorMsg::MacDriverUpdate { allow_vertical_sync, capture_displays, precise_scrolling, retina_mode, left_option_alt, right_option_alt, left_command_ctrl, right_command_ctrl } => {
                if let Some(editor_arc) = &self.registry_editor {
                    let editor_arc_clone = editor_arc.clone();
                    let sender_clone = sender.clone();
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            let mut mac_settings = MacDriverSettings::new();
                            if let Some(v) = allow_vertical_sync { mac_settings.allow_vertical_sync = Some(v); }
                            if let Some(v) = capture_displays { mac_settings.capture_displays_for_fullscreen = Some(v); }
                            if let Some(v) = precise_scrolling { mac_settings.use_precise_scrolling = Some(v); }
                            if let Some(v) = retina_mode { mac_settings.retina_mode = Some(v); }
                            if let Some(v) = left_option_alt { mac_settings.left_option_is_alt = Some(v); }
                            if let Some(v) = right_option_alt { mac_settings.right_option_is_alt = Some(v); }
                            if let Some(v) = left_command_ctrl { mac_settings.left_command_is_ctrl = Some(v); }
                            if let Some(v) = right_command_ctrl { mac_settings.right_command_is_ctrl = Some(v); }
                            editor.set_mac_driver_settings(&mac_settings).await
                        }.await;

                        match result {
                            Ok(()) => {
                                if let Some(v) = allow_vertical_sync { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "AllowVerticalSync", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = capture_displays { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "CaptureDisplaysForFullscreen", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = precise_scrolling { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "UsePreciseScrolling", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = retina_mode { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "RetinaMode", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = left_option_alt { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "LeftOptionIsAlt", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = right_option_alt { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "RightOptionIsAlt", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = left_command_ctrl { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "LeftCommandIsCtrl", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = right_command_ctrl { let _ = store.save_setting(&pp, "Software\\Wine\\Mac Driver", "RightCommandIsCtrl", Some(if v { "Y" } else { "N" })); }
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            let mut dpi_settings = if let Ok(Some(settings)) = editor.get_dpi_settings().await {
                                settings
                            } else {
                                DpiSettings { log_pixels: None }
                            };
                            if let Some(pixels) = log_pixels { dpi_settings.log_pixels = Some(pixels); }
                            editor.set_dpi_settings(&dpi_settings).await
                        }.await;

                        match result {
                            Ok(()) => {
                                let _ = store.save_setting(&pp, "Control Panel\\Desktop", "LogPixels", log_pixels.map(|v| v.to_string()).as_deref());
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
                    let pp = self.prefix_path.to_string_lossy().to_string();
                    let store = Arc::clone(&self.prefix_store);

                    tokio::spawn(async move {
                        let result = async {
                            let mut editor = editor_arc_clone.lock().await;
                            let mut x11_settings = if let Ok(Some(s)) = editor.get_x11_driver_settings().await { s } else { X11DriverSettings::new() };
                            if let Some(v) = decorated { x11_settings.decorated = Some(v); }
                            if let Some(v) = client_side_graphics { x11_settings.client_side_graphics = Some(v); }
                            if let Some(v) = client_side_with_render { x11_settings.client_side_with_render = Some(v); }
                            if let Some(v) = client_side_antialias_with_render { x11_settings.client_side_antialias_with_render = Some(v); }
                            if let Some(v) = client_side_antialias_with_core { x11_settings.client_side_antialias_with_core = Some(v); }
                            if let Some(v) = grab_fullscreen { x11_settings.grab_fullscreen = Some(v); }
                            if let Some(v) = grab_pointer { x11_settings.grab_pointer = Some(v); }
                            if let Some(v) = managed { x11_settings.managed = Some(v); }
                            if let Some(v) = use_xrandr { x11_settings.use_xrandr = Some(v); }
                            if let Some(v) = use_xvid_mode { x11_settings.use_xvid_mode = Some(v); }
                            editor.set_x11_driver_settings(&x11_settings).await
                        }.await;

                        match result {
                            Ok(()) => {
                                let sec = "Software\\Wine\\X11 Driver";
                                if let Some(v) = decorated { let _ = store.save_setting(&pp, sec, "Decorated", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = client_side_graphics { let _ = store.save_setting(&pp, sec, "ClientSideGraphics", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = client_side_with_render { let _ = store.save_setting(&pp, sec, "ClientSideWithRender", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = client_side_antialias_with_render { let _ = store.save_setting(&pp, sec, "ClientSideAntiAliasWithRender", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = client_side_antialias_with_core { let _ = store.save_setting(&pp, sec, "ClientSideAntiAliasWithCore", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = grab_fullscreen { let _ = store.save_setting(&pp, sec, "GrabFullscreen", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = grab_pointer { let _ = store.save_setting(&pp, sec, "GrabPointer", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = managed { let _ = store.save_setting(&pp, sec, "Managed", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = use_xrandr { let _ = store.save_setting(&pp, sec, "UseXRandR", Some(if v { "Y" } else { "N" })); }
                                if let Some(v) = use_xvid_mode { let _ = store.save_setting(&pp, sec, "UseXVidMode", Some(if v { "Y" } else { "N" })); }
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

fn spawn_registry_load(
    prefix_path: PathBuf,
    prefix_path_str: String,
    store: Arc<crate::prefix::PrefixStore>,
    warm: bool,
    sender: ComponentSender<RegistryEditorModel>,
) {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let result = async move {
            let editor = RegistryEditor::with_prefix(Arc::new(InMemoryRegistryCache::with_default_ttl()), &prefix_path).await?;
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
                windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode,
                video_memory_size, audio_driver, virtual_desktop, dpi_settings,
                x11_driver_settings,
                #[cfg(target_os = "macos")] mac_driver_settings,
                #[cfg(not(target_os = "macos"))] None::<MacDriverSettings>,
            );
            Ok::<_, PrefixError>((editor, data))
        }.await;
        let _ = tx.send(result);
    });

    let prefix_path_str2 = prefix_path_str;
    tokio::spawn(async move {
        match rx.await {
            Ok(Ok((editor, (windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode, video_memory_size, audio_driver, virtual_desktop, dpi_settings, x11_driver_settings, mac_driver_settings)))) => {
                if !warm {
                    let pp = &prefix_path_str2;
                    let _ = store.save_setting(pp, "Software\\Wine", "Version", windows_version.as_deref());
                    let _ = store.save_setting(pp, "Software\\Wine\\Direct3D", "renderer", d3d_renderer.as_deref());
                    let _ = store.save_setting(pp, "Software\\Wine\\Direct3D", "csmt", d3d_csmt.map(|v| v.to_string()).as_deref());
                    let _ = store.save_setting(pp, "Software\\Wine\\Direct3D", "OffscreenRenderingMode", offscreen_rendering_mode.as_deref());
                    let _ = store.save_setting(pp, "Software\\Wine\\Direct3D", "VideoMemorySize", video_memory_size.map(|v| v.to_string()).as_deref());
                    let _ = store.save_setting(pp, "Software\\Wine\\Drivers\\Audio", "", audio_driver.as_deref());
                    if let Some(ref vd) = virtual_desktop {
                        let _ = store.save_setting(pp, "Software\\Wine\\Explorer", "Desktop", Some(if vd.enabled { "Default" } else { "" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Explorer\\Desktops", "Default", Some(&format!("{}x{}", vd.width, vd.height)));
                    }
                    if let Some(ref dpi) = dpi_settings {
                        let _ = store.save_setting(pp, "Control Panel\\Desktop", "LogPixels", dpi.log_pixels.map(|v| v.to_string()).as_deref());
                    }
                    if let Some(ref x11) = x11_driver_settings {
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "Decorated", x11.decorated.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "ClientSideGraphics", x11.client_side_graphics.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "ClientSideWithRender", x11.client_side_with_render.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "ClientSideAntiAliasWithRender", x11.client_side_antialias_with_render.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "ClientSideAntiAliasWithCore", x11.client_side_antialias_with_core.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "GrabFullscreen", x11.grab_fullscreen.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "GrabPointer", x11.grab_pointer.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "Managed", x11.managed.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "UseXRandR", x11.use_xrandr.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\X11 Driver", "UseXVidMode", x11.use_xvid_mode.map(|v| if v { "Y" } else { "N" }));
                    }
                    #[cfg(target_os = "macos")]
                    if let Some(ref mac) = mac_driver_settings {
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "AllowVerticalSync", mac.allow_vertical_sync.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "CaptureDisplaysForFullscreen", mac.capture_displays_for_fullscreen.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "UsePreciseScrolling", mac.use_precise_scrolling.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "RetinaMode", mac.retina_mode.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "LeftOptionIsAlt", mac.left_option_is_alt.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "RightOptionIsAlt", mac.right_option_is_alt.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "LeftCommandIsCtrl", mac.left_command_is_ctrl.map(|v| if v { "Y" } else { "N" }));
                        let _ = store.save_setting(pp, "Software\\Wine\\Mac Driver", "RightCommandIsCtrl", mac.right_command_is_ctrl.map(|v| if v { "Y" } else { "N" }));
                    }
                }
                sender.input(RegistryEditorMsg::RegistryEditorUpdateTabs(
                    windows_version, d3d_renderer, d3d_csmt, offscreen_rendering_mode,
                    video_memory_size, audio_driver, virtual_desktop, dpi_settings,
                    x11_driver_settings, mac_driver_settings,
                ));
                sender.input(RegistryEditorMsg::RegistryEditorLoaded(Arc::new(Mutex::new(editor))));
            }
            Ok(Err(e)) => { eprintln!("Failed to load registry: {}", e); }
            Err(_) => { eprintln!("Failed to receive registry load result"); }
        }
    });
}