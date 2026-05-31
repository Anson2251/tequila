use adw::prelude::*;
use prefix::{
    GraphicsBackend, Manager as PrefixManager,
    runtime::{self, Channel, RuntimeManager, RuntimeSource},
};
use relm4::prelude::*;
use std::path::PathBuf;
use tracker;

use super::managed_download_row;

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct RuntimeSettings {
    pub prefix_manager: PrefixManager,
    parent: gtk::Window,

    #[tracker::do_not_track]
    list_group: adw::PreferencesGroup,
    #[tracker::do_not_track]
    rows: Vec<adw::ActionRow>,
    #[tracker::do_not_track]
    available_ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum RuntimeSettingsMsg {
    RefreshRuntimes,
    SetDefault(String),
    RemoveRuntime(String),
    DownloadComplete(RuntimeManager),
    ImportRuntime,
    ImportFromPath(PathBuf),
}

#[derive(Debug)]
pub enum RuntimeSettingsOutput {
    RuntimesUpdated(RuntimeManager),
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for RuntimeSettings {
    type Init = (PrefixManager, gtk::Window);
    type Input = RuntimeSettingsMsg;
    type Output = RuntimeSettingsOutput;
    type CommandOutput = ();
    type Widgets = RuntimeSettingsWidgets;

    view! {
        #[root]
        adw::NavigationPage {
            set_title: "Wine Runtime",
            set_child: Some(&prefs_page),
        },

        #[name = "prefs_page"]
        adw::PreferencesPage {
            #[name = "list_group"]
            adw::PreferencesGroup {
                set_title: "Installed Runtimes",
            },

            adw::PreferencesGroup {
                adw::ActionRow {
                    set_title: "Import from Disk",
                    set_subtitle: "Select an existing Wine installation folder",
                    set_activatable: true,
                    connect_activated[sender] => move |_| {
                        sender.input(RuntimeSettingsMsg::ImportRuntime);
                    },
                },
            },

            #[name = "avail_group"]
            adw::PreferencesGroup {
                set_title: "Available",
                set_description: Some("Download Wine runtimes"),
            },
        }
    }

    async fn init(
        (prefix_manager, parent): Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Placeholder groups — replaced with real ones from view! after view_output!()
        let placeholder_group = adw::PreferencesGroup::new();
        let mut model = RuntimeSettings {
            prefix_manager,
            parent,
            list_group: placeholder_group,
            rows: Vec::new(),
            available_ctrls: Vec::new(),
            tracker: 0,
        };

        let widgets = view_output!();

        // Replace placeholder with the real widget from view!
        model.list_group = widgets.list_group.clone();

        // Populate the groups
        refresh_runtime_list(
            &model.list_group,
            model.prefix_manager.runtime_manager(),
            &sender,
            &mut model.rows,
        );
        model.available_ctrls =
            build_available_channels(&widgets.avail_group, &model.prefix_manager, &sender).await;

        // Platform-specific group description
        #[cfg(target_os = "macos")]
        widgets
            .avail_group
            .set_description(Some("Download Wine runtimes from Homebrew"));
        #[cfg(not(target_os = "macos"))]
        widgets.avail_group.set_description(Some(
            "Download Wine runtimes from Kron4ek/Wine-Builds (requires 32-bit libraries)",
        ));

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.reset();
        match msg {
            RuntimeSettingsMsg::RefreshRuntimes => {
                // Re-detect system Wine in case it was installed/uninstalled/updated
                self.prefix_manager
                    .runtime_manager_mut()
                    .ensure_system_runtime();
                refresh_runtime_list(
                    &self.list_group,
                    self.prefix_manager.runtime_manager(),
                    &sender,
                    &mut self.rows,
                );
            }
            RuntimeSettingsMsg::SetDefault(id) => {
                self.prefix_manager.set_default_runtime(&id);
                self.prefix_manager.save_runtime_state();
                refresh_runtime_list(
                    &self.list_group,
                    self.prefix_manager.runtime_manager(),
                    &sender,
                    &mut self.rows,
                );
                emit_runtimes_updated(&self.prefix_manager, &sender);
            }
            RuntimeSettingsMsg::RemoveRuntime(id) => {
                if id != "wine-system" {
                    // Delete the runtime directory from disk
                    let dir = prefix::runtime::download::runtimes_dir().join(&id);
                    if dir.exists() {
                        let _ = std::fs::remove_dir_all(&dir);
                    }
                    // Remove from the runtime list and save config
                    self.prefix_manager.remove_runtime(&id);
                    refresh_runtime_list(
                        &self.list_group,
                        self.prefix_manager.runtime_manager(),
                        &sender,
                        &mut self.rows,
                    );
                    emit_runtimes_updated(&self.prefix_manager, &sender);
                    // Refresh the Available section rows so they show Install again
                    for ctrl in &self.available_ctrls {
                        ctrl.emit(managed_download_row::ManagedDownloadRowMsg::RefreshStatus);
                    }
                }
            }
            RuntimeSettingsMsg::DownloadComplete(updated_rm) => {
                let rm_ref = self.prefix_manager.runtime_manager_mut();
                let _old = std::mem::replace(rm_ref, updated_rm);
                self.prefix_manager.save_runtime_state();
                refresh_runtime_list(
                    &self.list_group,
                    self.prefix_manager.runtime_manager(),
                    &sender,
                    &mut self.rows,
                );
                emit_runtimes_updated(&self.prefix_manager, &sender);
                // Refresh Available rows so their check_status picks up the new state
                for ctrl in &self.available_ctrls {
                    ctrl.emit(managed_download_row::ManagedDownloadRowMsg::RefreshStatus);
                }
            }
            RuntimeSettingsMsg::ImportRuntime => {
                #[cfg(target_os = "macos")]
                macos_import_dialog(&sender);
                #[cfg(not(target_os = "macos"))]
                {
                    let file_dialog = gtk::FileDialog::builder()
                        .title("Select Wine Installation")
                        .build();
                    let s = sender.clone();
                    file_dialog.select_folder(
                        Some(&self.parent),
                        None::<&gtk::gio::Cancellable>,
                        move |result| {
                            if let Ok(file) = result {
                                if let Some(path) = file.path() {
                                    let _ = s.input(RuntimeSettingsMsg::ImportFromPath(path));
                                }
                            }
                        },
                    );
                }
            }
            RuntimeSettingsMsg::ImportFromPath(path) => {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("imported")
                    .to_string();
                match self.prefix_manager.import_runtime(&path, &dir_name) {
                    Ok(_runtime) => {
                        self.prefix_manager.save_runtime_state();
                        refresh_runtime_list(
                            &self.list_group,
                            self.prefix_manager.runtime_manager(),
                            &sender,
                            &mut self.rows,
                        );
                        emit_runtimes_updated(&self.prefix_manager, &sender);
                    }
                    Err(e) => {
                        let alert = adw::AlertDialog::new(
                            Some("Import Failed"),
                            Some(&format!("Failed to import Wine runtime:\n{}", e)),
                        );
                        alert.add_response("ok", "OK");
                        alert.set_default_response(Some("ok"));
                        alert.set_close_response("ok");
                        alert.choose(Some(&self.parent), None::<&gtk::gio::Cancellable>, |_| {});
                    }
                }
            }
        }
    }
}

// ── Available channel rows (ManagedDownloadRow per channel) ──────────────

/// Build the "Available" section of the Wine Runtime settings page.
///
/// On macOS the rows come from Homebrew casks (Stable / Staging / Devel).
/// On Linux they are fetched from Kron4ek/Wine-Builds (all versions).
#[cfg(target_os = "macos")]
async fn build_available_channels(
    group: &adw::PreferencesGroup,
    prefix_manager: &PrefixManager,
    sender: &AsyncComponentSender<RuntimeSettings>,
) -> Vec<AsyncController<managed_download_row::ManagedDownloadRow>> {
    let mut ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>> = Vec::new();

    for (channel, display_name, description) in [
        (Channel::Stable, "Stable", "Latest stable Wine release"),
        (Channel::Staging, "Staging", "Wine with performance patches"),
        (Channel::Devel, "Devel", "Development version (unstable)"),
    ] {
        let pm = prefix_manager.clone();
        let runtime_id = channel.runtime_id().to_string();

        // ── check_status (checks filesystem directly, not a stale in-memory snapshot) ──
        let check_id = runtime_id.clone();
        let check_status = Box::new(move || {
            let dir = prefix::runtime::download::runtimes_dir().join(&check_id);
            let installed = dir.is_dir();
            managed_download_row::DownloadRowStatus {
                installed,
                managed: installed,
                status_text: if installed {
                    format!("✓ Installed ({})", display_name)
                } else {
                    description.to_string()
                },
            }
        });

        // ── start_download (spawns blocking work on a thread to avoid freezing the UI) ──
        let dl_pm = pm.clone();
        let dl_sender = sender.clone();
        let dl_channel = channel;
        let start_download: managed_download_row::DownloadFn = Box::new(
            move |_data_dir: PathBuf,
                  progress: prefix::runtime::download::PhaseProgressFn,
                  cancel: std::sync::Arc<std::sync::atomic::AtomicBool>| {
                let pm = dl_pm.clone();
                let s = dl_sender.clone();
                let channel = dl_channel.clone();
                Box::pin(async move {
                    // Shared state: (downloaded_bytes, total_bytes, phase)
                    // phase: 0=Download, 1=Verify, 2=Extract
                    let dl_state = std::sync::Arc::new(std::sync::Mutex::new((0u64, 0u64, 0u8)));
                    let dl_state_t = dl_state.clone();

                    let (tx, rx) = std::sync::mpsc::channel::<
                        Result<RuntimeManager, prefix::base::error::PrefixError>,
                    >();

                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new()
                            .expect("Failed to create tokio runtime for download thread");

                        // Load latest state from disk
                        let mut runtime_manager: RuntimeManager =
                            if let Some(settings) = prefix::Settings::load() {
                                settings.into()
                            } else {
                                RuntimeManager::new()
                            };
                        runtime_manager.ensure_system_runtime();

                        // 1. Fetch cask info (needs tokio runtime for reqwest)
                        let cask = match rt
                            .block_on(prefix::runtime::homebrew::fetch_cask(channel.cask_name()))
                        {
                            Ok(c) => c,
                            Err(e) => {
                                let _ = tx.send(Err(e.into()));
                                return;
                            }
                        };

                        // 2. Setup temp directory
                        let runtimes_dir = prefix::runtime::download::runtimes_dir();
                        let _ = std::fs::create_dir_all(&runtimes_dir);
                        prefix::runtime::download::cleanup_temp_runtimes(&runtimes_dir);
                        let runtime_id = channel.runtime_id();
                        let tmp_dir = runtimes_dir.join(format!(".tmp-{}", runtime_id));
                        let final_dir = runtimes_dir.join(runtime_id);
                        let _ = std::fs::remove_dir_all(&tmp_dir);
                        let _ = std::fs::create_dir_all(&tmp_dir);
                        let archive_path = tmp_dir.join("wine.tar.xz");

                        // 3. Download (reports phase 0 via simple_prog)
                        let dl_state_prog = dl_state_t.clone();
                        let simple_prog: runtime::download::ProgressFn = Box::new(move |d, t| {
                            *dl_state_prog.lock().unwrap() = (d, t, 0u8);
                        });
                        if let Err(e) = rt.block_on(prefix::runtime::download::download_file(
                            &cask.url,
                            &archive_path,
                            &simple_prog,
                        )) {
                            let _ = tx.send(Err(e));
                            return;
                        }

                        // 4. Verify checksum (slow — UI shows "Verifying checksum...")
                        *dl_state_t.lock().unwrap() = (1, 1, 1u8);
                        if let Err(e) =
                            prefix::runtime::download::verify_sha256(&archive_path, &cask.sha256)
                        {
                            let _ = tx.send(Err(e));
                            return;
                        }

                        // 5. Extract archive (slow — UI shows "Unpacking...")
                        *dl_state_t.lock().unwrap() = (1, 1, 2u8);
                        if let Err(e) =
                            prefix::runtime::download::extract_tar(&archive_path, &tmp_dir)
                        {
                            let _ = tx.send(Err(e));
                            return;
                        }

                        // 6. Finalize — verify extraction, clean up, move into place
                        if let Err(e) = prefix::runtime::download::find_wine_binary(&tmp_dir) {
                            let _ = tx.send(Err(e));
                            return;
                        }
                        let _ = std::fs::remove_file(&archive_path);
                        if final_dir.exists() {
                            let _ = std::fs::remove_dir_all(&final_dir);
                        }
                        if let Err(e) = std::fs::rename(&tmp_dir, &final_dir) {
                            let _ = tx.send(Err(prefix::base::error::PrefixError::Io(e)));
                            return;
                        }

                        // 7. Register in runtime manager and save
                        runtime_manager.register_channel(channel, cask.version, final_dir);
                        let settings: prefix::Settings = runtime_manager.clone().into();
                        if let Err(e) = settings.save() {
                            eprintln!("Failed to save runtime settings: {}", e);
                        }
                        let rm = runtime_manager;
                        let _ = tx.send(Ok(rm));
                    });

                    // Poll for completion
                    loop {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err("Download cancelled".into());
                        }

                        // Bridge progress from shared state (includes phase)
                        {
                            let (d, t, phase) = *dl_state.lock().unwrap();
                            if t > 0 {
                                let phase = match phase {
                                    1 => runtime::download::InstallPhase::Verify,
                                    2 => runtime::download::InstallPhase::Extract,
                                    _ => runtime::download::InstallPhase::Download,
                                };
                                progress(d, t, phase);
                            }
                        }

                        match rx.try_recv() {
                            Ok(Ok(rm)) => {
                                let _ = s.input(RuntimeSettingsMsg::DownloadComplete(rm));
                                return Ok(());
                            }
                            Ok(Err(e)) => return Err(e.to_string()),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                gtk::glib::timeout_future(std::time::Duration::from_millis(200))
                                    .await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                return Err("Download thread crashed".into());
                            }
                        }
                    }
                })
            },
        );

        // ── perform_remove ──
        let remove_id = runtime_id.clone();
        let remove_sender = sender.clone();
        let perform_remove = Box::new(move || {
            // Delete the runtime directory from disk
            let dir = prefix::runtime::download::runtimes_dir().join(&remove_id);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }

            // Load the latest state from disk
            let mut runtime_manager: RuntimeManager =
                if let Some(settings) = prefix::Settings::load() {
                    settings.into()
                } else {
                    RuntimeManager::new()
                };
            runtime_manager.remove(&remove_id);
            let settings: prefix::Settings = runtime_manager.clone().into();
            if let Err(e) = settings.save() {
                eprintln!("Failed to save runtime settings: {}", e);
            }

            let _ = remove_sender.input(RuntimeSettingsMsg::DownloadComplete(runtime_manager));
            Ok(())
        });

        let ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: display_name.to_string(),
                check_status,
                check_update: None,
                start_download,
                perform_remove,
                data_dir: Default::default(),
            })
            .forward(sender.input_sender(), |_out| {
                RuntimeSettingsMsg::RefreshRuntimes
            });

        group.add(ctrl.widget());
        ctrls.push(ctrl);
    }

    ctrls
}

/// Build available Wine channels from Kron4ek/Wine-Builds (Linux).
///
/// Fetches all releases via the GitHub API and creates a download row for
/// every vanilla + Staging release.  Each row downloads the `amd64` / `x86`
/// archive matching the current system architecture.
#[cfg(not(target_os = "macos"))]
async fn build_available_channels(
    group: &adw::PreferencesGroup,
    _prefix_manager: &PrefixManager,
    sender: &AsyncComponentSender<RuntimeSettings>,
) -> Vec<AsyncController<managed_download_row::ManagedDownloadRow>> {
    use prefix::runtime::kron4ek::WineBuild;

    let mut ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>> = Vec::new();

    // ── Fetch available builds from GitHub API ──────────────────────
    let builds: Vec<WineBuild> = match prefix::runtime::kron4ek::fetch_all_builds().await {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to fetch Kron4ek builds: {}", e);
            let row = adw::ActionRow::builder()
                .title("Failed to fetch available Wine versions")
                .subtitle(&format!("{}", e))
                .build();
            group.add(&row);
            return ctrls;
        }
    };

    for build in builds {
        let runtime_id = format!("wine-{}", build.version);
        let base_version = build.version.trim_end_matches("-staging");
        let version_label = if build.is_staging {
            format!("{} (Staging)", base_version)
        } else {
            build.version.clone()
        };

        // ── check_status ───────────────────────────────────────────
        let check_id = runtime_id.clone();
        let chk_version = version_label.clone();
        let is_staging = build.is_staging;
        let check_status = Box::new(move || {
            let dir = prefix::runtime::download::runtimes_dir().join(&check_id);
            let installed = dir.is_dir();
            managed_download_row::DownloadRowStatus {
                installed,
                managed: installed,
                status_text: if installed {
                    format!("✓ Installed (Wine {})", chk_version)
                } else if is_staging {
                    "Wine with Staging patchset".to_string()
                } else {
                    "Vanilla Wine build".to_string()
                },
            }
        });

        // ── start_download ─────────────────────────────────────────
        let dl_sender = sender.clone();
        let dl_build = build.clone();
        let dl_runtime_id = runtime_id.clone();
        let start_download: managed_download_row::DownloadFn = Box::new(
            move |_data_dir: PathBuf,
                  progress: prefix::runtime::download::PhaseProgressFn,
                  cancel: std::sync::Arc<std::sync::atomic::AtomicBool>| {
                let s = dl_sender.clone();
                let build = dl_build.clone();
                let runtime_id = dl_runtime_id.clone();
                Box::pin(async move {
                    // Shared state: (downloaded_bytes, total_bytes, phase)
                    // phase: 0=Download, 1=Extract
                    let dl_state = std::sync::Arc::new(std::sync::Mutex::new((0u64, 0u64, 0u8)));
                    let dl_state_t = dl_state.clone();

                    let (tx, rx) = std::sync::mpsc::channel::<
                        Result<RuntimeManager, prefix::base::error::PrefixError>,
                    >();

                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new()
                            .expect("Failed to create tokio runtime for download thread");

                        // Load the latest runtime state from disk so that any
                        // runtimes deleted between row creation and now are gone.
                        let mut runtime_manager: RuntimeManager =
                            if let Some(settings) = prefix::Settings::load() {
                                let mut rm: RuntimeManager = settings.into();
                                rm.ensure_system_runtime();
                                rm
                            } else {
                                let mut rm = RuntimeManager::new();
                                rm.ensure_system_runtime();
                                rm
                            };

                        // 1. Setup temp directory
                        let runtimes_dir = prefix::runtime::download::runtimes_dir();
                        let _ = std::fs::create_dir_all(&runtimes_dir);
                        prefix::runtime::download::cleanup_temp_runtimes(&runtimes_dir);
                        let tmp_dir = runtimes_dir.join(format!(".tmp-{}", runtime_id));
                        let final_dir = runtimes_dir.join(&runtime_id);
                        let _ = std::fs::remove_dir_all(&tmp_dir);
                        let _ = std::fs::create_dir_all(&tmp_dir);
                        let archive_path = tmp_dir.join(&build.archive_name);

                        // 2. Download (phase 0)
                        let dl_state_prog = dl_state_t.clone();
                        let simple_prog: runtime::download::ProgressFn = Box::new(move |d, t| {
                            *dl_state_prog.lock().unwrap() = (d, t, 0u8);
                        });
                        if let Err(e) = rt.block_on(prefix::runtime::download::download_file(
                            &build.archive_url,
                            &archive_path,
                            &simple_prog,
                        )) {
                            let _ = tx.send(Err(e));
                            return;
                        }

                        // 3. Extract (phase 1)
                        *dl_state_t.lock().unwrap() = (1, 1, 1u8);
                        if let Err(e) =
                            prefix::runtime::download::extract_tar(&archive_path, &tmp_dir)
                        {
                            let _ = tx.send(Err(e));
                            return;
                        }

                        // Remove the archive file so find_content_dir below
                        // only sees the extracted content directory.
                        let _ = std::fs::remove_file(&archive_path);

                        // 4. Resolve content root — Kron4ek archives contain a
                        //    top-level directory (e.g. wine-11.8-amd64/).
                        let content_dir =
                            match prefix::runtime::download::find_content_dir(&tmp_dir) {
                                Ok(d) => d,
                                Err(e) => {
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            };

                        // 5. Find wine binary inside the content root
                        if let Err(e) = prefix::runtime::download::find_wine_binary(&content_dir) {
                            let _ = tx.send(Err(e));
                            return;
                        }
                        let _ = std::fs::remove_file(&archive_path);
                        if final_dir.exists() {
                            let _ = std::fs::remove_dir_all(&final_dir);
                        }
                        // Rename content_dir (not tmp_dir) so we drop the
                        // synthetic top-level wrapper
                        if let Err(e) = std::fs::rename(&content_dir, &final_dir) {
                            let _ = tx.send(Err(prefix::base::error::PrefixError::Io(e)));
                            return;
                        }
                        // Clean up tmp_dir if content_dir != tmp_dir
                        if content_dir != tmp_dir {
                            let _ = std::fs::remove_dir_all(&tmp_dir);
                        }

                        // 5. Register in runtime manager and save
                        runtime_manager.register_version(
                            &build.version,
                            build.archive_url.clone(),
                            final_dir,
                        );
                        let settings: prefix::Settings = runtime_manager.clone().into();
                        if let Err(e) = settings.save() {
                            eprintln!("Failed to save runtime settings: {}", e);
                        }
                        let rm = runtime_manager;
                        let _ = tx.send(Ok(rm));
                    });

                    // Poll for completion
                    loop {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err("Download cancelled".into());
                        }

                        // Bridge progress from shared state
                        {
                            let (d, t, phase) = *dl_state.lock().unwrap();
                            if t > 0 {
                                let phase = match phase {
                                    1 => runtime::download::InstallPhase::Extract,
                                    _ => runtime::download::InstallPhase::Download,
                                };
                                progress(d, t, phase);
                            }
                        }

                        match rx.try_recv() {
                            Ok(Ok(rm)) => {
                                let _ = s.input(RuntimeSettingsMsg::DownloadComplete(rm));
                                return Ok(());
                            }
                            Ok(Err(e)) => return Err(e.to_string()),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                gtk::glib::timeout_future(std::time::Duration::from_millis(200))
                                    .await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                return Err("Download thread crashed".into());
                            }
                        }
                    }
                })
            },
        );

        // ── perform_remove ──
        let remove_id = runtime_id.clone();
        let remove_sender = sender.clone();
        let perform_remove = Box::new(move || {
            let dir = prefix::runtime::download::runtimes_dir().join(&remove_id);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }

            // Load the latest state from disk, remove the runtime, and save back.
            // We cannot use a stale in-memory clone to avoid resurrecting
            // previously-deleted runtimes.
            let mut runtime_manager: RuntimeManager =
                if let Some(settings) = prefix::Settings::load() {
                    settings.into()
                } else {
                    RuntimeManager::new()
                };
            runtime_manager.remove(&remove_id);
            let settings: prefix::Settings = runtime_manager.clone().into();
            if let Err(e) = settings.save() {
                eprintln!("Failed to save runtime settings: {}", e);
            }

            let rm = runtime_manager;
            let _ = remove_sender.input(RuntimeSettingsMsg::DownloadComplete(rm));
            Ok(())
        });

        let ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: format!("Wine {}", version_label),
                check_status,
                check_update: None,
                start_download,
                perform_remove,
                data_dir: Default::default(),
            })
            .forward(sender.input_sender(), |_out| {
                RuntimeSettingsMsg::RefreshRuntimes
            });

        group.add(ctrl.widget());
        ctrls.push(ctrl);
    }

    ctrls
}

// ── Runtime list helpers ─────────────────────────────────────────────────

fn refresh_runtime_list(
    group: &adw::PreferencesGroup,
    rm: &RuntimeManager,
    sender: &AsyncComponentSender<RuntimeSettings>,
    rows: &mut Vec<adw::ActionRow>,
) {
    for row in rows.drain(..) {
        group.remove(&row);
    }

    for runtime in &rm.runtimes {
        let is_default = runtime.id == rm.default_id;
        let is_system = matches!(runtime.source, RuntimeSource::System);

        let _source = match &runtime.source {
            RuntimeSource::System => "System (PATH)".to_string(),
            RuntimeSource::ManagedChannel {
                channel,
                installed_cask_version,
            } => {
                format!(
                    "Homebrew {} — cask {}",
                    channel.display_name(),
                    installed_cask_version
                )
            }
            RuntimeSource::ManagedVersion { source_url } => {
                if source_url.contains("Kron4ek") {
                    "Kron4ek".to_string()
                } else {
                    "Managed (versioned)".to_string()
                }
            }
            RuntimeSource::Imported {
                label,
                original_path,
            } => {
                format!("Imported: {} ({})", label, original_path.display())
            }
        };

        let mut subtitle = format!(
            "{} · Installed {}",
            runtime.wine_version,
            &runtime.installed_at[..10]
        );

        let gfx_names: Vec<&str> = runtime
            .graphics
            .iter()
            .map(|g| match g {
                GraphicsBackend::Dxmt { .. } => "DXMT",
                GraphicsBackend::D3DMetal { .. } => "D3DMetal",
                GraphicsBackend::DxvkVkd3d { .. } => "DXVK+VKD3D",
            })
            .collect();
        if !gfx_names.is_empty() {
            subtitle.push_str(&format!(" · {}", gfx_names.join(", ")));
        }

        let row = adw::ActionRow::builder()
            .title(&runtime.name)
            .subtitle(&subtitle)
            .activatable(true)
            .build();

        let radio = gtk::CheckButton::builder()
            .css_classes(["selection-mode"])
            .active(is_default)
            .valign(gtk::Align::Center)
            .build();
        row.add_prefix(&radio);
        {
            let id = runtime.id.clone();
            let s = sender.clone();
            radio.connect_toggled(move |r| {
                if r.is_active() {
                    s.input(RuntimeSettingsMsg::SetDefault(id.clone()));
                }
            });
        }

        if is_default {
            let badge = gtk::Label::builder()
                .label("Default")
                .css_classes(["badge", "accent"])
                .valign(gtk::Align::Center)
                .margin_end(4)
                .build();
            row.add_suffix(&badge);
        }

        if !is_system {
            let id = runtime.id.clone();
            let s = sender.clone();
            let remove_btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove Runtime")
                .css_classes(["flat", "circular", "destructive-action"])
                .valign(gtk::Align::Center)
                .build();
            remove_btn.connect_clicked(move |_| {
                s.input(RuntimeSettingsMsg::RemoveRuntime(id.clone()));
            });
            row.add_suffix(&remove_btn);
        }

        group.add(&row);
        rows.push(row);
    }
}

fn emit_runtimes_updated(pm: &PrefixManager, sender: &AsyncComponentSender<RuntimeSettings>) {
    let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(
        pm.runtime_manager().clone(),
    ));
}

// ── Native file dialog (macOS) ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_import_dialog(sender: &AsyncComponentSender<RuntimeSettings>) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSString;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&NSString::from_str("Select Wine Installation")));

    let panel_for_block = panel.clone();
    let s = sender.clone();
    let block = RcBlock::new(move |result: NSModalResponse| {
        if result == NSModalResponseOK {
            let urls = panel_for_block.URLs();
            if let Some(url) = urls.firstObject() {
                if let Some(path_str) = url.path() {
                    let path: String = path_str.to_string();
                    let _ = s.input(RuntimeSettingsMsg::ImportFromPath(PathBuf::from(path)));
                }
            }
        }
    });

    panel.beginWithCompletionHandler(&block);
}
