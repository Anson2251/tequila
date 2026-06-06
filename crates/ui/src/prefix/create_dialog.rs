use crate::AppMsg;
use adw::prelude::*;
use gtk::glib;
use prefix::base::GraphicsBackend;
use prefix::runtime;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};
use service::AppService;

pub struct CreatePrefixDialog {
    name_entry: gtk::Entry,
    arch_combo: gtk::DropDown,
    runtime_combo: gtk::DropDown,
    graphics_combo: gtk::DropDown,
    graphics_backends: Vec<Option<GraphicsBackend>>, // None = no backend
    create_btn: gtk::Button,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    dialog: gtk::Window,
    parent: gtk::ApplicationWindow,
}

#[derive(Debug)]
pub enum CreatePrefixMsg {
    Create,
}

impl CreatePrefixDialog {
    fn build_runtime_combo(prefix_manager: &prefix::Manager) -> gtk::DropDown {
        let rm = &*prefix_manager.read_runtime();
        let default_id = &rm.default_id;
        let items: Vec<String> = rm
            .runtimes
            .iter()
            .map(|rt| format!("{} ({})", rt.name, rt.wine_version))
            .collect();
        let str_refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
        let combo = gtk::DropDown::from_strings(&str_refs);
        combo.set_hexpand(true);
        if let Some(idx) = rm.runtimes.iter().position(|rt| &rt.id == default_id) {
            combo.set_selected(idx as u32);
        }
        combo
    }

    fn build_graphics_combo() -> (gtk::DropDown, Vec<Option<GraphicsBackend>>) {
        let backends = runtime::graphics::installed_backends();
        let mut items = vec!["WineD3D (built-in)".to_string()];
        let mut mapping: Vec<Option<GraphicsBackend>> = vec![None];

        for b in backends {
            items.push(format!("{} ({})", b.display_name(), b.version_string()));
            mapping.push(Some(b));
        }

        let str_refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
        let combo = gtk::DropDown::from_strings(&str_refs);
        combo.set_hexpand(true);
        combo.set_selected(0); // default to None

        (combo, mapping)
    }
}

#[relm4::component(pub)]
impl SimpleComponent for CreatePrefixDialog {
    type Init = (gtk::ApplicationWindow,);
    type Input = CreatePrefixMsg;
    type Output = AppMsg;

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some(&crate::t!("prefix.create.title")),
            set_modal: true,
            set_transient_for: Some(&parent),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_top: 10,
                set_margin_bottom: 10,
                set_margin_start: 10,
                set_margin_end: 10,
                set_spacing: 10,

                gtk::Box {
                    set_hexpand: true,
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Label {
                        set_label: &crate::t!("prefix.create.name_label"),
                        set_halign: gtk::Align::Start,
                    },
                    #[name = "name_entry"]
                    gtk::Entry {
                        set_placeholder_text: Some(&crate::t!("prefix.create.name_placeholder")),
                        set_hexpand: true,
                        set_width_chars: 32,
                    },
                },

                gtk::Box {
                    set_spacing: 10,
                    set_margin_top: 10,
                    set_hexpand: true,
                    set_orientation: gtk::Orientation::Horizontal,

                    gtk::Box {
                        set_spacing: 10,
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::Label {
                            set_label: &crate::t!("prefix.create.arch_label"),
                            set_halign: gtk::Align::Start,
                        },
                        #[name = "arch_combo"]
                        gtk::DropDown {
                            set_model: Some(&gtk::StringList::new(&["win32", "win64"])),
                            set_selected: 1u32,
                            set_hexpand: true,
                        },
                    },
                    gtk::Box {
                        set_hexpand: true,
                        set_spacing: 10,
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::Label {
                            set_label: &crate::t!("prefix.create.runtime_label"),
                            set_halign: gtk::Align::Start,
                        },
                        #[local_ref]
                        runtime_combo -> gtk::DropDown {
                            set_hexpand: true,
                        },
                    }
                },

                gtk::Box {
                    set_hexpand: true,
                    set_spacing: 10,
                    set_margin_top: 10,
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Label {
                        set_label: &crate::t!("prefix.create.graphics_label"),
                        set_halign: gtk::Align::Start,
                    },
                    #[local_ref]
                    graphics_combo -> gtk::DropDown {
                        set_hexpand: true,
                    },
                },

                #[name = "progress_label"]
                gtk::Label {
                    set_label: &crate::t!("prefix.create.progress"),
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
        (parent,): Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let svc = AppService::global();
        let prefix_manager = svc.prefix_manager();
        let runtime_combo = Self::build_runtime_combo(&prefix_manager);
        let (graphics_combo, graphics_backends) = Self::build_graphics_combo();

        let widgets = view_output!();

        // Build header bar manually (view! macro can't pack buttons into HeaderBar)
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        let create_btn = gtk::Button::builder()
            .label(&crate::t!("prefix.create.create_btn"))
            .icon_name("object-select-symbolic")
            .css_classes(["suggested-action"])
            .build();
        let s = sender.clone();
        create_btn.connect_clicked(move |_| {
            let _ = s.input(CreatePrefixMsg::Create);
        });

        #[cfg(target_os = "macos")]
        header_bar.pack_end(&create_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&create_btn);

        widgets.dialog.set_titlebar(Some(&header_bar));
        widgets.dialog.present();

        let model = CreatePrefixDialog {
            name_entry: widgets.name_entry.clone(),
            arch_combo: widgets.arch_combo.clone(),
            runtime_combo: runtime_combo.clone(),
            graphics_combo: graphics_combo.clone(),
            graphics_backends,
            create_btn,
            progress_bar: widgets.progress_bar.clone(),
            progress_label: widgets.progress_label.clone(),
            dialog: widgets.dialog.clone(),
            parent,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            CreatePrefixMsg::Create => {
                let name = self.name_entry.text().to_string();
                if name.is_empty() {
                    log::warn!("[create] prefix name cannot be empty");
                    return;
                }

                let architecture = if self.arch_combo.selected() == 0 {
                    "win32"
                } else {
                    "win64"
                };

                let runtime_id = {
                    let i = self.runtime_combo.selected() as usize;
                    let svc = AppService::global();
                    let pm = svc.prefix_manager();
                    let rm = &*pm.read_runtime();
                    rm.runtimes
                        .get(i)
                        .map(|r| r.id.clone())
                        .unwrap_or_else(|| rm.default_id.clone())
                };

                let selected_backend = {
                    let i = self.graphics_combo.selected() as usize;
                    self.graphics_backends.get(i).cloned().unwrap_or(None)
                };

                // Show progress, disable inputs
                self.name_entry.set_sensitive(false);
                self.arch_combo.set_sensitive(false);
                self.runtime_combo.set_sensitive(false);
                self.graphics_combo.set_sensitive(false);
                self.progress_label.set_visible(true);
                self.progress_bar.set_visible(true);
                self.progress_bar.set_fraction(0.0);
                self.create_btn.set_sensitive(false);

                let pb = self.progress_bar.clone();
                let pulse_id =
                    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                        pb.pulse();
                        glib::ControlFlow::Continue
                    });

                let prefix_name = name.clone();
                let pm = AppService::global().prefix_manager().clone();
                let mw = self.parent.clone();
                let dlg = self.dialog.clone();
                let sc = sender.clone();
                let ctx = glib::MainContext::default();
                ctx.spawn_local(async move {
                    // Step 1: Create prefix (blocking)
                    let n = prefix_name.clone();
                    let a = architecture;
                    let rid = runtime_id.clone();
                    let pm_create = pm.clone();
                    let create_result = tokio::task::spawn_blocking(move || {
                        pm_create.create_prefix_with_runtime(&n, &a, &rid)
                    })
                    .await;

                    let prefix_path = match create_result {
                        Ok(Ok(path)) => path,
                        Ok(Err(e)) => {
                            pulse_id.remove();
                            dlg.close();
                            log::error!(
                                "[create] failed to create prefix '{}': {}",
                                prefix_name,
                                e
                            );
                            let alert = adw::AlertDialog::new(
                                Some(&crate::t!("dialogs.error")),
                                Some(&crate::tf!("prefix.create.error_msg", "name" => &prefix_name, "error" => &e.to_string())),
                            );
                            alert.add_response("ok", &crate::t!("dialogs.ok"));
                            alert.set_default_response(Some("ok"));
                            alert.set_close_response("ok");
                            alert.choose(Some(&mw), None::<&gtk::gio::Cancellable>, |_| {});
                            return;
                        }
                        Err(e) => {
                            pulse_id.remove();
                            dlg.close();
                            let msg = if e.is_panic() {
                                "panic in create_prefix".to_string()
                            } else {
                                format!("{}", e)
                            };
                            log::error!(
                                "[create] failed to create prefix '{}': {}",
                                prefix_name,
                                msg
                            );
                            return;
                        }
                    };

                    // Step 2: Activate graphics backend (symlink DLLs + registry + config)
                    if let Some(backend) = selected_backend {
                        if let Err(e) = pm.activate_graphics_backend(&backend, &prefix_path).await {
                            log::error!(
                                "[create] failed to activate {}: {}",
                                backend.display_name(),
                                e
                            );
                            let alert = adw::AlertDialog::new(
                                Some(&crate::t!("dialogs.warning")),
                                Some(&crate::tf!("prefix.create.warning_msg", "backend" => &backend.display_name(), "error" => &e.to_string())),
                            );
                            alert.add_response("ok", &crate::t!("dialogs.ok"));
                            alert.set_default_response(Some("ok"));
                            alert.set_close_response("ok");
                            alert.choose(Some(&mw), None::<&gtk::gio::Cancellable>, |_| {});
                        }
                    }

                    pulse_id.remove();
                    dlg.close();
                    log::info!(
                        "[create] created prefix: {} at {}",
                        prefix_name,
                        prefix_path.display()
                    );
                    sc.output(AppMsg::RefreshPrefixes).unwrap_or(());
                });
            }
        }
    }
}
