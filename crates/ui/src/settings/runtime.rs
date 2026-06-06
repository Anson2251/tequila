use adw::prelude::*;
#[cfg(target_os = "macos")]
use prefix::runtime::Channel;
use prefix::{
    GraphicsBackend, Manager as PrefixManager,
    runtime::{RuntimeManager, RuntimeSource},
};
use relm4::prelude::*;
use service::AppService;
use std::path::PathBuf;
use tracker;

use super::managed_download_row;

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct RuntimeSettings {
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

// ── Internal helpers ─────────────────────────────────────────────────────

impl RuntimeSettings {
    fn refresh_list(&mut self, sender: &AsyncComponentSender<Self>) {
        let svc = AppService::global();
        let pm = svc.prefix_manager();
        let rm = &*pm.read_runtime();
        refresh_runtime_list(&self.list_group, rm, sender, &mut self.rows);
    }
}

/// Build a `perform_remove` closure for a managed download row.
fn make_remove_runtime(
    runtime_id: String,
    sender: AsyncComponentSender<RuntimeSettings>,
) -> Box<dyn FnMut() -> Result<(), String> + Send + 'static> {
    Box::new(move || {
        let dir = prefix::runtime::download::runtimes_dir().join(&runtime_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        let mut runtime_manager: RuntimeManager = if let Some(settings) = prefix::Settings::load() {
            settings.into()
        } else {
            RuntimeManager::new()
        };
        runtime_manager.remove(&runtime_id);
        let settings: prefix::Settings = runtime_manager.clone().into();
        if let Err(e) = settings.save() {
            log::error!("[runtime] failed to save runtime settings: {}", e);
        }
        let _ = sender.input(RuntimeSettingsMsg::DownloadComplete(runtime_manager));
        Ok(())
    })
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for RuntimeSettings {
    type Init = gtk::Window;
    type Input = RuntimeSettingsMsg;
    type Output = RuntimeSettingsOutput;
    type CommandOutput = ();
    type Widgets = RuntimeSettingsWidgets;

    view! {
        #[root]
        adw::NavigationPage {
            set_title: &crate::t!("settings.runtime.title"),
            set_child: Some(&prefs_page),
        },

        #[name = "prefs_page"]
        adw::PreferencesPage {
            #[name = "list_group"]
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.runtime.installed"),
            },

            adw::PreferencesGroup {
                adw::ActionRow {
                    set_title: &crate::t!("settings.runtime.import_disk"),
                    set_subtitle: &crate::t!("settings.runtime.import_disk_sub"),
                    set_activatable: true,
                    connect_activated[sender] => move |_| {
                        sender.input(RuntimeSettingsMsg::ImportRuntime);
                    },
                },
            },

            #[name = "avail_group"]
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.runtime.available"),
                set_description: Some(&crate::t!("settings.runtime.available_desc")),
            },
        }
    }

    async fn init(
        parent: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Placeholder groups — replaced with real ones from view! after view_output!()
        let placeholder_group = adw::PreferencesGroup::new();
        let mut model = RuntimeSettings {
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
        let svc = AppService::global();
        let pm = svc.prefix_manager();
        refresh_runtime_list(
            &model.list_group,
            &*pm.read_runtime(),
            &sender,
            &mut model.rows,
        );
        model.available_ctrls = build_available_channels(&widgets.avail_group, &pm, &sender).await;

        // Platform-specific group description
        #[cfg(target_os = "macos")]
        widgets
            .avail_group
            .set_description(Some(&crate::t!("settings.runtime.available_desc_macos")));
        #[cfg(not(target_os = "macos"))]
        widgets.avail_group.set_description(Some(
            &crate::t!("settings.runtime.available_desc_linux"),
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
                service::runtime_ops::ensure_system_runtime();
                self.refresh_list(&sender);
            }
            RuntimeSettingsMsg::SetDefault(id) => {
                if let Ok(updated_rm) = service::runtime_ops::set_default_runtime(&id) {
                    // Replace the runtime manager in the SettingsWindow parent
                    let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(updated_rm));
                    self.refresh_list(&sender);
                }
            }
            RuntimeSettingsMsg::RemoveRuntime(id) => {
                if let Ok(updated_rm) = service::runtime_ops::remove_runtime_full(&id) {
                    let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(updated_rm));
                    self.refresh_list(&sender);
                    // Refresh the Available section rows so they show Install again
                    for ctrl in &self.available_ctrls {
                        ctrl.emit(managed_download_row::ManagedDownloadRowMsg::RefreshStatus);
                    }
                }
            }
            RuntimeSettingsMsg::DownloadComplete(updated_rm) => {
                // Replace the runtime manager in the global service
                let svc = AppService::global();
                let pm = svc.prefix_manager_mut();
                *pm.write_runtime() = updated_rm;
                pm.save_runtime_state();
                drop(pm);

                let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(
                    svc.prefix_manager().clone_runtime(),
                ));
                self.refresh_list(&sender);
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
                        .title(&crate::t!("settings.runtime.select_wine"))
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
                match service::runtime_ops::import_runtime_from_path(&path, &dir_name) {
                    Ok(updated_rm) => {
                        let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(updated_rm));
                        self.refresh_list(&sender);
                    }
                    Err(e) => {
                        let msg = crate::tf!("settings.runtime.import_failed_desc", "error" => &e);
                        let alert = adw::AlertDialog::new(
                            Some(&crate::t!("settings.runtime.import_failed")),
                            Some(&msg),
                        );
                        alert.add_response("ok", &crate::t!("dialogs.ok"));
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
        (Channel::Stable, crate::t!("settings.runtime.stable"), crate::t!("settings.runtime.stable_desc")),
        (Channel::Staging, crate::t!("settings.runtime.staging"), crate::t!("settings.runtime.staging_desc")),
        (Channel::Devel, crate::t!("settings.runtime.devel"), crate::t!("settings.runtime.devel_desc")),
    ] {
        let pm = prefix_manager.clone();
        let runtime_id = channel.runtime_id().to_string();

        // ── check_status (checks filesystem directly, not a stale in-memory snapshot) ──
        let check_id = runtime_id.clone();
        // Clone outside closure since both closure and outer scope need it
        let dn_for_title = display_name.clone();
        let check_status = Box::new(move || {
            let dir = prefix::runtime::download::runtimes_dir().join(&check_id);
            let installed = dir.is_dir();
            managed_download_row::DownloadRowStatus {
                installed,
                managed: installed,
                status_text: if installed {
                    crate::tf!("settings.runtime.installed_channel", "name" => &display_name)
                } else {
                    description.clone()
                },
            }
        });

        // ── start_download ──
        let dl_sender = sender.clone();
        let dl_channel = channel;
        let start_download: managed_download_row::DownloadFn = Box::new(
            move |_data_dir: PathBuf,
                  progress: prefix::runtime::download::PhaseProgressFn,
                  cancel: std::sync::Arc<std::sync::atomic::AtomicBool>| {
                let s = dl_sender.clone();
                let channel = dl_channel.clone();
                Box::pin(async move {
                    let (tx, rx) = std::sync::mpsc::channel::<Result<RuntimeManager, String>>();

                    std::thread::spawn(move || {
                        let rt =
                            tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

                        let result: Result<RuntimeManager, String> = rt.block_on(async {
                            // 1. Download + verify + extract with phase progress
                            let final_dir = prefix::runtime::download::install_channel_with_phase(
                                &channel, &progress,
                            )
                            .await
                            .map_err(|e| e.to_string())?;

                            // 2. Fetch cask for version info
                            let cask = prefix::runtime::homebrew::fetch_cask(channel.cask_name())
                                .await
                                .map_err(|e| e.to_string())?;

                            // 3. Load state, register, save
                            let mut runtime_manager: RuntimeManager =
                                if let Some(settings) = prefix::Settings::load() {
                                    settings.into()
                                } else {
                                    RuntimeManager::new()
                                };
                            runtime_manager.ensure_system_runtime();
                            runtime_manager.register_channel(channel, cask.version, final_dir);
                            let settings: prefix::Settings = runtime_manager.clone().into();
                            if let Err(e) = settings.save() {
                                log::error!("[runtime] failed to save: {}", e);
                            }
                            Ok(runtime_manager)
                        });
                        let _ = tx.send(result);
                    });

                    // Poll for completion
                    loop {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(crate::t!("settings.runtime.download_cancelled").into());
                        }
                        match rx.try_recv() {
                            Ok(Ok(rm)) => {
                                let _ = s.input(RuntimeSettingsMsg::DownloadComplete(rm));
                                return Ok(());
                            }
                            Ok(Err(e)) => return Err(e),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                gtk::glib::timeout_future(std::time::Duration::from_millis(200))
                                    .await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                return Err(crate::t!("settings.runtime.download_crashed").into());
                            }
                        }
                    }
                })
            },
        );

        // ── perform_remove ──
        let perform_remove = make_remove_runtime(runtime_id.clone(), sender.clone());

        let ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: dn_for_title,
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
    let api_key = prefix::Settings::load().and_then(|s| s.github_api_key);
    let builds: Vec<WineBuild> =
        match prefix::runtime::kron4ek::fetch_all_builds(api_key.as_deref()).await {
            Ok(b) => b,
            Err(e) => {
                log::error!("[runtime] failed to fetch Kron4ek builds: {}", e);
                let row = adw::ActionRow::builder()
                    .title(&crate::t!("settings.runtime.fetch_failed"))
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
            crate::tf!("settings.runtime.staging_label", "version" => base_version)
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
                    crate::tf!("settings.runtime.installed_wine", "version" => &chk_version)
                } else if is_staging {
                    crate::t!("settings.runtime.staging_patchset")
                } else {
                    crate::t!("settings.runtime.vanilla_build")
                },
            }
        });

        // ── start_download ──
        let dl_sender = sender.clone();
        let dl_build = build.clone();
        let start_download: managed_download_row::DownloadFn = Box::new(
            move |_data_dir: PathBuf,
                  progress: prefix::runtime::download::PhaseProgressFn,
                  cancel: std::sync::Arc<std::sync::atomic::AtomicBool>| {
                let s = dl_sender.clone();
                let build = dl_build.clone();
                Box::pin(async move {
                    let (tx, rx) = std::sync::mpsc::channel::<Result<RuntimeManager, String>>();

                    std::thread::spawn(move || {
                        let rt =
                            tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

                        let result: Result<RuntimeManager, String> = rt.block_on(async {
                            // 1. Download + extract with phase progress
                            let final_dir = prefix::runtime::download::install_kron4ek_build(
                                &build.version,
                                &build.archive_url,
                                &build.archive_name,
                                &progress,
                            )
                            .await
                            .map_err(|e| e.to_string())?;

                            // 2. Load state, register, save
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
                            runtime_manager.register_version(
                                &build.version,
                                build.archive_url.clone(),
                                final_dir,
                            );
                            let settings: prefix::Settings = runtime_manager.clone().into();
                            if let Err(e) = settings.save() {
                                log::error!("[runtime] failed to save: {}", e);
                            }
                            Ok(runtime_manager)
                        });
                        let _ = tx.send(result);
                    });

                    // Poll for completion
                    loop {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(crate::t!("settings.runtime.download_cancelled").into());
                        }
                        match rx.try_recv() {
                            Ok(Ok(rm)) => {
                                let _ = s.input(RuntimeSettingsMsg::DownloadComplete(rm));
                                return Ok(());
                            }
                            Ok(Err(e)) => return Err(e),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                gtk::glib::timeout_future(std::time::Duration::from_millis(200))
                                    .await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                return Err(crate::t!("settings.runtime.download_crashed").into());
                            }
                        }
                    }
                })
            },
        );

        // ── perform_remove ──
        let perform_remove = make_remove_runtime(runtime_id.clone(), sender.clone());

        let ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: crate::tf!("settings.runtime.wine_version_title", "version" => &version_label),
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
            RuntimeSource::System => crate::t!("settings.runtime.system"),
            RuntimeSource::ManagedChannel {
                channel,
                installed_cask_version,
            } => {
                crate::tf!(
                    "settings.runtime.homebrew_source",
                    "channel" => &channel.display_name(),
                    "version" => &installed_cask_version,
                )
            }
            RuntimeSource::ManagedVersion { source_url } => {
                if source_url.contains("Kron4ek") {
                    "Kron4ek".to_string()
                } else {
                    crate::t!("settings.runtime.managed_versioned")
                }
            }
            RuntimeSource::Imported {
                label,
                original_path,
            } => {
                crate::tf!("settings.runtime.imported_label", "label" => &label, "path" => &original_path.display().to_string())
            }
        };

        let mut subtitle = crate::tf!(
            "settings.runtime.installed_format",
            "version" => &runtime.wine_version,
            "date" => &runtime.installed_at[..10].to_string(),
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
                .label(&crate::t!("settings.runtime.default"))
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
                .tooltip_text(&crate::t!("settings.runtime.remove"))
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
    panel.setTitle(Some(&NSString::from_str(&crate::t!("settings.runtime.select_wine"))));

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
