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
    #[tracker::do_not_track]
    d3dmetal_dialog: Option<Controller<D3DMetalImportDialog>>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GraphicsSettingsMsg {
    RefreshInstalled,
    ShowD3DMetalImportDialog(std::sync::mpsc::Sender<Option<String>>),
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
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Placeholder groups — replaced with real ones from view! after view_output!()
        let placeholder_group = adw::PreferencesGroup::new();
        let mut model = GraphicsSettings {
            installed_group: placeholder_group,
            rows: Vec::new(),
            available_ctrls: Vec::new(),
            d3dmetal_dialog: None,
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
        root: &Self::Root,
    ) {
        match msg {
            GraphicsSettingsMsg::RefreshInstalled => {
                refresh_graphics_list(&self.installed_group, &mut self.rows);
                let _ = sender.output(GraphicsSettingsOutput::Changed);
            }
            GraphicsSettingsMsg::ShowD3DMetalImportDialog(tx) => {
                // Lazily initialize the dialog on first use
                if self.d3dmetal_dialog.is_none() {
                    if let Some(parent) = root
                        .root()
                        .and_then(|s| s.downcast::<gtk::Window>().ok())
                    {
                        let skip = runtime::graphics::graphics_dir()
                            .join(".d3dmetal_skip_dialog");
                        let ctrl = D3DMetalImportDialog::builder()
                            .launch((parent, skip))
                            .detach();
                        self.d3dmetal_dialog = Some(ctrl);
                    }
                }
                if let Some(ref ctrl) = self.d3dmetal_dialog {
                    let _ = ctrl.sender().send(D3DMetalImportMsg::Show(tx));
                }
            }
        }
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
    sender: &AsyncComponentSender<GraphicsSettings>,
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
            .forward(sender.input_sender(), |_out| {
                GraphicsSettingsMsg::RefreshInstalled
            });
        group.add(dxmt_ctrl.widget());
        ctrls.push(dxmt_ctrl);

        // ── D3DMetal (via GPTK) — requires manual download from Apple Developer ──
        let d3d_gs_sender = sender.input_sender();
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
                start_download: Box::new({
                    let gs_sender = d3d_gs_sender.clone();
                    move |_data_dir, _progress, cancel| {
                        let gs = gs_sender.clone();
                        Box::pin(async move {
                            use std::sync::atomic::Ordering;
                            use std::sync::mpsc::{channel, TryRecvError};
                            use std::time::Duration;

                            let (tx, rx) = channel::<Option<String>>();

                            // Tell GraphicsSettings to show the pre-initialized dialog
                            if gs
                                .send(GraphicsSettingsMsg::ShowD3DMetalImportDialog(tx))
                                .is_err()
                            {
                                return Err("Failed to open import dialog".into());
                            }

                            // Poll for user action (folder path or cancel)
                            let selected = loop {
                                match rx.try_recv() {
                                    Ok(Some(path)) => break path,
                                    Ok(None) => return Err("Import cancelled".into()),
                                    Err(TryRecvError::Empty) => {
                                        if cancel.load(Ordering::Relaxed) {
                                            return Err("Cancelled".into());
                                        }
                                        gtk::glib::timeout_future(Duration::from_millis(100)).await;
                                    }
                                    Err(TryRecvError::Disconnected) => {
                                        return Err("Import cancelled".into());
                                    }
                                }
                            };

                            // ── Import (blocking — run on background thread) ──
                            let path = std::path::PathBuf::from(&selected);
                            let (tx_import, rx_import) = std::sync::mpsc::channel();
                            std::thread::spawn(move || {
                                let result = if path.extension().map(|e| e == "dmg").unwrap_or(false)
                                {
                                    runtime::graphics::import_d3dmetal_from_dmg(&path)
                                        .map(|_| ())
                                        .map_err(|e| e.to_string())
                                } else {
                                    let r = (|| -> Result<(), String> {
                                        let gfx_dir = runtime::graphics::graphics_dir();
                                        let ts = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let dest =
                                            gfx_dir.join(format!("d3dmetal-imported-{}", ts));
                                        let lib = [path.join("lib"), path.join("redist").join("lib")]
                                            .iter()
                                            .find(|p| p.is_dir())
                                            .cloned()
                                            .ok_or_else(|| {
                                                "Could not find GPTK lib directory in \
                                                 the selected path."
                                                    .to_string()
                                            })?;
                                        std::fs::create_dir_all(&dest)
                                            .map_err(|e| format!("Failed to create dir: {}", e))?;
                                        let status = std::process::Command::new("cp")
                                            .arg("-R")
                                            .arg(&lib)
                                            .arg(&dest)
                                            .status()
                                            .map_err(|e| format!("cp failed: {}", e))?;
                                        if !status.success() {
                                            return Err("Failed to copy GPTK files.".into());
                                        }
                                        Ok(())
                                    })();
                                    r
                                };
                                let _ = tx_import.send(result);
                            });

                            // Poll for completion
                            let result = loop {
                                match rx_import.try_recv() {
                                    Ok(r) => break r,
                                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                                        if cancel.load(Ordering::Relaxed) {
                                            return Err("Cancelled".into());
                                        }
                                        gtk::glib::timeout_future(Duration::from_millis(200)).await;
                                    }
                                    Err(_) => {
                                        break Err("Import thread crashed unexpectedly.".into());
                                    }
                                }
                            };
                            result?;
                            Ok(())
                        })
                    }
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
            .forward(sender.input_sender(), |_out| {
                GraphicsSettingsMsg::RefreshInstalled
            });
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
            .forward(sender.input_sender(), |_out| {
                GraphicsSettingsMsg::RefreshInstalled
            });
        group.add(dxvk_ctrl.widget());
        ctrls.push(dxvk_ctrl);
    }

    ctrls
}

// ═══ D3DMetal Import Dialog — Apple Developer → pick folder → import ═════

struct D3DMetalImportDialog {
    skip_path: std::path::PathBuf,
    result_tx: Option<std::sync::mpsc::Sender<Option<String>>>,
    dialog: gtk::Window,
    info_label: gtk::Label,
    checkbox: gtk::CheckButton,
}

#[derive(Debug)]
enum D3DMetalImportMsg {
    Show(std::sync::mpsc::Sender<Option<String>>),
    SelectDmg,
    Cancel,
}

#[relm4::component]
impl SimpleComponent for D3DMetalImportDialog {
    type Init = (gtk::Window, std::path::PathBuf);
    type Input = D3DMetalImportMsg;
    type Output = ();

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some("Import D3DMetal (via GPTK)"),
            set_modal: true,
            set_default_width: 440,
            set_transient_for: Some(&parent),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 8,
                set_margin_all: 16,

                #[name = "info_label"]
                gtk::Label {
                    set_use_markup: true,
                    set_label: "D3DMetal (via GPTK) — compatible with GPTK 3.0.\
                         \n\n\
                         Download requires an Apple Developer account:\n  \
                         <a href=\"https://developer.apple.com/games/game-porting-toolkit/\">\
                         developer.apple.com/games/game-porting-toolkit/</a>\
                         \n\n\
                         Then, click \"Select DMG\" to choose the DMG downloaded.\
                         \n\n\
                         By proceeding, you agree to Apple's Software License \
                         Agreement for Game Porting Toolkit.",
                    set_wrap: true,
                    set_halign: gtk::Align::Start,
                    set_selectable: false,
                    set_visible: false,
                },

                #[name = "checkbox"]
                gtk::CheckButton {
                    set_label: Some("Don't show this message again"),
                    set_margin_bottom: 8,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::End,
                    set_spacing: 8,
                    set_margin_top: 8,

                    gtk::Button {
                        set_label: "Cancel",
                        connect_clicked => D3DMetalImportMsg::Cancel,
                    },
                    gtk::Button {
                        set_label: "Select DMG",
                        add_css_class: "suggested-action",
                        connect_clicked => D3DMetalImportMsg::SelectDmg,
                    },
                },
            },
        }
    }

    fn init(
        (parent, skip_path): Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        // Intercept window close (red X) — treat as cancel
        let s = sender.clone();
        widgets.dialog.connect_close_request(move |_| {
            let _ = s.input(D3DMetalImportMsg::Cancel);
            gtk::glib::Propagation::Stop
        });

        let model = D3DMetalImportDialog {
            skip_path,
            result_tx: None,
            dialog: widgets.dialog.clone(),
            info_label: widgets.info_label.clone(),
            checkbox: widgets.checkbox.clone(),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            D3DMetalImportMsg::Show(tx) => {
                self.result_tx = Some(tx);
                let show_info = !self.skip_path.exists();
                if show_info {
                    self.info_label.set_visible(true);
                    self.checkbox.set_visible(true);
                    self.dialog.present();
                } else {
                    // "Don't show again" — skip the dialog entirely,
                    // go straight to the file picker.
                    self.start_pick_file();
                }
            }
            D3DMetalImportMsg::SelectDmg => {
                self.start_pick_file();
            }
            D3DMetalImportMsg::Cancel => {
                if let Some(tx) = self.result_tx.take() {
                    let _ = tx.send(None);
                }
                self.dialog.set_visible(false);
            }
        }
    }
}

impl D3DMetalImportDialog {
    fn start_pick_file(&mut self) {
        if let Some(tx) = self.result_tx.take() {
            let skip = self.skip_path.clone();
            let dont_ask = self.checkbox.is_active();
            let dlg = self.dialog.clone();
            crate::utils::pick_file(&self.dialog, "Select GPTK DMG", &["dmg"], move |path| {
                if let Some(p) = path {
                    if dont_ask {
                        let _ = std::fs::write(&skip, "1");
                    }
                    let _ = tx.send(Some(p));
                }
                // if cancelled, tx drops (receiver gets Disconnected)
                dlg.set_visible(false);
            });
        }
    }
}
