use adw::prelude::*;
use prefix::Manager as PrefixManager;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use std::path::PathBuf;

use crate::AppMsg;

pub struct ImportDialogModel {
    prefix_manager: PrefixManager,
    archive_path: PathBuf,
    prefix_name: String,
    runtime_combo: gtk::DropDown,
    runtime_ids: Vec<String>,
    import_btn: gtk::Button,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    dialog: gtk::Window,
    parent: gtk::ApplicationWindow,
    pulse_id: Option<gtk::glib::SourceId>,
}

#[derive(Debug)]
pub enum ImportDialogMsg {
    Import,
    ImportComplete(std::result::Result<PathBuf, String>),
}

#[relm4::component(pub)]
impl SimpleComponent for ImportDialogModel {
    type Init = (PrefixManager, PathBuf, String, gtk::ApplicationWindow);
    type Input = ImportDialogMsg;
    type Output = AppMsg;

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some("Import Prefix"),
            set_modal: true,
            set_transient_for: Some(&parent),
            set_default_width: 420,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_top: 10,
                set_margin_bottom: 10,
                set_margin_start: 10,
                set_margin_end: 10,
                set_spacing: 12,

                gtk::Label {
                    set_label: "Import prefix:",
                    set_halign: gtk::Align::Start,
                    set_css_classes: &["heading"],
                },
                gtk::Label {
                    set_label: prefix_name.as_str(),
                    set_halign: gtk::Align::Start,
                    set_css_classes: &["dim-label"],
                },
                gtk::Label {
                    set_label: "Wine Runtime:",
                    set_halign: gtk::Align::Start,
                    set_margin_top: 8,
                },
                #[local_ref]
                runtime_combo -> gtk::DropDown {
                    set_hexpand: true,
                },

                #[name = "progress_label"]
                gtk::Label {
                    set_label: "Importing prefix...",
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
        (prefix_manager, archive_path, prefix_name, parent): Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Build runtime dropdown
        let rm = prefix_manager.runtime_manager();
        let items: Vec<String> = rm
            .runtimes
            .iter()
            .map(|rt| format!("{} ({})", rt.name, rt.wine_version))
            .collect();
        let ids: Vec<String> = rm.runtimes.iter().map(|rt| rt.id.clone()).collect();
        let str_items: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
        let runtime_combo = gtk::DropDown::from_strings(&str_items);
        if !ids.is_empty() {
            let default_idx = ids.iter().position(|id| *id == rm.default_id).unwrap_or(0);
            runtime_combo.set_selected(default_idx as u32);
        }

        let widgets = view_output!();

        // Header bar
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        let import_btn = gtk::Button::builder()
            .label("Import")
            .icon_name("document-open-symbolic")
            .css_classes(["suggested-action"])
            .build();
        let s = sender.clone();
        import_btn.connect_clicked(move |_| {
            let _ = s.input(ImportDialogMsg::Import);
        });

        #[cfg(target_os = "macos")]
        header_bar.pack_end(&import_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&import_btn);

        widgets.dialog.set_titlebar(Some(&header_bar));
        widgets.dialog.present();

        let model = ImportDialogModel {
            prefix_manager,
            archive_path,
            prefix_name,
            runtime_combo,
            runtime_ids: ids,
            import_btn,
            progress_bar: widgets.progress_bar.clone(),
            progress_label: widgets.progress_label.clone(),
            dialog: widgets.dialog.clone(),
            parent,
            pulse_id: None,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImportDialogMsg::Import => {
                let idx = self.runtime_combo.selected() as usize;
                let runtime_id = self.runtime_ids.get(idx).cloned().unwrap_or_default();

                let pp = self.archive_path.clone();
                let pm = self.prefix_manager.clone();

                // Disable + show progress
                self.import_btn.set_sensitive(false);
                self.runtime_combo.set_sensitive(false);
                self.dialog.set_deletable(false);
                self.progress_label.set_label("Extracting prefix...");
                self.progress_label.set_visible(true);
                self.progress_bar.set_visible(true);
                self.progress_bar.set_fraction(0.0);

                let pb = self.progress_bar.clone();
                let pulse_id = gtk::glib::timeout_add_local(
                    std::time::Duration::from_millis(120),
                    move || {
                        pb.pulse();
                        gtk::glib::ControlFlow::Continue
                    },
                );

                let s = sender.clone();
                std::thread::spawn(move || {
                    let result = pm.import_prefix(&pp, &runtime_id);
                    let msg = match result {
                        Ok(p) => ImportDialogMsg::ImportComplete(Ok(p)),
                        Err(e) => ImportDialogMsg::ImportComplete(Err(e.to_string())),
                    };
                    let _ = s.input(msg);
                });

                self.pulse_id = Some(pulse_id);
            }
            ImportDialogMsg::ImportComplete(result) => {
                if let Some(id) = self.pulse_id.take() {
                    id.remove();
                }

                match result {
                    Ok(path) => {
                        log::info!("[import] Imported to {}", path.display());
                        self.dialog.set_deletable(true);
                        self.dialog.close();
                    }
                    Err(e) => {
                        log::error!("[import] Failed: {}", e);
                        self.import_btn.set_sensitive(true);
                        self.runtime_combo.set_sensitive(true);
                        self.dialog.set_deletable(true);
                        self.progress_label.set_visible(false);
                        self.progress_bar.set_visible(false);

                        let alert = adw::AlertDialog::new(Some("Import Failed"), Some(&e));
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
