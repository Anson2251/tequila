use adw::prelude::*;
use relm4::prelude::*;
use tracker;
use prefix::runtime;

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
pub enum GraphicsSettingsMsg {}

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
            set_title: "Graphics Backends",
            set_child: Some(&prefs_page),
        },

        #[name = "prefs_page"]
        adw::PreferencesPage {
            #[name = "installed_group"]
            adw::PreferencesGroup {
                set_title: "Installed",
            },

            #[name = "avail_group"]
            adw::PreferencesGroup {
                set_title: "Available",
                set_description: Some("Translation layers that can improve DirectX performance"),
            },
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
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
        model.available_ctrls = build_available_graphics_rows(&widgets.avail_group);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {}
    }
}

// ── Graphics list helpers ────────────────────────────────────────────────

fn refresh_graphics_list(group: &adw::PreferencesGroup, rows: &mut Vec<adw::ActionRow>) {
    for row in rows.drain(..) {
        group.remove(&row);
    }

    let dir = runtime::graphics::graphics_dir();
    if !dir.is_dir() {
        let row = adw::ActionRow::builder()
            .title("No backends installed")
            .subtitle("Download graphics backends to improve DirectX performance")
            .activatable(false)
            .build();
        group.add(&row);
        rows.push(row);
        return;
    }

    let mut found = false;
    for entry in std::fs::read_dir(&dir).ok().into_iter().flatten() {
        let entry = match entry {
            Ok(e) => e,
            _ => continue,
        };
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        found = true;
        let name = entry.file_name().to_string_lossy().to_string();

        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(&format!("Installed in {}", entry.path().display()))
            .activatable(false)
            .build();

        let remove_btn = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove backend")
            .css_classes(["flat", "circular", "destructive-action"])
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&remove_btn);

        group.add(&row);
        rows.push(row);
    }

    if !found {
        let row = adw::ActionRow::builder()
            .title("No backends installed")
            .subtitle("Download from the Available section below")
            .activatable(false)
            .build();
        group.add(&row);
        rows.push(row);
    }
}

// ── Available backends — each gets its own ManagedDownloadRow ───────────

fn build_available_graphics_rows(
    group: &adw::PreferencesGroup,
) -> Vec<AsyncController<managed_download_row::ManagedDownloadRow>> {
    let mut ctrls: Vec<AsyncController<managed_download_row::ManagedDownloadRow>> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // ── DXMT ──
        let dxmt_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: "DXMT".into(),
                check_status: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    let found = std::fs::read_dir(&dir).ok().into_iter().flatten().any(|e| {
                        e.as_ref()
                            .ok()
                            .and_then(|e| e.file_type().ok())
                            .map(|t| t.is_dir())
                            .unwrap_or(false)
                            && e.as_ref()
                                .ok()
                                .map(|e| e.file_name().to_string_lossy().starts_with("dxmt-"))
                                .unwrap_or(false)
                    });
                    managed_download_row::DownloadRowStatus {
                        installed: found,
                        managed: found,
                        status_text: if found {
                            "✓ Installed".into()
                        } else {
                            "DirectX → Metal translation layer (recommended)".into()
                        },
                    }
                }),
                check_update: None,
                start_download: Box::new(|_data_dir, progress, cancel| {
                    Box::pin(async move {
                        let (version, url) =
                            runtime::graphics::fetch_dxmt_release()
                                .await
                                .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err("Cancelled".into());
                        }
                        let simple_prog: runtime::download::ProgressFn =
                            Box::new(move |d, t| {
                                progress(
                                    d,
                                    t,
                                    runtime::download::InstallPhase::Download,
                                );
                            });
                        runtime::graphics::download_dxmt(&version, &url, &simple_prog)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    })
                }),
                perform_remove: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    if !dir.is_dir() {
                        return Ok(());
                    }
                    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                        let entry = entry.map_err(|e| e.to_string())?;
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("dxmt-")
                            && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        {
                            std::fs::remove_dir_all(&entry.path())
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                }),
                data_dir: Default::default(),
            })
            .detach();
        group.add(dxmt_ctrl.widget());
        ctrls.push(dxmt_ctrl);

        // ── D3DMetal (via GPTK) ──
        let d3d_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: "D3DMetal (via GPTK)".into(),
                check_status: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    let found = std::fs::read_dir(&dir).ok().into_iter().flatten().any(|e| {
                        e.as_ref()
                            .ok()
                            .and_then(|e| e.file_type().ok())
                            .map(|t| t.is_dir())
                            .unwrap_or(false)
                            && e.as_ref()
                                .ok()
                                .map(|e| {
                                    e.file_name()
                                        .to_string_lossy()
                                        .starts_with("d3dmetal-")
                                })
                                .unwrap_or(false)
                    });
                    managed_download_row::DownloadRowStatus {
                        installed: found,
                        managed: found,
                        status_text: if found {
                            "✓ Installed".into()
                        } else {
                            "Apple's Game Porting Toolkit".into()
                        },
                    }
                }),
                check_update: None,
                start_download: Box::new(|_data_dir, progress, cancel| {
                    Box::pin(async move {
                        // D3DMetal download not yet implemented – placeholder.
                        let _ = cancel;
                        let _ = progress;
                        Err("D3DMetal download not yet supported".into())
                    })
                }),
                perform_remove: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    if !dir.is_dir() {
                        return Ok(());
                    }
                    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                        let entry = entry.map_err(|e| e.to_string())?;
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("d3dmetal-")
                            && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        {
                            std::fs::remove_dir_all(&entry.path())
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                }),
                data_dir: Default::default(),
            })
            .detach();
        group.add(d3d_ctrl.widget());
        ctrls.push(d3d_ctrl);
    }

    #[cfg(target_os = "linux")]
    {
        // ── DXVK + VKD3D ──
        let dxvk_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: "DXVK + VKD3D".into(),
                check_status: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    let found = std::fs::read_dir(&dir).ok().into_iter().flatten().any(|e| {
                        e.as_ref()
                            .ok()
                            .and_then(|e| e.file_type().ok())
                            .map(|t| t.is_dir())
                            .unwrap_or(false)
                            && e.as_ref()
                                .ok()
                                .map(|e| {
                                    let n = e.file_name().to_string_lossy();
                                    n.starts_with("dxvk-") || n.starts_with("vkd3d-")
                                })
                                .unwrap_or(false)
                    });
                    managed_download_row::DownloadRowStatus {
                        installed: found,
                        managed: found,
                        status_text: if found {
                            "✓ Installed".into()
                        } else {
                            "DirectX → Vulkan translation layers".into()
                        },
                    }
                }),
                check_update: None,
                start_download: Box::new(|_data_dir, progress, cancel| {
                    Box::pin(async move {
                        let simple_prog: runtime::download::ProgressFn =
                            Box::new(move |d, t| {
                                progress(
                                    d,
                                    t,
                                    runtime::download::InstallPhase::Download,
                                );
                            });

                        // Download DXVK
                        let (v_version, v_url) =
                            runtime::graphics::fetch_dxvk_release()
                                .await
                                .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err("Cancelled".into());
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
                            return Err("Cancelled".into());
                        }

                        // Download VKD3D-Proton
                        let (v3_version, v3_url) =
                            runtime::graphics::fetch_vkd3d_release()
                                .await
                                .map_err(|e| e.to_string())?;
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err("Cancelled".into());
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
                perform_remove: Box::new(|| {
                    let dir = runtime::graphics::graphics_dir();
                    if !dir.is_dir() {
                        return Ok(());
                    }
                    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                        let entry = entry.map_err(|e| e.to_string())?;
                        let name = entry.file_name().to_string_lossy().to_string();
                        if (name.starts_with("dxvk-") || name.starts_with("vkd3d-"))
                            && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        {
                            std::fs::remove_dir_all(&entry.path())
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                }),
                data_dir: Default::default(),
            })
            .detach();
        group.add(dxvk_ctrl.widget());
        ctrls.push(dxvk_ctrl);
    }

    ctrls
}
