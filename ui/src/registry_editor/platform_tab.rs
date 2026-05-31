use adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, adw, gtk};
use tracker;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacSettings {
    pub allow_vertical_sync: Option<bool>,
    pub capture_displays: Option<bool>,
    pub precise_scrolling: Option<bool>,
    pub retina_mode: Option<bool>,
    pub left_option_alt: Option<bool>,
    pub right_option_alt: Option<bool>,
    pub left_command_ctrl: Option<bool>,
    pub right_command_ctrl: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct X11Settings {
    pub decorated: Option<bool>,
    pub client_side_graphics: Option<bool>,
    pub client_side_with_render: Option<bool>,
    pub client_side_antialias_with_render: Option<bool>,
    pub client_side_antialias_with_core: Option<bool>,
    pub grab_fullscreen: Option<bool>,
    pub grab_pointer: Option<bool>,
    pub managed: Option<bool>,
    pub use_xrandr: Option<bool>,
    pub use_xvid_mode: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct PlatformSettings {
    pub mac: Option<MacSettings>,
    pub x11: Option<X11Settings>,
}

#[derive(Debug)]
#[tracker::track]
pub struct PlatformTabModel {
    editing: bool,
    mac: MacSettings,
    x11: X11Settings,
}

#[derive(Debug)]
pub enum PlatformTabInput {
    SetEditing(bool),
    LoadSettings(PlatformSettings),
    UpdateField(String, String),
}

#[derive(Debug)]
pub enum PlatformTabOutput {
    SettingChanged(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for PlatformTabModel {
    type Init = PlatformSettings;
    type Input = PlatformTabInput;
    type Output = PlatformTabOutput;

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_vexpand: true,
            set_hexpand: true,
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vscrollbar_policy: gtk::PolicyType::Automatic,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_all: 18,
                set_vexpand: true,
                set_hexpand: true,

                // ══════════════════════════════════════
                //  Mac Driver
                // ══════════════════════════════════════

                adw::PreferencesGroup {
                    set_visible: cfg!(target_os = "macos"),
                    set_title: "Mac Driver",
                    set_description: Some("Configure macOS-specific Wine display and input settings"),

                    adw::ActionRow {
                        set_title: "Allow Vertical Sync",
                        set_subtitle: "Synchronize frame buffer updates with display refresh",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.allow_vertical_sync.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_allow_vertical_sync".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Capture Displays for Fullscreen",
                        set_subtitle: "Allow Wine to capture displays when entering fullscreen",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.capture_displays.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_capture_displays".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Use Precise Scrolling",
                        set_subtitle: "Enable precise pixel-based scrolling",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.precise_scrolling.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_precise_scrolling".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Enable Retina Mode",
                        set_subtitle: "Enable high-DPI Retina display support",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.retina_mode.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_retina_mode".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },
                },

                adw::PreferencesGroup {
                    set_margin_top: 18,
                    #[watch]
                    set_visible: cfg!(target_os = "macos"),
                    set_title: "Mac Keyboard Modifiers",
                    set_description: Some("Configure keyboard modifier key behavior in Wine"),

                    adw::ActionRow {
                        set_title: "Left Option is Alt",
                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.left_option_alt.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_left_option_alt".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Right Option is Alt",
                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.right_option_alt.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_right_option_alt".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Left Command is Ctrl",
                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.left_command_ctrl.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_left_command_ctrl".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Right Command is Ctrl",
                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::mac())"]
                            set_active: model.mac.right_command_ctrl.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "mac_right_command_ctrl".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },
                },

                // ══════════════════════════════════════
                //  X11 Driver
                // ══════════════════════════════════════
                adw::PreferencesGroup {
                    set_margin_top: if cfg!(target_os = "macos") { 18 } else { 0 },
                    set_title: "Window Management",
                    set_description: Some("Configure X11 window manager integration"),

                    adw::ActionRow {
                        set_title: "Decorated Windows",
                        set_subtitle: "Show window decorations",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.decorated.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_decorated".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Managed by Window Manager",
                        set_subtitle: "Let the window manager control window positions",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.managed.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_managed".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Grab Pointer",
                        set_subtitle: "Confine pointer to the Wine window",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.grab_pointer.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_grab_pointer".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Grab Fullscreen",
                        set_subtitle: "Grab the pointer when entering fullscreen mode",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.grab_fullscreen.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_grab_fullscreen".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },
                },

                adw::PreferencesGroup {
                    set_margin_top: 18,
                    set_title: "Rendering",
                    set_description: Some("Configure X11 client-side rendering"),

                    adw::ActionRow {
                        set_title: "Client Side Graphics",
                        set_subtitle: "Render graphics using client-side buffers",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.client_side_graphics.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_client_side_graphics".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Client Side With Render",
                        set_subtitle: "Use shared memory for client-side rendering",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.client_side_with_render.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_client_side_with_render".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Client Side Anti-Alias With Render",
                        set_subtitle: "Enable anti-aliasing for client-side rendering with Render extension",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.client_side_antialias_with_render.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_client_side_antialias_with_render".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Client Side Anti-Alias With Core",
                        set_subtitle: "Enable anti-aliasing for client-side rendering with core protocol",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.client_side_antialias_with_core.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_client_side_antialias_with_core".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },
                },

                adw::PreferencesGroup {
                    set_margin_top: 18,
                    set_title: "Display Management",
                    set_description: Some("Configure X11 display management"),

                    adw::ActionRow {
                        set_title: "Use XRandR",
                        set_subtitle: "Use the XRandR extension for display configuration",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.use_xrandr.unwrap_or(true),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_use_xrandr".into(), sw.is_active().to_string(),
                                ));
                            },
                        },
                    },

                    adw::ActionRow {
                        set_title: "Use XVidMode",
                        set_subtitle: "Use the XVidMode extension for video mode switching",

                        add_suffix = &gtk::Switch {
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(PlatformTabModel::x11())"]
                            set_active: model.x11.use_xvid_mode.unwrap_or(false),
                            #[track = "model.changed(PlatformTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_active_notify[sender] => move |sw| {
                                sender.input(PlatformTabInput::UpdateField(
                                    "x11_use_xvid_mode".into(), sw.is_active().to_string(),
                                ));
                            },
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
        let model = PlatformTabModel {
            editing: false,
            mac: init.mac.unwrap_or_default(),
            x11: init.x11.unwrap_or_default(),
            tracker: 0,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            PlatformTabInput::SetEditing(v) => {
                self.set_editing(v);
            }
            PlatformTabInput::LoadSettings(s) => {
                if let Some(m) = s.mac {
                    self.set_mac(m);
                }
                if let Some(x) = s.x11 {
                    self.set_x11(x);
                }
            }
            PlatformTabInput::UpdateField(field, value) => {
                let v = value == "true";
                let section = "Software\\Wine";
                let (key, reg_section, reg_key) = match field.as_str() {
                    "mac_allow_vertical_sync" => {
                        ("mac_allow_vertical_sync", "Mac Driver", "AllowVerticalSync")
                    }
                    "mac_capture_displays" => (
                        "mac_capture_displays",
                        "Mac Driver",
                        "CaptureDisplaysForFullscreen",
                    ),
                    "mac_precise_scrolling" => {
                        ("mac_precise_scrolling", "Mac Driver", "UsePreciseScrolling")
                    }
                    "mac_retina_mode" => ("mac_retina_mode", "Mac Driver", "RetinaMode"),
                    "mac_left_option_alt" => {
                        ("mac_left_option_alt", "Mac Driver", "LeftOptionIsAlt")
                    }
                    "mac_right_option_alt" => {
                        ("mac_right_option_alt", "Mac Driver", "RightOptionIsAlt")
                    }
                    "mac_left_command_ctrl" => {
                        ("mac_left_command_ctrl", "Mac Driver", "LeftCommandIsCtrl")
                    }
                    "mac_right_command_ctrl" => {
                        ("mac_right_command_ctrl", "Mac Driver", "RightCommandIsCtrl")
                    }
                    "x11_decorated" => ("x11_decorated", "X11 Driver", "Decorated"),
                    "x11_client_side_graphics" => (
                        "x11_client_side_graphics",
                        "X11 Driver",
                        "ClientSideGraphics",
                    ),
                    "x11_client_side_with_render" => (
                        "x11_client_side_with_render",
                        "X11 Driver",
                        "ClientSideWithRender",
                    ),
                    "x11_client_side_antialias_with_render" => (
                        "x11_client_side_antialias_with_render",
                        "X11 Driver",
                        "ClientSideAntiAliasWithRender",
                    ),
                    "x11_client_side_antialias_with_core" => (
                        "x11_client_side_antialias_with_core",
                        "X11 Driver",
                        "ClientSideAntiAliasWithCore",
                    ),
                    "x11_grab_fullscreen" => {
                        ("x11_grab_fullscreen", "X11 Driver", "GrabFullscreen")
                    }
                    "x11_grab_pointer" => ("x11_grab_pointer", "X11 Driver", "GrabPointer"),
                    "x11_managed" => ("x11_managed", "X11 Driver", "Managed"),
                    "x11_use_xrandr" => ("x11_use_xrandr", "X11 Driver", "UseXRandR"),
                    "x11_use_xvid_mode" => ("x11_use_xvid_mode", "X11 Driver", "UseXVidMode"),
                    _ => return,
                };

                match key {
                    "mac_allow_vertical_sync" => {
                        let mut m = self.mac.clone();
                        m.allow_vertical_sync = Some(v);
                        self.set_mac(m);
                    }
                    "mac_capture_displays" => {
                        let mut m = self.mac.clone();
                        m.capture_displays = Some(v);
                        self.set_mac(m);
                    }
                    "mac_precise_scrolling" => {
                        let mut m = self.mac.clone();
                        m.precise_scrolling = Some(v);
                        self.set_mac(m);
                    }
                    "mac_retina_mode" => {
                        let mut m = self.mac.clone();
                        m.retina_mode = Some(v);
                        self.set_mac(m);
                    }
                    "mac_left_option_alt" => {
                        let mut m = self.mac.clone();
                        m.left_option_alt = Some(v);
                        self.set_mac(m);
                    }
                    "mac_right_option_alt" => {
                        let mut m = self.mac.clone();
                        m.right_option_alt = Some(v);
                        self.set_mac(m);
                    }
                    "mac_left_command_ctrl" => {
                        let mut m = self.mac.clone();
                        m.left_command_ctrl = Some(v);
                        self.set_mac(m);
                    }
                    "mac_right_command_ctrl" => {
                        let mut m = self.mac.clone();
                        m.right_command_ctrl = Some(v);
                        self.set_mac(m);
                    }
                    "x11_decorated" => {
                        let mut x = self.x11.clone();
                        x.decorated = Some(v);
                        self.set_x11(x);
                    }
                    "x11_client_side_graphics" => {
                        let mut x = self.x11.clone();
                        x.client_side_graphics = Some(v);
                        self.set_x11(x);
                    }
                    "x11_client_side_with_render" => {
                        let mut x = self.x11.clone();
                        x.client_side_with_render = Some(v);
                        self.set_x11(x);
                    }
                    "x11_client_side_antialias_with_render" => {
                        let mut x = self.x11.clone();
                        x.client_side_antialias_with_render = Some(v);
                        self.set_x11(x);
                    }
                    "x11_client_side_antialias_with_core" => {
                        let mut x = self.x11.clone();
                        x.client_side_antialias_with_core = Some(v);
                        self.set_x11(x);
                    }
                    "x11_grab_fullscreen" => {
                        let mut x = self.x11.clone();
                        x.grab_fullscreen = Some(v);
                        self.set_x11(x);
                    }
                    "x11_grab_pointer" => {
                        let mut x = self.x11.clone();
                        x.grab_pointer = Some(v);
                        self.set_x11(x);
                    }
                    "x11_managed" => {
                        let mut x = self.x11.clone();
                        x.managed = Some(v);
                        self.set_x11(x);
                    }
                    "x11_use_xrandr" => {
                        let mut x = self.x11.clone();
                        x.use_xrandr = Some(v);
                        self.set_x11(x);
                    }
                    "x11_use_xvid_mode" => {
                        let mut x = self.x11.clone();
                        x.use_xvid_mode = Some(v);
                        self.set_x11(x);
                    }
                    _ => unreachable!(),
                }

                let reg_val = if v { "Y" } else { "N" };
                let _ = sender.output(PlatformTabOutput::SettingChanged(
                    format!("{}\\{}", section, reg_section),
                    format!("{}={}", reg_key, reg_val),
                ));
            }
        }
    }
}
