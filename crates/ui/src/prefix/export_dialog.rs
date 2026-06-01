use adw::prelude::*;
use prefix::Manager as PrefixManager;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use std::path::PathBuf;

use crate::AppMsg;

pub struct ExportDialogModel {
    prefix_manager: PrefixManager,
    prefix_path: PathBuf,
    prefix_name: String,
    dest_entry: gtk::Entry,
    browse_btn: gtk::Button,
    user_data_check: gtk::CheckButton,
    level_scale: gtk::Scale,
    export_btn: gtk::Button,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    dialog: gtk::Window,
    parent: gtk::ApplicationWindow,
    pulse_id: Option<gtk::glib::SourceId>,
    exporting: bool,
}

#[derive(Debug)]
pub enum ExportDialogMsg {
    Browse,
    Export,
    ExportProgress(u64, u64),
    ExportComplete(std::result::Result<PathBuf, String>),
}

/// Progress update from the background thread.
/// `(bytes_done, bytes_total)` for progress; `(0, 0, Some(result))` for completion.
type ExportProgress = (u64, u64, Option<std::result::Result<PathBuf, String>>);

#[relm4::component(pub)]
impl SimpleComponent for ExportDialogModel {
    type Init = (PrefixManager, PathBuf, String, gtk::ApplicationWindow);
    type Input = ExportDialogMsg;
    type Output = AppMsg;

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some("Export Prefix"),
            set_modal: true,
            set_transient_for: Some(&parent),
            set_default_width: 480,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_top: 10,
                set_margin_bottom: 10,
                set_margin_start: 10,
                set_margin_end: 10,
                set_spacing: 16,

                // Destination
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 4,

                    gtk::Label {
                        set_label: "Save to:",
                        set_halign: gtk::Align::Start,
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,

                        #[name = "dest_entry"]
                        gtk::Entry {
                            set_hexpand: true,
                            set_placeholder_text: Some("Select destination…"),
                        },
                        #[name = "browse_btn"]
                        gtk::Button {
                            set_label: "Browse…",
                            connect_clicked[sender] => move |_| {
                                sender.input(ExportDialogMsg::Browse);
                            },
                        },
                    },
                },

                // User data
                #[name = "user_data_check"]
                gtk::CheckButton {
                    set_label: Some("Include user data (AppData, Documents, etc.)"),
                    set_active: true,
                    set_margin_top: 4,
                },

                // Compression level
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 4,
                    set_margin_top: 4,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,

                        gtk::Label {
                            set_label: "Compression level:",
                        },
                        #[name = "level_label"]
                        gtk::Label {
                            set_label: "3",
                            set_halign: gtk::Align::End,
                            set_hexpand: true,
                            set_css_classes: &["numeric", "monospace"],
                        },
                    },
                    #[name = "level_scale"]
                    gtk::Scale {
                        set_range: (1.0, 22.0),
                        set_value: 3.0,
                        set_draw_value: false,
                        set_hexpand: true,
                        set_size_request: (-1, 28),
                    },
                    gtk::Label {
                        set_label: "1 (fastest)  ───────────  22 (best compression)",
                        set_halign: gtk::Align::Center,
                        set_css_classes: &["dim-label", "caption"],
                    },
                },

                // Progress
                #[name = "progress_label"]
                gtk::Label {
                    set_label: "Exporting prefix...",
                    set_visible: false,
                },
                #[name = "progress_bar"]
                gtk::ProgressBar {
                    set_visible: false,
                },
            },
        }
    }

    fn init(
        (prefix_manager, prefix_path, prefix_name, parent): Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        // Wire slider to live-update the label
        widgets.level_scale.connect_value_changed({
            let ll = widgets.level_label.clone();
            move |scale| {
                let val = scale.value() as i32;
                ll.set_label(&val.to_string());
            }
        });

        // Header bar
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        let export_btn = gtk::Button::builder()
            .label("Export")
            .icon_name("document-save-symbolic")
            .css_classes(["suggested-action"])
            .build();
        let s = sender.clone();
        export_btn.connect_clicked(move |_| {
            let _ = s.input(ExportDialogMsg::Export);
        });

        #[cfg(target_os = "macos")]
        header_bar.pack_end(&export_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&export_btn);

        widgets.dialog.set_titlebar(Some(&header_bar));
        widgets.dialog.present();

        let model = ExportDialogModel {
            prefix_manager,
            prefix_path,
            prefix_name,
            dest_entry: widgets.dest_entry.clone(),
            browse_btn: widgets.browse_btn.clone(),
            user_data_check: widgets.user_data_check.clone(),
            level_scale: widgets.level_scale.clone(),
            export_btn,
            progress_bar: widgets.progress_bar.clone(),
            progress_label: widgets.progress_label.clone(),
            dialog: widgets.dialog.clone(),
            parent,
            pulse_id: None,
            exporting: false,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ExportDialogMsg::Browse => {
                let parent = self.parent.clone();
                let entry = self.dest_entry.clone();
                let suggested = format!("{}.zst.{}", self.prefix_name, prefix::TQL_EXTENSION);
                let exts = [&format!("zst.{}", prefix::TQL_EXTENSION)[..]];
                crate::dialogs::save_file(
                    &parent.upcast::<gtk::Window>(),
                    "Choose Destination",
                    &suggested,
                    &exts,
                    move |path| {
                        if let Some(path) = path {
                            entry.set_text(&path);
                        }
                    },
                );
            }
            ExportDialogMsg::Export => {
                let dest = self.dest_entry.text().to_string();
                if dest.is_empty() {
                    return;
                }

                let include_user_data = self.user_data_check.is_active();
                let compression_level = self.level_scale.value() as i32;
                let pp = self.prefix_path.clone();
                let pm = self.prefix_manager.clone();

                self.exporting = true;

                // Disable all interactive elements
                self.dest_entry.set_sensitive(false);
                self.browse_btn.set_sensitive(false);
                self.user_data_check.set_sensitive(false);
                self.level_scale.set_sensitive(false);
                self.export_btn.set_sensitive(false);
                self.dialog.set_deletable(false);
                self.progress_label.set_visible(true);
                self.progress_bar.set_visible(true);
                self.progress_bar.set_fraction(0.0);

                // Channel carries progress updates from the background thread
                let (progress_tx, progress_rx) = std::sync::mpsc::channel::<ExportProgress>();

                // Timeout only captures sender + channel — no widget clones needed
                let s = sender.clone();
                let timeout_id =
                    gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
                        while let Ok((done, total, completion)) = progress_rx.try_recv() {
                            if total > 0 {
                                let _ = s.input(ExportDialogMsg::ExportProgress(done, total));
                            }
                            if let Some(result) = completion {
                                let _ = s.input(ExportDialogMsg::ExportComplete(result));
                                return gtk::glib::ControlFlow::Break;
                            }
                        }
                        gtk::glib::ControlFlow::Continue
                    });

                let progress_tx_thread = progress_tx.clone();
                std::thread::spawn(move || {
                    let result = pm.export_prefix(
                        &pp,
                        &PathBuf::from(&dest),
                        include_user_data,
                        compression_level,
                        move |done, total| {
                            let _ = progress_tx_thread.send((done, total, None));
                        },
                    );

                    let completion = result.map(|p| p).map_err(|e| e.to_string());
                    let _ = progress_tx.send((1, 1, Some(completion)));
                });

                self.pulse_id = Some(timeout_id);
            }
            ExportDialogMsg::ExportProgress(done, total) => {
                if total > 0 {
                    self.progress_bar.set_fraction(done as f64 / total as f64);
                }
            }
            ExportDialogMsg::ExportComplete(result) => {
                self.exporting = false;
                // Timeout already stopped itself via ControlFlow::Break
                self.pulse_id.take();

                match result {
                    Ok(path) => {
                        log::info!("[export] Exported to {}", path.display());
                        self.dialog.set_deletable(true);
                        self.dialog.close();
                    }
                    Err(e) => {
                        log::error!("[export] Failed: {}", e);
                        // Re-enable UI for retry
                        self.dest_entry.set_sensitive(true);
                        self.browse_btn.set_sensitive(true);
                        self.user_data_check.set_sensitive(true);
                        self.level_scale.set_sensitive(true);
                        self.export_btn.set_sensitive(true);
                        self.dialog.set_deletable(true);
                        self.progress_label.set_visible(false);
                        self.progress_bar.set_visible(false);

                        let alert = adw::AlertDialog::new(Some("Export Failed"), Some(&e));
                        alert.add_response("ok", "OK");
                        alert.set_default_response(Some("ok"));
                        alert.set_close_response("ok");
                        alert.choose(
                            Some(&self.parent.clone().upcast::<gtk::Window>()),
                            None::<&gtk::gio::Cancellable>,
                            |_| {},
                        );
                    }
                }

                let _ = sender.output(crate::AppMsg::RefreshPrefixes);
            }
        }
    }
}
