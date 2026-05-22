use adw::prelude::*;
use relm4::prelude::*;
use tracker;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use prefix::{
    Manager as PrefixManager,
    runtime::{RuntimeManager, download, graphics as prefix_graphics},
    runtime::download::InstallPhase,
    GraphicsBackend,
};

mod runtime;
mod graphics;

// ── Model (data only, no widget references) ──────────────────────────────

#[tracker::track]
pub struct SettingsWindow {
    pub prefix_manager: PrefixManager,

    // Page subtitle data
    runtime_subtitle: String,
    graphics_subtitle: String,

    // GStreamer state
    gst_visible: bool,        // false on non-macOS (hides the entire section)
    gst_installed: bool,
    gst_managed: bool,
    gst_progress: f64,
    gst_status_text: String,
    gst_downloading: bool,

    #[tracker::do_not_track]
    gst_cancel_flag: Option<Arc<AtomicBool>>,

    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_last_bytes: u64,
    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_last_time: std::time::Instant,
    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_current_speed: f64,

    // NavigationView kept in model for push/pop actions in update()
    #[tracker::do_not_track]
    nav: adw::NavigationView,

    // Child subpage controllers
    #[tracker::do_not_track]
    runtime_ctrl: AsyncController<runtime::RuntimeSettings>,
    #[tracker::do_not_track]
    graphics_ctrl: AsyncController<graphics::GraphicsSettings>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SettingsMsg {
    // Navigation
    ShowRuntime,
    ShowGraphics,

    // Forwarded from child components
    RuntimeSubtitleChanged(String),
    GraphicsSubtitleChanged(String),
    RuntimesUpdated(RuntimeManager),

    // GStreamer operations
    InstallGStreamer,
    BeginGStreamerInstall,
    CancelGStreamer,
    GStreamerProgress(u64, u64, InstallPhase),
    GStreamerComplete,
    GStreamerFailed(String),
    RemoveGStreamer,

    // Window
    Close,
}

#[derive(Debug)]
pub enum SettingsOutput {
    RuntimesUpdated(RuntimeManager),
}

// ── Status helpers ───────────────────────────────────────────────────────

fn runtime_subtitle(rm: &RuntimeManager) -> String {
    match rm.get_default() {
        Some(rt) => {
            let graphics = if rt.graphics.is_empty() {
                String::new()
            } else {
                let names: Vec<&str> = rt.graphics.iter().map(|g| match g {
                    GraphicsBackend::Dxmt { .. } => "DXMT",
                    GraphicsBackend::D3DMetal { .. } => "D3DMetal",
                    GraphicsBackend::DxvkVkd3d { .. } => "DXVK+VKD3D",
                }).collect();
                format!(" · {}", names.join(", "))
            };
            let count = rm.runtimes.len();
            format!("{}{} · {} runtime{}", rt.wine_version, graphics, count, if count == 1 { "" } else { "s" })
        }
        None => "No runtimes installed".to_string(),
    }
}

fn graphics_subtitle() -> String {
    let dir = prefix_graphics::graphics_dir();
    if !dir.is_dir() {
        return "No backends installed".to_string();
    }
    let backends: Vec<String> = std::fs::read_dir(&dir).ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    if backends.is_empty() {
        "No backends installed".to_string()
    } else {
        backends.join(" · ")
    }
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for SettingsWindow {
    type Init = PrefixManager;
    type Input = SettingsMsg;
    type Output = SettingsOutput;
    type CommandOutput = ();
    type Widgets = SettingsWindowWidgets;

    // The view! macro declares the entire UI tree.
    // Widget state is bound to model fields via #[watch] / #[track].
    // prefs_page is created in init() and populated via #[local_ref] below.
    view! {
        #[root]
        gtk::Window {
            set_title: Some("Tequila Settings"),
            set_default_width: 480,
            set_default_height: 520,
            set_modal: true,
            set_hide_on_close: true,

            set_titlebar: Some(&header_bar),

            connect_close_request[sender] => move |_| {
                sender.input(SettingsMsg::Close);
                gtk::glib::Propagation::Stop
            },

            #[name = "nav"]
            adw::NavigationView {
                push: root_page = &adw::NavigationPage {
                    set_title: "Tequila Settings",
                    set_can_pop: false,
                    set_child: Some(&prefs_page),
                },
            }
        },

        #[local_ref]
        prefs_page -> adw::PreferencesPage {
            adw::PreferencesGroup {
                adw::ActionRow {
                    set_title: "Wine Runtime",
                    set_activatable: true,
                    #[watch]
                    set_subtitle: &model.runtime_subtitle,
                    connect_activated => SettingsMsg::ShowRuntime,
                },
                adw::ActionRow {
                    set_title: "Graphics Backends",
                    set_activatable: true,
                    #[watch]
                    set_subtitle: &model.graphics_subtitle,
                    connect_activated => SettingsMsg::ShowGraphics,
                },
            },

            // ── GStreamer section (visible only on macOS) ──
            adw::PreferencesGroup {
                set_title: "GStreamer",
                set_description: Some("Audio and video framework required by Wine on macOS"),
                #[watch]
                set_visible: model.gst_visible,

                adw::ActionRow {
                    set_title: "GStreamer",
                    set_activatable: false,
                    #[watch]
                    set_subtitle: &model.gst_status_text,
                    #[track = "model.changed(SettingsWindow::gst_installed())"]
                    set_class_active: ("gst-installed", model.gst_installed),

                    add_suffix = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::End,
                        set_spacing: 4,

                        // Install button
                        #[name = "gst_install_btn"]
                        gtk::Button {
                            set_icon_name: "document-save-symbolic",
                            set_tooltip: "Install GStreamer",
                            add_css_class: "flat",
                            set_valign: gtk::Align::Center,
                            add_css_class: "circular",
                            #[watch]
                            set_visible: !model.gst_installed && !model.gst_downloading,
                            connect_clicked => SettingsMsg::InstallGStreamer,
                        },
                        // Remove button (only for managed installations)
                        #[name = "gst_remove_btn"]
                        gtk::Button {
                            set_icon_name: "user-trash-symbolic",
                            set_tooltip: "Remove GStreamer",
                            add_css_class: "flat",
                            set_valign: gtk::Align::Center,
                            add_css_class: "circular",
                            add_css_class: "destructive-action",
                            #[watch]
                            set_visible: model.gst_managed && !model.gst_downloading,
                            connect_clicked => SettingsMsg::RemoveGStreamer,
                        },
                        // Download progress area
                        gtk::Box {
                            set_valign: gtk::Align::Center,
                            set_spacing: 6,
                            #[watch]
                            set_visible: model.gst_downloading,
                            #[name = "gst_progress_bar"]
                            gtk::LevelBar {
                                set_valign: gtk::Align::Center,
                                set_min_value: 0.0,
                                set_max_value: 1.0,
                                set_width_request: 80,
                                #[watch]
                                set_value: model.gst_progress,
                            },
                            #[name = "gst_cancel_btn"]
                            gtk::Button {
                                set_icon_name: "window-close-symbolic",
                                set_tooltip: "Cancel download",
                                add_css_class: "flat",
                                add_css_class: "circular",
                                connect_clicked => SettingsMsg::CancelGStreamer,
                            },
                        },
                    },
                },
            },
        },
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let prefix_manager = init;

        // ── Build header bar ──
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        // Navigation back button (hidden until a subpage is pushed)
        let back_btn = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .visible(false)
            .build();
        header_bar.pack_start(&back_btn);

        #[cfg(not(target_os = "macos"))]
        {
            let close_btn = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text("Close")
                .build();
            let s = sender.clone();
            close_btn.connect_clicked(move |_| {
                let _ = s.input(SettingsMsg::Close);
            });
            header_bar.pack_end(&close_btn);
        }

        // Initialize GStreamer CSS (macOS only)
        #[cfg(target_os = "macos")]
        init_gst_css();

        // ── Compute model data (before view_output! so #[watch] can reference it) ──
        let rm = prefix_manager.runtime_manager();
        let runtime_subtitle = runtime_subtitle(rm);
        let graphics_subtitle_str = graphics_subtitle();

        #[cfg(target_os = "macos")]
        let (gst_installed, gst_managed, gst_status_text) = gst_initial_status();
        #[cfg(not(target_os = "macos"))]
        let (gst_installed, gst_managed, gst_status_text) = (false, false, String::new());

        // Create child subpage controllers (independent of widgets)
        let runtime_ctrl = runtime::RuntimeSettings::builder()
            .launch((prefix_manager.clone(), root.clone()))
            .forward(sender.input_sender(), |msg| match msg {
                runtime::RuntimeSettingsOutput::RuntimesUpdated(rm) => {
                    SettingsMsg::RuntimesUpdated(rm)
                }
            });

        let graphics_ctrl = graphics::GraphicsSettings::builder()
            .launch(())
            .forward(sender.input_sender(), |_| {
                SettingsMsg::GraphicsSubtitleChanged(graphics_subtitle())
            });

        // Create local widgets referenced by #[local_ref] in view!
        let prefs_page = adw::PreferencesPage::new();

        // Placeholder nav — will be replaced with the real one from view! after view_output!()
        let placeholder_nav = adw::NavigationView::new();
        let mut model = SettingsWindow {
            prefix_manager,
            runtime_subtitle,
            graphics_subtitle: graphics_subtitle_str,
            #[cfg(target_os = "macos")]
            gst_visible: true,
            #[cfg(not(target_os = "macos"))]
            gst_visible: false,
            gst_installed,
            gst_managed,
            gst_progress: 0.0,
            gst_status_text,
            gst_downloading: false,
            gst_cancel_flag: None,
            #[cfg(target_os = "macos")]
            gst_last_bytes: 0,
            #[cfg(target_os = "macos")]
            gst_last_time: std::time::Instant::now(),
            #[cfg(target_os = "macos")]
            gst_current_speed: 0.0,
            nav: placeholder_nav,
            runtime_ctrl,
            graphics_ctrl,
            tracker: 0,
        };

        // Generate all named widgets from view! block (needs model to exist)
        let widgets = view_output!();

        // Replace placeholder nav with the real one created by view!
        model.nav = widgets.nav.clone();

        // Wire up back button to NavigationView
        {
            let nav = widgets.nav.clone();
            back_btn.connect_clicked(move |_| {
                nav.pop();
            });
        }

        // Show/hide back button when visible page changes
        {
            let nav = widgets.nav.clone();
            let back = back_btn.clone();
            let root_page = widgets.root_page.clone();
            nav.connect_notify_local(Some("visible-page"), move |nav, _| {
                let visible = nav.visible_page();
                let is_root = visible.as_ref().map_or(false, |p| *p == root_page);
                back.set_visible(!is_root);
            });
        }

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        self.reset();
        match msg {
            // ── Navigation ──
            SettingsMsg::ShowRuntime => {
                self.nav.push(self.runtime_ctrl.widget());
            }
            SettingsMsg::ShowGraphics => {
                self.nav.push(self.graphics_ctrl.widget());
            }

            // ── Subtitle updates from child components ──
            SettingsMsg::RuntimeSubtitleChanged(subtitle) => {
                self.runtime_subtitle = subtitle;
            }
            SettingsMsg::GraphicsSubtitleChanged(subtitle) => {
                self.graphics_subtitle = subtitle;
            }

            // ── Forwarded from RuntimeSettings ──
            SettingsMsg::RuntimesUpdated(rm) => {
                *self.prefix_manager.runtime_manager_mut() = rm;
                self.runtime_subtitle = runtime_subtitle(self.prefix_manager.runtime_manager());
                let _ = sender.output(SettingsOutput::RuntimesUpdated(
                    self.prefix_manager.runtime_manager().clone(),
                ));
            }

            // ── GStreamer: confirmation dialog ──
            #[cfg(target_os = "macos")]
            SettingsMsg::InstallGStreamer => {
                let alert = adw::AlertDialog::new(
                    Some("Install GStreamer?"),
                    Some("GStreamer is required for audio and video support in Wine. This will download and install the GStreamer runtime package (~500 MB)."),
                );
                alert.add_response("cancel", "Cancel");
                alert.add_response("install", "Install");
                alert.set_response_appearance("install", adw::ResponseAppearance::Suggested);
                alert.set_default_response(Some("install"));
                alert.set_close_response("cancel");
                let s = sender.clone();
                alert.choose(Some(root), None::<&gtk::gio::Cancellable>, move |response| {
                    if response == "install" {
                        let _ = s.input(SettingsMsg::BeginGStreamerInstall);
                    }
                });
            }

            // ── GStreamer: actual download start (after confirmation) ──
            #[cfg(target_os = "macos")]
            SettingsMsg::BeginGStreamerInstall => {
                self.gst_downloading = true;
                self.gst_progress = 0.0;
                self.gst_status_text = "Starting download...".into();
                self.gst_last_bytes = 0;
                self.gst_last_time = std::time::Instant::now();

                // Cancel previous download if any
                if let Some(old) = self.gst_cancel_flag.take() {
                    old.store(true, Ordering::Relaxed);
                }
                let cancel = Arc::new(AtomicBool::new(false));
                self.gst_cancel_flag = Some(cancel.clone());

                let s = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let data_dir = dirs::data_dir()
                        .unwrap_or_else(|| PathBuf::from(".")).join("tequila");
                    let progress: download::PhaseProgressFn = Box::new({
                        let s = s.clone();
                        move |downloaded, total, phase| {
                            let _ = s.input(SettingsMsg::GStreamerProgress(downloaded, total, phase));
                        }
                    });
                    match download::download_gstreamer(&data_dir, progress, Some(cancel)).await {
                        Ok(_) => { let _ = s.input(SettingsMsg::GStreamerComplete); }
                        Err(e) => { let _ = s.input(SettingsMsg::GStreamerFailed(e.to_string())); }
                    }
                });
            }

            // ── GStreamer: download progress ──
            #[cfg(target_os = "macos")]
            SettingsMsg::GStreamerProgress(downloaded, total, phase) => {
                let now = std::time::Instant::now();
                let mb = |b: u64| b as f64 / 1_048_576.0;

                match phase {
                    InstallPhase::Download => {
                        let elapsed = now.duration_since(self.gst_last_time);
                        if elapsed.as_secs_f64() > 0.5 {
                            self.gst_current_speed = (downloaded.saturating_sub(self.gst_last_bytes)) as f64
                                / elapsed.as_secs_f64();
                            self.gst_last_bytes = downloaded;
                            self.gst_last_time = now;
                        }

                        if total > 0 {
                            self.gst_progress = (downloaded as f64 / total as f64) * 0.8;
                            let speed_mb = self.gst_current_speed / 1_048_576.0;
                            self.gst_status_text = format!(
                                "Downloading — {:.1} / {:.1} MB ({:.1} MB/s)",
                                mb(downloaded), mb(total), speed_mb
                            );
                        }
                    }
                    InstallPhase::Verify => {
                        self.gst_progress = 0.80 + (downloaded as f64 / total.max(1) as f64) * 0.10;
                        self.gst_status_text = "Verifying checksum...".into();
                    }
                    InstallPhase::Extract => {
                        self.gst_progress = 0.90 + (downloaded as f64 / total.max(1) as f64) * 0.10;
                        self.gst_status_text = "Installing GStreamer...".into();
                    }
                }
            }

            #[cfg(target_os = "macos")]
            SettingsMsg::CancelGStreamer => {
                if let Some(cancel) = &self.gst_cancel_flag {
                    cancel.store(true, Ordering::Relaxed);
                }
            }

            #[cfg(target_os = "macos")]
            SettingsMsg::GStreamerComplete => {
                self.gst_cancel_flag = None;
                let (inst, mgd, text) = gst_initial_status();
                self.gst_installed = inst;
                self.gst_managed = mgd;
                self.gst_status_text = text;
                self.gst_downloading = false;
            }

            #[cfg(target_os = "macos")]
            SettingsMsg::GStreamerFailed(err) => {
                self.gst_cancel_flag = None;
                let (inst, mgd, text) = gst_initial_status();
                self.gst_installed = inst;
                self.gst_managed = mgd;
                self.gst_status_text = text;
                self.gst_downloading = false;

                let alert = adw::AlertDialog::new(
                    Some("Download Failed"),
                    Some(&err),
                );
                alert.add_response("ok", "OK");
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
            }

            #[cfg(target_os = "macos")]
            SettingsMsg::RemoveGStreamer => {
                let gst_dir = download::runtimes_dir().join("gstreamer");
                if gst_dir.exists() {
                    let _ = std::fs::remove_dir_all(&gst_dir);
                }
                let (inst, mgd, text) = gst_initial_status();
                self.gst_installed = inst;
                self.gst_managed = mgd;
                self.gst_status_text = text;
            }

            // ── Window ──
            SettingsMsg::Close => {
                root.set_visible(false);
            }

            // Non-macOS: GStreamer messages are unreachable but must be exhaustive
            #[cfg(not(target_os = "macos"))]
            _ => {}
        }
    }
}

// ── GStreamer helpers (macOS only) ───────────────────────────────────────

#[cfg(target_os = "macos")]
fn init_gst_css() {
    use gtk::gdk::Display;
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".gst-installed { background-color: rgba(76, 175, 80, 0.12); }"
    );
    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(target_os = "macos")]
fn gst_initial_status() -> (bool, bool, String) {
    // Check managed installation (has version.txt)
    let gst_dir = download::runtimes_dir().join("gstreamer");
    let managed = std::fs::read_to_string(gst_dir.join("version.txt"))
        .ok()
        .and_then(|v| {
            let t = v.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });

    if let Some(ver) = managed {
        return (true, true, format!("✓ Installed ({})", ver));
    }

    // Homebrew GStreamer
    if let Ok(output) = std::process::Command::new("brew")
        .args(["--prefix", "gstreamer"])
        .output()
    {
        if output.status.success() {
            if let Ok(prefix) = String::from_utf8(output.stdout) {
                let p = std::path::Path::new(prefix.trim());
                if p.join("bin").join("gst-launch-1.0").exists() {
                    return (true, false, "✓ Installed (system)".into());
                }
            }
        }
    }

    // System gst-launch-1.0 in PATH
    if let Ok(output) = std::process::Command::new("which")
        .arg("gst-launch-1.0")
        .output()
    {
        if output.status.success() {
            return (true, false, "✓ Installed (system)".into());
        }
    }

    (false, false, "Not installed".into())
}
