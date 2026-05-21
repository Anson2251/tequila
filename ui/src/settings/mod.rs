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

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct SettingsWindow {
    pub prefix_manager: PrefixManager,

    #[tracker::do_not_track]
    nav: adw::NavigationView,

    // Root page widgets
    #[tracker::do_not_track]
    root_page: adw::NavigationPage,
    #[tracker::do_not_track]
    runtime_row: adw::ActionRow,
    #[tracker::do_not_track]
    graphics_row: adw::ActionRow,

    // GStreamer inline controls
    #[tracker::do_not_track]
    gst_status_row: adw::ActionRow,
    #[tracker::do_not_track]
    gst_install_btn: gtk::Button,
    #[tracker::do_not_track]
    gst_remove_btn: gtk::Button,
    #[tracker::do_not_track]
    gst_progress_bar: gtk::LevelBar,
    #[tracker::do_not_track]
    gst_cancel_btn: gtk::Button,
    #[tracker::do_not_track]
    gst_cancel_flag: Option<Arc<AtomicBool>>,
    #[tracker::do_not_track]
    gst_installed: bool,
    // GStreamer download speed tracking
    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_last_bytes: u64,
    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_last_time: std::time::Instant,
    #[cfg(target_os = "macos")]
    #[tracker::do_not_track]
    gst_current_speed: f64,

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

    // GStreamer operations (inline on root page)
    InstallGStreamer,
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

    view! {
        #[root]
        gtk::Window {
            set_title: Some("Tequila Settings"),
            set_default_width: 480,
            set_default_height: 520,
            set_modal: true,
            set_hide_on_close: true,

            set_titlebar: Some(&header_bar),

            #[name = "nav"]
            adw::NavigationView {
                // Pages are built programmatically in init()
            }
        }
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

        let widgets = view_output!();

        // ── Build root page ──
        let (root_page, prefs_page, runtime_row, graphics_row) =
            build_root_page(&sender);
        widgets.nav.push(&root_page);

        // Connect back button to NavigationView
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
            let root = root_page.clone();
            nav.connect_notify_local(Some("visible-page"), move |nav, _| {
                let visible = nav.visible_page();
                let is_root = visible.as_ref().map_or(false, |p| *p == root);
                back.set_visible(!is_root);
            });
        }

        // ── Build GStreamer section (inline on root page) ──
        #[cfg(target_os = "macos")]
        let (gst_status_row, gst_install_btn, gst_remove_btn, gst_cancel_btn, gst_progress_bar, gst_installed) = {
            let (row, ib, rb, cb, pb) = build_gstreamer_section(&prefs_page, &sender, &root);
            let installed = refresh_gst_ui(&row, &ib, &rb, &cb, &pb);
            (row, ib, rb, cb, pb, installed)
        };
        #[cfg(not(target_os = "macos"))]
        let (gst_status_row, gst_install_btn, gst_remove_btn, gst_cancel_btn, gst_progress_bar, gst_installed) = {
            let row = adw::ActionRow::new();
            let btn = gtk::Button::new();
            btn.set_visible(false);
            row.set_visible(false);
            (row, btn.clone(), btn.clone(), btn.clone(), gtk::LevelBar::new(), false)
        };

        // ── Create child subpage components ──
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

        // ── Update root page status ──
        let rm = prefix_manager.runtime_manager();
        runtime_row.set_subtitle(&runtime_subtitle(rm));
        graphics_row.set_subtitle(&graphics_subtitle());

        // ── Close on window close ──
        {
            let s = sender.clone();
            root.connect_close_request(move |_win| {
                let _ = s.input(SettingsMsg::Close);
                gtk::glib::Propagation::Stop
            });
        }

        let model = SettingsWindow {
            prefix_manager,
            nav: widgets.nav.clone(),
            root_page,
            runtime_row,
            graphics_row,
            gst_status_row,
            gst_install_btn,
            gst_remove_btn,
            gst_progress_bar,
            gst_cancel_btn,
            gst_cancel_flag: None,
            gst_installed,
            gst_last_bytes: 0,
            gst_last_time: std::time::Instant::now(),
            gst_current_speed: 0.0,
            runtime_ctrl,
            graphics_ctrl,
            tracker: 0,
        };

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
                self.runtime_row.set_subtitle(&subtitle);
            }
            SettingsMsg::GraphicsSubtitleChanged(subtitle) => {
                self.graphics_row.set_subtitle(&subtitle);
            }

            // ── Forwarded from RuntimeSettings ──
            SettingsMsg::RuntimesUpdated(rm) => {
                *self.prefix_manager.runtime_manager_mut() = rm;
                self.runtime_row
                    .set_subtitle(&runtime_subtitle(self.prefix_manager.runtime_manager()));
                let _ = sender.output(SettingsOutput::RuntimesUpdated(
                    self.prefix_manager.runtime_manager().clone(),
                ));
            }

            // ── GStreamer operations ──
            #[cfg(target_os = "macos")]
            SettingsMsg::InstallGStreamer => {
                self.gst_progress_bar.set_visible(true);
                self.gst_cancel_btn.set_visible(true);
                self.gst_install_btn.set_visible(false);
                self.gst_remove_btn.set_visible(false);
                self.gst_status_row.set_subtitle("Starting download...");
                self.gst_last_bytes = 0;
                self.gst_last_time = std::time::Instant::now();

                // Cancel any previous download before starting a new one
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
                            let frac = downloaded as f64 / total as f64;
                            self.gst_progress_bar.set_value(frac * 0.8);
                            let speed_mb = self.gst_current_speed / 1_048_576.0;
                            self.gst_status_row.set_subtitle(&format!(
                                "Downloading — {:.1} / {:.1} MB ({:.1} MB/s)",
                                mb(downloaded), mb(total), speed_mb
                            ));
                        }
                    }
                    InstallPhase::Verify => {
                        let frac = 0.80 + (downloaded as f64 / total.max(1) as f64) * 0.10;
                        self.gst_progress_bar.set_value(frac);
                        self.gst_status_row.set_subtitle("Verifying checksum...");
                    }
                    InstallPhase::Extract => {
                        let frac = 0.90 + (downloaded as f64 / total.max(1) as f64) * 0.10;
                        self.gst_progress_bar.set_value(frac);
                        self.gst_status_row.set_subtitle("Installing GStreamer...");
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
                self.gst_installed = refresh_gst_ui(
                    &self.gst_status_row,
                    &self.gst_install_btn,
                    &self.gst_remove_btn,
                    &self.gst_cancel_btn,
                    &self.gst_progress_bar,
                );
            }
            #[cfg(target_os = "macos")]
            SettingsMsg::GStreamerFailed(err) => {
                self.gst_cancel_flag = None;
                self.gst_installed = refresh_gst_ui(
                    &self.gst_status_row,
                    &self.gst_install_btn,
                    &self.gst_remove_btn,
                    &self.gst_cancel_btn,
                    &self.gst_progress_bar,
                );
                let alert = gtk::AlertDialog::builder()
                    .message("Download Failed")
                    .detail(&err)
                    .build();
                alert.set_buttons(&["OK"]);
                alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
            }
            #[cfg(target_os = "macos")]
            SettingsMsg::RemoveGStreamer => {
                let gst_dir = download::runtimes_dir().join("gstreamer");
                if gst_dir.exists() {
                    let _ = std::fs::remove_dir_all(&gst_dir);
                }
                self.gst_installed = refresh_gst_ui(
                    &self.gst_status_row,
                    &self.gst_install_btn,
                    &self.gst_remove_btn,
                    &self.gst_cancel_btn,
                    &self.gst_progress_bar,
                );
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

// ── Root page builder ────────────────────────────────────────────────────

fn build_root_page(
    sender: &AsyncComponentSender<SettingsWindow>,
) -> (adw::NavigationPage, adw::PreferencesPage, adw::ActionRow, adw::ActionRow) {
    let nav_page = adw::NavigationPage::builder()
        .title("Tequila Settings")
        .can_pop(false)
        .build();

    let prefs_page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();

    // Wine Runtime row
    let runtime_row = adw::ActionRow::builder()
        .title("Wine Runtime")
        .subtitle("Loading...")
        .activatable(true)
        .build();
    {
        let s = sender.clone();
        runtime_row.connect_activated(move |_| {
            s.input(SettingsMsg::ShowRuntime);
        });
    }
    group.add(&runtime_row);

    // Graphics row
    let graphics_row = adw::ActionRow::builder()
        .title("Graphics Backends")
        .subtitle("Checking...")
        .activatable(true)
        .build();
    {
        let s = sender.clone();
        graphics_row.connect_activated(move |_| {
            s.input(SettingsMsg::ShowGraphics);
        });
    }
    group.add(&graphics_row);

    prefs_page.add(&group);
    nav_page.set_child(Some(&prefs_page));

    (nav_page, prefs_page, runtime_row, graphics_row)
}

// ── GStreamer section (inline on root page, macOS only) ──────────────────

#[cfg(target_os = "macos")]
fn init_gst_css() {
    use gtk::gdk::Display;
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".gst-installed { background-color: rgba(76, 175, 80, 0.12); }"
    );
    if let Some(display) = Display::default() {
        gtk::StyleContext::add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(target_os = "macos")]
fn is_gstreamer_available() -> bool {
    // Managed installation — must have valid version.txt
    let gst_dir = download::runtimes_dir().join("gstreamer");
    if let Ok(ver) = std::fs::read_to_string(gst_dir.join("version.txt")) {
        if !ver.trim().is_empty() {
            return true;
        }
    }
    // Homebrew GStreamer — brew prefix + key binary
    if let Ok(output) = std::process::Command::new("brew")
        .args(["--prefix", "gstreamer"])
        .output()
    {
        if output.status.success() {
            if let Ok(prefix) = String::from_utf8(output.stdout) {
                let p = std::path::Path::new(prefix.trim());
                if p.join("bin").join("gst-launch-1.0").exists() {
                    return true;
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
            return true;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn refresh_gst_ui(
    status_row: &adw::ActionRow,
    install_btn: &gtk::Button,
    remove_btn: &gtk::Button,
    cancel_btn: &gtk::Button,
    progress_bar: &gtk::LevelBar,
) -> bool {
    let available = is_gstreamer_available();
    progress_bar.set_visible(false);
    progress_bar.set_value(0.0);
    cancel_btn.set_visible(false);

    if available {
        let gst_dir = download::runtimes_dir().join("gstreamer");
        let managed = std::fs::read_to_string(gst_dir.join("version.txt"))
            .ok()
            .and_then(|v| {
                let t = v.trim().to_string();
                if t.is_empty() { None } else { Some(t) }
            });
        let ver = managed.as_deref().unwrap_or("system");
        status_row.set_title("GStreamer");
        status_row.set_subtitle(&format!("✓ Installed ({})", ver));
        status_row.set_activatable(false);
        status_row.add_css_class("gst-installed");
        status_row.remove_css_class("gst-not-installed");
        install_btn.set_visible(false);
        remove_btn.set_visible(managed.is_some());
    } else {
        status_row.set_title("GStreamer");
        status_row.set_subtitle("Not installed");
        status_row.set_activatable(false);
        status_row.remove_css_class("gst-installed");
        status_row.add_css_class("gst-not-installed");
        install_btn.set_visible(true);
        remove_btn.set_visible(false);
    }
    available
}

#[cfg(target_os = "macos")]
fn build_gstreamer_section(
    prefs_page: &adw::PreferencesPage,
    sender: &AsyncComponentSender<SettingsWindow>,
    parent: &gtk::Window,
) -> (adw::ActionRow, gtk::Button, gtk::Button, gtk::Button, gtk::LevelBar) {
    init_gst_css();

    let gst_group = adw::PreferencesGroup::builder()
        .title("GStreamer")
        .description("Audio and video framework required by Wine on macOS")
        .build();

    let status_row = adw::ActionRow::new();
    status_row.set_valign(gtk::Align::Center);
    let install_btn = gtk::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text("Install GStreamer")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .build();
    let remove_btn = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove GStreamer")
        .css_classes(["flat", "circular", "destructive-action"])
        .valign(gtk::Align::Center)
        .build();

    let progress_bar = gtk::LevelBar::builder()
        .visible(false)
        .min_value(0.0)
        .max_value(1.0)
        .width_request(80)
        .valign(gtk::Align::Center)
        .build();
    let cancel_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Cancel download")
        .css_classes(["flat", "circular"])
        .visible(false)
        .valign(gtk::Align::Center)
        .build();
    let outer_progress_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_top(12)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::End)
        .spacing(6)
        .build();
    outer_progress_box.append(&progress_bar);
    outer_progress_box.append(&cancel_btn);

    // Install button
    {
        let s = sender.clone();
        let parent = parent.clone();
        install_btn.connect_clicked(move |_| {
            let alert = gtk::AlertDialog::builder()
                .message("Install GStreamer?")
                .detail("GStreamer is required for audio and video support in Wine. This will download and install the GStreamer runtime package (~500 MB).")
                .build();
            alert.set_buttons(&["Cancel", "Install"]);
            alert.set_cancel_button(0);
            alert.set_default_button(1);
            let s = s.clone();
            alert.choose(Some(&parent), None::<&gtk::gio::Cancellable>, move |result| {
                if result == Ok(1) {
                    let _ = s.input(SettingsMsg::InstallGStreamer);
                }
            });
        });
    }
    {
        let s = sender.clone();
        remove_btn.connect_clicked(move |_| {
            s.input(SettingsMsg::RemoveGStreamer);
        });
    }
    {
        let s = sender.clone();
        cancel_btn.connect_clicked(move |_| {
            s.input(SettingsMsg::CancelGStreamer);
        });
    }

    let suffix_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(4)
        .build();
    suffix_box.append(&install_btn);
    suffix_box.append(&remove_btn);
    suffix_box.append(&outer_progress_box);
    status_row.add_suffix(&suffix_box);
    gst_group.add(&status_row);

    prefs_page.add(&gst_group);

    (status_row, install_btn, remove_btn, cancel_btn, progress_bar)
}
