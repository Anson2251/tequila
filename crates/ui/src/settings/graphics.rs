use adw::prelude::*;
use prefix::runtime;
use relm4::prelude::*;
use tracker;

use super::managed_download_row;

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct GraphicsSettings {
    #[tracker::do_not_track]
    installed_group: adw::PreferencesGroup,
    #[tracker::do_not_track]
    rows: Vec<adw::ActionRow>,
    #[tracker::do_not_track]
    available_ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GraphicsSettingsMsg {
    RefreshInstalled,
}

#[derive(Debug)]
pub enum GraphicsSettingsOutput {
    Changed,
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for GraphicsSettings {
    type Init = ();
    type Input = GraphicsSettingsMsg;
    type Output = GraphicsSettingsOutput;
    type CommandOutput = ();
    type Widgets = GraphicsSettingsWidgets;

    view! {
        #[root]
        adw::NavigationPage {
            set_title: &crate::t!("settings.graphics.title"),
            set_child: Some(&prefs_page),
        },

        #[name = "prefs_page"]
        adw::PreferencesPage {
            #[name = "installed_group"]
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.graphics.installed"),
            },

            #[name = "avail_group"]
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.graphics.available"),
                set_description: Some(&crate::t!("settings.graphics.available_desc")),
            },
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Placeholder groups — replaced with real ones from view! after view_output!()
        let placeholder_group = adw::PreferencesGroup::new();
        let mut model = GraphicsSettings {
            installed_group: placeholder_group,
            rows: Vec::new(),
            available_ctrls: Vec::new(),
            tracker: 0,
        };

        let widgets = view_output!();

        // Replace placeholder with the real widget from view!
        model.installed_group = widgets.installed_group.clone();

        // Populate the groups
        refresh_graphics_list(&model.installed_group, &mut model.rows);
        model.available_ctrls = build_available_graphics_rows(&widgets.avail_group, &sender);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            GraphicsSettingsMsg::RefreshInstalled => {
                refresh_graphics_list(&self.installed_group, &mut self.rows);
                let _ = sender.output(GraphicsSettingsOutput::Changed);
            }
        }
    }
}

// ── Graphics list helpers ────────────────────────────────────────────────

fn refresh_graphics_list(group: &adw::PreferencesGroup, rows: &mut Vec<adw::ActionRow>) {
    for row in rows.drain(..) {
        group.remove(&row);
    }

    let backends = runtime::graphics::installed_backends();

    if backends.is_empty() {
        let row = adw::ActionRow::builder()
            .title(&crate::t!("settings.graphics.no_backends"))
            .subtitle(&crate::t!("settings.graphics.no_backends_sub"))
            .activatable(false)
            .build();
        group.add(&row);
        rows.push(row);
        return;
    }

    for backend in &backends {
        let name = backend.display_name();
        let subtitle = crate::tf!("settings.graphics.version_format", "version" => &backend.version_string());

        let row = adw::ActionRow::builder()
            .title(name)
            .subtitle(&subtitle)
            .activatable(false)
            .build();

        group.add(&row);
        rows.push(row);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Check if any installed backend's directory name starts with the given prefix.
fn has_backend_dir(prefix: &str) -> bool {
    let dir = runtime::graphics::graphics_dir();
    if !dir.is_dir() {
        return false;
    }
    std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with(prefix)
        })
}

/// Build a `check_status` closure for a backend type.
fn make_check_status(
    prefix: &'static str,
    status_installed: String,
    status_missing: String,
) -> Box<dyn Fn() -> managed_download_row::DownloadRowStatus + Send + 'static> {
    Box::new(move || {
        let installed = has_backend_dir(prefix);
        managed_download_row::DownloadRowStatus {
            installed,
            managed: installed,
            status_text: if installed {
                status_installed.clone()
            } else {
                status_missing.clone()
            },
        }
    })
}

/// Build a `perform_remove` closure for a backend type.
fn make_remove_backends(
    prefix: &'static str,
) -> Box<dyn FnMut() -> Result<(), String> + Send + 'static> {
    Box::new(move || runtime::graphics::remove_backends(prefix).map_err(|e| e.to_string()))
}

// ── Available backends — each gets its own ManagedDownloadRow ───────────

#[cfg_attr(target_os = "macos", allow(unused_variables, unused_mut))]
fn build_available_graphics_rows(
    group: &adw::PreferencesGroup,
    sender: &AsyncComponentSender<GraphicsSettings>,
) -> Vec<AsyncController<managed_download_row::ManagedDownloadRow>> {
    let mut ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>> = Vec::new();

    // macOS: DXMT and D3DMetal are no longer offered as separate downloads.
    // The Anson2251 crossover-foss build bundles DXMT directly (see Wine Runtime settings).

    #[cfg(target_os = "linux")]
    {
        // ── DXVK + VKD3D ──
        let dxvk_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: crate::t!("settings.graphics.dxvk_vkd3d"),
                check_status: make_check_status(
                    "dxvk-",
                    crate::t!("settings.graphics.installed_status"),
                    crate::t!("settings.graphics.dxvk_vkd3d_desc"),
                ),
                check_update: None,
                start_download: Box::new(|_data_dir, progress, cancel| {
                    Box::pin(async move {
                        let simple_prog: runtime::download::ProgressFn = Box::new(move |d, t| {
                            progress(d, t, runtime::download::InstallPhase::Download);
                        });

                        let (v_version, v_url) =
                            runtime::graphics::fetch_dxvk_release(&prefix::github_client())
                                .await
                                .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(crate::t!("settings.graphics.cancelled").into());
                        }
                        runtime::graphics::download_linux_backend(
                            "dxvk",
                            &v_version,
                            &v_url,
                            false,
                            &simple_prog,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(crate::t!("settings.graphics.cancelled").into());
                        }
                        let (v3_version, v3_url) =
                            runtime::graphics::fetch_vkd3d_release(&prefix::github_client())
                                .await
                                .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(crate::t!("settings.graphics.cancelled").into());
                        }
                        runtime::graphics::download_linux_backend(
                            "vkd3d",
                            &v3_version,
                            &v3_url,
                            true,
                            &simple_prog,
                        )
                        .await
                        .map_err(|e| e.to_string())?;

                        Ok(())
                    })
                }),
                perform_remove: make_remove_backends("dxvk-"),
                data_dir: Default::default(),
            })
            .forward(sender.input_sender(), |_out| {
                GraphicsSettingsMsg::RefreshInstalled
            });
        group.add(dxvk_ctrl.widget());
        ctrls.push(dxvk_ctrl);
    }

    ctrls
}


