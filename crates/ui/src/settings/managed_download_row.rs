use adw::prelude::*;
use relm4::prelude::*;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tracker;

use prefix::runtime::download::{InstallPhase, PhaseProgressFn};

// ── Exported types ──────────────────────────────────────────────────────

/// Returned by the status-check callback to describe the current state.
pub struct DownloadRowStatus {
    pub installed: bool,
    pub managed: bool,
    pub status_text: String,
}

/// Asynchronous download hook.
///
/// Receives the data directory, a phase-aware progress reporter, and a
/// cancellation flag.  Return `Ok(())` on success or `Err(reason)` on failure.
///
/// The returned future runs via `spawn_future_local` (main thread), so it does
/// **not** need to be `Send`.
pub type DownloadFn = Box<
    dyn Fn(
            PathBuf,
            PhaseProgressFn,
            Arc<AtomicBool>,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + 'static>>
        + Send
        + 'static,
>;

// ── Initialisation ─────────────────────────────────────────────────────

pub struct ManagedDownloadRowInit {
    pub title: String,
    pub badge: Option<String>,
    pub check_status: Box<dyn Fn() -> DownloadRowStatus + Send + 'static>,
    pub check_update: Option<Box<dyn Fn() -> Option<String> + Send + 'static>>,
    pub start_download: DownloadFn,
    pub perform_remove: Box<dyn FnMut() -> Result<(), String> + Send + 'static>,
    pub data_dir: PathBuf,
}

// ── Messages ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ManagedDownloadRowMsg {
    /// User clicked the install button → shows a confirmation dialog.
    Install,
    /// Confirmation accepted → start the actual download.
    BeginInstall,
    /// User clicked the cancel button.
    Cancel,
    /// Download progress update.
    Progress(u64, u64, InstallPhase),
    /// Download completed successfully.
    Complete,
    /// Download failed with an error message.
    Failed(String),
    /// User clicked the remove button.
    Remove,
    /// Re-check current installed state (called after remove).
    RefreshStatus,
}

#[derive(Debug)]
pub enum ManagedDownloadRowOutput {
    StatusChanged,
    DownloadStarted,
    DownloadFinished,
    DownloadFailed(String),
    Removed,
}

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct ManagedDownloadRow {
    // Static config
    title: String,
    badge: Option<String>,

    // Tracked reactive state
    installed: bool,
    managed: bool,
    progress: f64,
    status_text: String,
    downloading: bool,
    cancellable: bool,

    // Speed tracking helpers (for large downloads like GStreamer)
    #[tracker::do_not_track]
    cancel_flag: Option<Arc<AtomicBool>>,
    #[tracker::do_not_track]
    last_bytes: u64,
    #[tracker::do_not_track]
    last_time: Instant,
    #[tracker::do_not_track]
    current_speed: f64,

    // Callbacks – kept in the model for reuse across message handlers.
    #[tracker::do_not_track]
    check_status: Box<dyn Fn() -> DownloadRowStatus + Send + 'static>,
    #[tracker::do_not_track]
    check_update: Option<Box<dyn Fn() -> Option<String> + Send + 'static>>,
    #[tracker::do_not_track]
    start_download: DownloadFn,
    #[tracker::do_not_track]
    perform_remove: Box<dyn FnMut() -> Result<(), String> + Send + 'static>,
    #[tracker::do_not_track]
    data_dir: PathBuf,
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for ManagedDownloadRow {
    type Init = ManagedDownloadRowInit;
    type Input = ManagedDownloadRowMsg;
    type Output = ManagedDownloadRowOutput;
    type CommandOutput = ();
    type Widgets = ManagedDownloadRowWidgets;

    view! {
        #[root]
        adw::ActionRow {
            set_title: &model.title,
            set_activatable: false,
            #[watch]
            set_subtitle: &model.status_text,
            #[track = "model.changed(ManagedDownloadRow::installed())"]
            set_class_active: ("managed-installed", model.installed),

            add_suffix = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::End,
                set_spacing: 4,

                // ── Pre-release badge ──
                #[name = "badge_label"]
                gtk::Label {
                    #[watch]
                    set_visible: model.badge.is_some(),
                    #[watch]
                    set_label: model.badge.as_ref().map(|s| s.as_str()).unwrap_or(""),
                    add_css_class: "badge-label",
                    add_css_class: "caption",
                    set_valign: gtk::Align::Center,
                    set_margin_end: 4,
                },

                // ── Install button ──
                #[name = "install_btn"]
                gtk::Button {
                    set_icon_name: "document-save-symbolic",
                    set_tooltip: &crate::t!("settings.runtime.install_btn"),
                    add_css_class: "flat",
                    set_valign: gtk::Align::Center,
                    add_css_class: "circular",
                    #[watch]
                    set_visible: !model.installed && !model.downloading,
                    connect_clicked => ManagedDownloadRowMsg::Install,
                },

                // ── Remove button (only for managed installations) ──
                #[name = "remove_btn"]
                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    set_tooltip: &crate::t!("settings.runtime.remove_btn"),
                    add_css_class: "flat",
                    set_valign: gtk::Align::Center,
                    add_css_class: "circular",
                    add_css_class: "destructive-action",
                    #[watch]
                    set_visible: model.managed && !model.downloading,
                    connect_clicked => ManagedDownloadRowMsg::Remove,
                },

                // ── Download progress area ──
                gtk::Box {
                    set_valign: gtk::Align::Center,
                    set_spacing: 6,
                    #[watch]
                    set_visible: model.downloading,

                    #[name = "progress_bar"]
                    gtk::LevelBar {
                        set_valign: gtk::Align::Center,
                        set_min_value: 0.0,
                        set_max_value: 1.0,
                        set_width_request: 80,
                        #[watch]
                        set_value: model.progress,
                    },

                    #[name = "cancel_btn"]
                    gtk::Button {
                        set_icon_name: "window-close-symbolic",
                        set_tooltip: &crate::t!("settings.runtime.cancel_btn"),
                        add_css_class: "flat",
                        add_css_class: "circular",
                        #[watch]
                        set_sensitive: model.cancellable,
                        connect_clicked => ManagedDownloadRowMsg::Cancel,
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Load once global CSS for the installed-highlight class
        init_css_once();

        // Run the status check to populate initial state
        let status = (init.check_status)();

        let model = ManagedDownloadRow {
            title: init.title,
            badge: init.badge,
            installed: status.installed,
            managed: status.managed,
            progress: 0.0,
            status_text: status.status_text,
            downloading: false,
            cancellable: false,
            cancel_flag: None,
            last_bytes: 0,
            last_time: Instant::now(),
            current_speed: 0.0,
            check_status: init.check_status,
            check_update: init.check_update,
            start_download: init.start_download,
            perform_remove: init.perform_remove,
            data_dir: init.data_dir,
            tracker: 0,
        };

        let widgets = view_output!();
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
            // ── Install: confirmation dialog ──
            ManagedDownloadRowMsg::Install => {
                let alert = adw::AlertDialog::new(
                    Some(&crate::tf!("settings.runtime.install_confirm_title", "name" => &self.title)),
                    Some(&crate::tf!(
                        "settings.runtime.install_confirm_body",
                        "name" => &self.title,
                    )),
                );
                alert.add_response("cancel", &crate::t!("dialogs.cancel"));
                alert.add_response("install", &crate::t!("settings.runtime.install_btn"));
                alert.set_response_appearance("install", adw::ResponseAppearance::Suggested);
                alert.set_default_response(Some("install"));
                alert.set_close_response("cancel");
                let s = sender.clone();
                alert.choose(
                    Some(root),
                    None::<&gtk::gio::Cancellable>,
                    move |response| {
                        if response == "install" {
                            let _ = s.input(ManagedDownloadRowMsg::BeginInstall);
                        }
                    },
                );
            }

            // ── Actual download start ──
            ManagedDownloadRowMsg::BeginInstall => {
                self.set_downloading(true);
                self.set_cancellable(true);
                self.set_progress(0.0);
                self.set_status_text(crate::t!("settings.runtime.starting_download"));
                self.last_bytes = 0;
                self.last_time = Instant::now();

                // Cancel any previous in-flight download first
                if let Some(old) = self.cancel_flag.take() {
                    old.store(true, Ordering::Relaxed);
                }
                let cancel = Arc::new(AtomicBool::new(false));
                self.cancel_flag = Some(cancel.clone());

                let _ = sender.output(ManagedDownloadRowOutput::DownloadStarted);

                let data_dir = self.data_dir.clone();

                // Clone sender for the progress closure (needs its own copy)
                let s_progress = sender.clone();
                let progress: PhaseProgressFn = Box::new(move |downloaded, total, phase| {
                    let _ =
                        s_progress.input(ManagedDownloadRowMsg::Progress(downloaded, total, phase));
                });
                // Call the download callback on the main thread to get the future,
                // then spawn it.  Box<dyn Fn> is callable from &mut self.
                let future = (self.start_download)(data_dir, progress, cancel);

                // Clone sender again for the completion future
                let s_completion = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    match future.await {
                        Ok(()) => {
                            let _ = s_completion.input(ManagedDownloadRowMsg::Complete);
                        }
                        Err(e) => {
                            let _ = s_completion.input(ManagedDownloadRowMsg::Failed(e));
                        }
                    }
                });
            }

            // ── Cancel ──
            ManagedDownloadRowMsg::Cancel => {
                if let Some(cancel) = &self.cancel_flag {
                    cancel.store(true, Ordering::Relaxed);
                }
            }

            // ── Download progress ──
            ManagedDownloadRowMsg::Progress(downloaded, total, phase) => {
                let now = Instant::now();
                let mb = |b: u64| b as f64 / 1_048_576.0;

                match phase {
                    InstallPhase::Download => {
                        let elapsed = now.duration_since(self.last_time);
                        if elapsed.as_secs_f64() > 0.5 {
                            self.current_speed = (downloaded.saturating_sub(self.last_bytes))
                                as f64
                                / elapsed.as_secs_f64();
                            self.last_bytes = downloaded;
                            self.last_time = now;
                        }

                        if total > 0 {
                            self.set_progress((downloaded as f64 / total as f64) * 0.8);
                            let speed_mb = self.current_speed / 1_048_576.0;
                            self.set_status_text(crate::tf!(
                                "settings.runtime.download_progress",
                                "downloaded" => &format!("{:.1}", mb(downloaded)),
                                "total" => &format!("{:.1}", mb(total)),
                                "speed" => &format!("{:.1}", speed_mb),
                            ));
                        }
                    }
                    InstallPhase::Verify => {
                        self.set_cancellable(false);
                        self.set_progress(0.80 + (downloaded as f64 / total.max(1) as f64) * 0.10);
                        self.set_status_text(crate::t!("settings.runtime.verifying_checksum"));
                    }
                    InstallPhase::Extract => {
                        self.set_cancellable(false);
                        self.set_progress(0.90 + (downloaded as f64 / total.max(1) as f64) * 0.10);
                        self.set_status_text(crate::t!("settings.runtime.unpacking"));
                    }
                }
            }

            // ── Complete ──
            ManagedDownloadRowMsg::Complete => {
                self.cancel_flag = None;
                self.set_cancellable(false);
                let status = (self.check_status)();
                self.set_installed(status.installed);
                self.set_managed(status.managed);
                self.set_status_text(status.status_text);
                self.set_downloading(false);

                let _ = sender.output(ManagedDownloadRowOutput::DownloadFinished);
                let _ = sender.output(ManagedDownloadRowOutput::StatusChanged);
            }

            // ── Failed ──
            ManagedDownloadRowMsg::Failed(err) => {
                self.cancel_flag = None;
                self.set_cancellable(false);
                let status = (self.check_status)();
                self.set_installed(status.installed);
                self.set_managed(status.managed);
                self.set_status_text(status.status_text);
                self.set_downloading(false);

                let _ = sender.output(ManagedDownloadRowOutput::DownloadFailed(err.clone()));

                let alert = adw::AlertDialog::new(Some(&crate::t!("settings.runtime.download_failed")), Some(&err));
                alert.add_response("ok", &crate::t!("dialogs.ok"));
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
            }

            // ── Remove ──
            ManagedDownloadRowMsg::Remove => {
                if let Err(e) = (self.perform_remove)() {
                    let alert = adw::AlertDialog::new(Some(&crate::t!("settings.runtime.remove_failed")), Some(&e));
                    alert.add_response("ok", &crate::t!("dialogs.ok"));
                    alert.set_default_response(Some("ok"));
                    alert.set_close_response("ok");
                    alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
                }

                let status = (self.check_status)();
                self.set_installed(status.installed);
                self.set_managed(status.managed);
                self.set_status_text(status.status_text);

                let _ = sender.output(ManagedDownloadRowOutput::Removed);
                let _ = sender.output(ManagedDownloadRowOutput::StatusChanged);
            }

            // ── Refresh ──
            ManagedDownloadRowMsg::RefreshStatus => {
                let status = (self.check_status)();
                self.set_installed(status.installed);
                self.set_managed(status.managed);
                self.set_status_text(status.status_text);
                let _ = sender.output(ManagedDownloadRowOutput::StatusChanged);
            }
        }
    }
}

// ── Global CSS (loaded once) ─────────────────────────────────────────────

fn init_css_once() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider
            .load_from_data(".managed-installed { background-color: rgba(76, 175, 80, 0.12); }\n\
                .badge-label {\n\
                    font-size: 0.8rem;\n\
                    font-weight: 600;\n\
                    padding: 2px 8px;\n\
                    border-radius: 10px;\n\
                    background-color: rgba(255, 179, 0, 0.2);\n\
                    color: #cc8800;\n\
                }");
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
