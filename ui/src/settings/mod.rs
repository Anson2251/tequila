use adw::prelude::*;
use relm4::prelude::*;
use tracker;
use std::path::PathBuf;
use prefix::{
    Manager as PrefixManager,
    runtime::{RuntimeManager, download, graphics as prefix_graphics},
    GraphicsBackend,
};

mod runtime;
mod graphics;
pub mod managed_download_row;

// ── Model (data only, no widget references) ──────────────────────────────

#[tracker::track]
pub struct SettingsWindow {
    pub prefix_manager: PrefixManager,

    // Page subtitle data
    runtime_subtitle: String,
    graphics_subtitle: String,

    // Managed download row controller for GStreamer
    #[tracker::do_not_track]
    gst_ctrl: AsyncController<managed_download_row::ManagedDownloadRow>,

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

    // Forwarded from GStreamer child component
    GStreamerStatusChanged,

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

            #[name = "gst_group"]
            adw::PreferencesGroup {
                set_title: "GStreamer",
                set_description: Some("Audio and video framework required by Wine on macOS"),
                #[watch]
                set_visible: cfg!(target_os = "macos"),
            },
        },

        #[name = "back_btn"]
        gtk::Button {
            set_icon_name: "go-previous-symbolic",
            set_visible: false,
        },

        #[name = "close_btn"]
        gtk::Button {
            set_icon_name: "window-close-symbolic",
            set_tooltip_text: Some("Close"),
            set_visible: cfg!(not(target_os = "macos")),
            connect_clicked[sender] => move |_| {
                sender.input(SettingsMsg::Close);
            },
        },
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let prefix_manager = init;

        // ── Build header bar (must exist before view_output! for the titlebar reference) ──
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        // ── Compute model data (before view_output! so #[watch] can reference it) ──
        let rm = prefix_manager.runtime_manager();
        let runtime_subtitle = runtime_subtitle(rm);
        let graphics_subtitle_str = graphics_subtitle();

        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tequila");

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

        // ── GStreamer managed download row (macOS only) ──
        let gst_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: "GStreamer".into(),
                check_status: Box::new(gst_initial_status),
                check_update: None,
                start_download: Box::new(|data_dir, progress, cancel| {
                    Box::pin(async move {
                        prefix::runtime::download::download_gstreamer(
                            &data_dir,
                            progress,
                            Some(cancel),
                        )
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                    })
                }),
                perform_remove: Box::new(|| {
                    let gst_dir = prefix::runtime::download::runtimes_dir().join("gstreamer");
                    if gst_dir.exists() {
                        std::fs::remove_dir_all(&gst_dir).map_err(|e| e.to_string())
                    } else {
                        Ok(())
                    }
                }),
                data_dir,
            })
            .forward(sender.input_sender(), |_out| {
                SettingsMsg::GStreamerStatusChanged
            });

        // Create local widgets referenced by #[local_ref] in view!
        let prefs_page = adw::PreferencesPage::new();

        // Placeholder nav — will be replaced with the real one from view! after view_output!()
        let placeholder_nav = adw::NavigationView::new();
        let mut model = SettingsWindow {
            prefix_manager,
            runtime_subtitle,
            graphics_subtitle: graphics_subtitle_str,
            gst_ctrl,
            nav: placeholder_nav,
            runtime_ctrl,
            graphics_ctrl,
            tracker: 0,
        };

        // Generate all named widgets from view! block (needs model to exist)
        let widgets = view_output!();

        // Replace placeholder nav with the real one created by view!
        model.nav = widgets.nav.clone();

        // Add buttons to header_bar (must be after view_output! so named widgets exist)
        header_bar.pack_start(&widgets.back_btn);
        header_bar.pack_end(&widgets.close_btn);

        // Wire up back button to NavigationView
        {
            let nav = widgets.nav.clone();
            widgets.back_btn.connect_clicked(move |_| {
                nav.pop();
            });
        }

        // Show/hide back button when visible page changes
        {
            let nav = widgets.nav.clone();
            let back = widgets.back_btn.clone();
            let root_page = widgets.root_page.clone();
            nav.connect_notify_local(Some("visible-page"), move |nav, _| {
                let visible = nav.visible_page();
                let is_root = visible.as_ref().map_or(false, |p| *p == root_page);
                back.set_visible(!is_root);
            });
        }

        // Add GStreamer ManagedDownloadRow to its group (group declared in view!)
        widgets.gst_group.add(model.gst_ctrl.widget());

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

            // ── Forwarded from GStreamer child component ──
            SettingsMsg::GStreamerStatusChanged => {
                // GStreamer component manages its own state internally.
                // This message is available if the parent needs to react
                // (e.g., to update a section-level summary).
            }

            // ── Window ──
            SettingsMsg::Close => {
                root.set_visible(false);
            }

        }
    }
}

// ── GStreamer helpers ───────────────────────────────────────────────────

fn gst_initial_status() -> managed_download_row::DownloadRowStatus {
    // Check managed installation (has version.txt)
    let gst_dir = download::runtimes_dir().join("gstreamer");
    let managed = std::fs::read_to_string(gst_dir.join("version.txt"))
        .ok()
        .and_then(|v| {
            let t = v.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });

    if let Some(ver) = managed {
        return managed_download_row::DownloadRowStatus {
            installed: true,
            managed: true,
            status_text: format!("✓ Installed ({})", ver),
        };
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
                    return managed_download_row::DownloadRowStatus {
                        installed: true,
                        managed: false,
                        status_text: "✓ Installed (system)".into(),
                    };
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
            return managed_download_row::DownloadRowStatus {
                installed: true,
                managed: false,
                status_text: "✓ Installed (system)".into(),
            };
        }
    }

    managed_download_row::DownloadRowStatus {
        installed: false,
        managed: false,
        status_text: "Not installed".into(),
    }
}
