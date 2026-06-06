use adw::prelude::*;
use prefix::{
    GraphicsBackend,
    runtime::{RuntimeManager, download, graphics as prefix_graphics},
};
use relm4::prelude::*;
use service::AppService;
use std::path::PathBuf;
use tracker;

mod graphics;
pub mod managed_download_row;
mod runtime;

// ── Model (data only, no widget references) ──────────────────────────────

#[tracker::track]
pub struct SettingsWindow {
    // Page subtitle data
    runtime_subtitle: String,
    graphics_subtitle: String,

    // Managed download row controller for GStreamer
    #[tracker::do_not_track]
    gst_ctrl: AsyncController<managed_download_row::ManagedDownloadRow>,

    // NavigationView kept in model for push/pop actions in update()
    #[tracker::do_not_track]
    nav: adw::NavigationView,

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

    // Forwarded from GStreamer child component
    GStreamerStatusChanged,

    // GitHub API key entry
    GithubKeyChanged(Option<String>),

    // Language
    LanguageChanged(u32),

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
                let names: Vec<&str> = rt
                    .graphics
                    .iter()
                    .map(|g| match g {
                        GraphicsBackend::Dxmt { .. } => "DXMT",
                        GraphicsBackend::D3DMetal { .. } => "D3DMetal",
                        GraphicsBackend::DxvkVkd3d { .. } => "DXVK+VKD3D",
                    })
                    .collect();
                format!(" · {}", names.join(", "))
            };
            let count = rm.runtimes.len();
            format!(
                "{}{} · {} runtime{}",
                rt.wine_version,
                graphics,
                count,
                if count == 1 { "" } else { "s" }
            )
        }
        None => crate::t!("settings.runtime.no_runtimes"),
    }
}

fn graphics_subtitle() -> String {
    let backends = prefix_graphics::installed_backends();
    if backends.is_empty() {
        crate::t!("settings.graphics.no_backends_subtitle")
    } else {
        backends
            .iter()
            .map(|b| b.display_name())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for SettingsWindow {
    type Init = AppService;
    type Input = SettingsMsg;
    type Output = SettingsOutput;
    type CommandOutput = ();
    type Widgets = SettingsWindowWidgets;

    // The view! macro declares the entire UI tree.
    // Widget state is bound to model fields via #[watch] / #[track].
    // prefs_page is created in init() and populated via #[local_ref] below.
    view! {
        #[root]
        gtk::Window {
            set_title: Some(&crate::t!("app.settings")),
            set_default_width: 480,
            set_default_height: 520,
            set_modal: true,
            set_hide_on_close: true,

            set_titlebar: Some(&header_bar),

            connect_close_request[sender] => move |_| {
                sender.input(SettingsMsg::Close);
                gtk::glib::Propagation::Stop
            },

            #[name = "nav"]
            adw::NavigationView {
                push: root_page = &adw::NavigationPage {
                    set_title: &crate::t!("app.settings"),
                    set_can_pop: false,
                    set_child: Some(&prefs_page),
                },
            }
        },

        #[local_ref]
        prefs_page -> adw::PreferencesPage {
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.environment"),
                set_description: Some(&crate::t!("settings.environment_desc")),

                adw::ActionRow {
                    set_title: &crate::t!("settings.wine_runtime"),
                    set_activatable: true,
                    #[watch]
                    set_subtitle: &model.runtime_subtitle,
                    connect_activated => SettingsMsg::ShowRuntime,
                },
                adw::ActionRow {
                    set_title: &crate::t!("settings.graphics_backends"),
                    set_activatable: true,
                    #[watch]
                    set_subtitle: &model.graphics_subtitle,
                    connect_activated => SettingsMsg::ShowGraphics,
                },
            },

            #[name = "gst_group"]
            adw::PreferencesGroup {
                set_title: &crate::t!("settings.gstreamer"),
                set_description: Some(&crate::t!("settings.gstreamer_desc")),
                #[watch]
                set_visible: cfg!(target_os = "macos"),
            },

            adw::PreferencesGroup {
                set_title: &crate::t!("settings.github"),
                set_description: Some(&crate::t!("settings.github_desc")),

                adw::ActionRow {
                    set_title: &crate::t!("settings.api_key"),
                    set_subtitle: &crate::t!("settings.api_key_sub"),
                    set_activatable_widget: Some(&github_key_entry),
                    add_suffix: &github_key_box,
                },

                #[name = "github_key_box"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 3,

                    #[name = "github_key_entry"]
                    gtk::PasswordEntry {
                        set_show_peek_icon: true,
                        set_placeholder_text: Some(&crate::t!("settings.api_key_placeholder")),
                        set_width_request: 180,
                        set_valign: gtk::Align::Center,
                        connect_changed[sender] => move |entry| {
                            let text = entry.text().to_string();
                            let value = if text.is_empty() { None } else { Some(text) };
                            sender.input(SettingsMsg::GithubKeyChanged(value));
                        },
                    },

                    #[name = "github_key_clear_btn"]
                    gtk::Button {
                        set_icon_name: "edit-clear-symbolic",
                        set_tooltip_text: Some(&crate::t!("settings.api_key_clear")),
                        set_valign: gtk::Align::Center,
                        connect_clicked[github_key_entry] => move |_| {
                            github_key_entry.set_text("");
                            let mut settings =
                                prefix::Settings::load()
                                    .unwrap_or_else(|| RuntimeManager::new().into());
                            settings.github_api_key = None;
                            if let Err(e) = settings.save() {
                                log::error!("[settings] failed to save github_api_key: {}", e);
                            }
                        },
                    },
                },
            },

            adw::PreferencesGroup {
                    set_title: &crate::t!("settings.language"),
                    set_description: Some(&crate::t!("settings.language_desc")),

                    adw::ActionRow {
                        set_title: &crate::t!("settings.language"),
                        set_subtitle: &crate::t!("settings.language_switch"),
                        set_activatable_widget: Some(&language_box),

                        add_suffix: &language_box,
                    },

                    #[name = "language_box"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 3,

                        #[name = "language_combo"]
                        gtk::DropDown {
                            set_valign: gtk::Align::Center,
                        },
                    },
                },

                adw::PreferencesGroup {
                    set_title: &crate::t!("settings.directories"),
                        set_description: Some(&crate::t!("settings.directories_desc")),

                adw::ActionRow {
                    set_title: &crate::t!("settings.open_prefixes"),
                    set_subtitle: &crate::t!("settings.open_prefixes_sub"),
                    set_activatable: true,
                    connect_activated[prefixes_dir] => move |_| {
                        let path = prefixes_dir.to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            #[cfg(target_os = "macos")]
                            let _ = std::process::Command::new("open").arg(&path).status();
                            #[cfg(not(target_os = "macos"))]
                            let _ = std::process::Command::new("xdg-open").arg(&path).status();
                        });
                    },
                },
                adw::ActionRow {
                    set_title: &crate::t!("settings.open_data"),
                    set_subtitle: &crate::t!("settings.open_data_sub"),
                    set_activatable: true,
                    connect_activated[data_dir] => move |_| {
                        let path = data_dir.to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            #[cfg(target_os = "macos")]
                            let _ = std::process::Command::new("open").arg(&path).status();
                            #[cfg(not(target_os = "macos"))]
                            let _ = std::process::Command::new("xdg-open").arg(&path).status();
                        });
                    },
                },
            },
        },

        #[name = "back_btn"]
        gtk::Button {
            set_icon_name: "go-previous-symbolic",
            set_visible: false,
        },
    }

    async fn init(
        _service: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // ── Build header bar (must exist before view_output! for the titlebar reference) ──
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        // ── Compute model data (before view_output! so #[watch] can reference it) ──
        // NOTE: scope each `prefix_manager()` lock acquisition so we never
        // hold two guards on the same Mutex at once (would deadlock).
        let runtime_subtitle = {
            let svc = AppService::global();
            let pm = svc.prefix_manager();
            runtime_subtitle(&*pm.read_runtime())
        };
        let graphics_subtitle_str = graphics_subtitle();

        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tequila");
        let prefixes_dir = {
            let svc = AppService::global();
            svc.prefix_manager().wine_dir().clone()
        };

        // Create child subpage controllers (independent of widgets)
        let runtime_ctrl = runtime::RuntimeSettings::builder()
            .launch(root.clone())
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

        // ── GStreamer managed download row (macOS only) ──
        let gst_ctrl = managed_download_row::ManagedDownloadRow::builder()
            .launch(managed_download_row::ManagedDownloadRowInit {
                title: "GStreamer".into(),
                check_status: Box::new(gst_initial_status),
                check_update: None,
                start_download: Box::new(|data_dir, progress, cancel| {
                    Box::pin(async move {
                        prefix::runtime::download::download_gstreamer(
                            &data_dir,
                            progress,
                            Some(cancel),
                        )
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                    })
                }),
                perform_remove: Box::new(|| {
                    let gst_dir = prefix::runtime::download::runtimes_dir().join("gstreamer");
                    if gst_dir.exists() {
                        std::fs::remove_dir_all(&gst_dir).map_err(|e| e.to_string())
                    } else {
                        Ok(())
                    }
                }),
                data_dir: data_dir.clone(),
            })
            .forward(sender.input_sender(), |_out| {
                SettingsMsg::GStreamerStatusChanged
            });

        // Create local widgets referenced by #[local_ref] in view!
        let prefs_page = adw::PreferencesPage::new();

        // Placeholder nav — will be replaced with the real one from view! after view_output!()
        let placeholder_nav = adw::NavigationView::new();
        let mut model = SettingsWindow {
            runtime_subtitle,
            graphics_subtitle: graphics_subtitle_str,
            gst_ctrl,
            nav: placeholder_nav,
            runtime_ctrl,
            graphics_ctrl,
            tracker: 0,
        };

        // Generate all named widgets from view! block (needs model to exist)
        let widgets = view_output!();

        // Replace placeholder nav with the real one created by view!
        model.nav = widgets.nav.clone();

        // Add buttons to header_bar (must be after view_output! so named widgets exist)
        header_bar.pack_start(&widgets.back_btn);

        // Wire up back button to NavigationView
        {
            let nav = widgets.nav.clone();
            widgets.back_btn.connect_clicked(move |_| {
                nav.pop();
            });
        }

        // Show/hide back button when visible page changes
        {
            let nav = widgets.nav.clone();
            let back = widgets.back_btn.clone();
            let root_page = widgets.root_page.clone();
            nav.connect_notify_local(Some("visible-page"), move |nav, _| {
                let visible = nav.visible_page();
                let is_root = visible.as_ref().map_or(false, |p| *p == root_page);
                back.set_visible(!is_root);
            });
        }

        // Add GStreamer ManagedDownloadRow to its group (group declared in view!)
        widgets.gst_group.add(model.gst_ctrl.widget());

        // Load initial GitHub API key from settings
        if let Some(key) = prefix::Settings::load().and_then(|s| s.github_api_key) {
            widgets.github_key_entry.set_text(&key);
        }

        // ── Language combo setup ──
        let language_items = gtk::StringList::new(&["Follow System", "中文（简体）", "English"]);
        widgets.language_combo.set_model(Some(&language_items));

        // Load current language preference
        let current_lang = prefix::Settings::load()
            .map(|s| s.language)
            .unwrap_or_else(|| "system".to_string());
        let lang_idx: u32 = match current_lang.as_str() {
            "zh-CN" => 1,
            "en" => 2,
            _ => 0,
        };
        widgets.language_combo.set_selected(lang_idx);

        // Connect the signal AFTER setting the initial value, so it doesn't
        // trigger the "Language Changed" dialog on every startup.
        let lang_sender = sender.clone();
        widgets.language_combo.connect_selected_notify(move |combo| {
            lang_sender.input(SettingsMsg::LanguageChanged(combo.selected()));
        });

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
                self.runtime_subtitle = subtitle;
            }
            SettingsMsg::GraphicsSubtitleChanged(subtitle) => {
                self.graphics_subtitle = subtitle;
            }

            // ── Forwarded from RuntimeSettings ──
            SettingsMsg::RuntimesUpdated(rm) => {
                let svc = AppService::global();
                *svc.prefix_manager_mut().write_runtime() = rm;
                let pm = svc.prefix_manager();
                self.runtime_subtitle = runtime_subtitle(&*pm.read_runtime());
                let _ = sender.output(SettingsOutput::RuntimesUpdated(
                    pm.clone_runtime(),
                ));
            }

            // ── Forwarded from GStreamer child component ──
            SettingsMsg::GStreamerStatusChanged => {
                // GStreamer component manages its own state internally.
                // This message is available if the parent needs to react
                // (e.g., to update a section-level summary).
            }

            // ── GitHub API key ──
            SettingsMsg::GithubKeyChanged(value) => {
                let mut settings =
                    prefix::Settings::load().unwrap_or_else(|| RuntimeManager::new().into());
                settings.github_api_key = value;
                if let Err(e) = settings.save() {
                    log::error!("[settings] failed to save github_api_key: {}", e);
                }
            }
            // ── Language ──
            SettingsMsg::LanguageChanged(idx) => {
                let lang_str = match idx {
                    1 => "zh-CN",
                    2 => "en",
                    _ => "system",
                };
                let mut settings =
                    prefix::Settings::load().unwrap_or_else(|| RuntimeManager::new().into());
                settings.language = lang_str.to_string();
                if let Err(e) = settings.save() {
                    log::error!("[settings] failed to save language: {}", e);
                } else {
                    log::info!("[i18n] language preference saved: {}", lang_str);
                    // Show a dialog to inform the user that a restart is required
                    let alert = adw::AlertDialog::new(
                        Some(&crate::t!("settings.language_changed")),
                        Some(&crate::t!("settings.language_changed_desc")),
                    );
                    alert.add_response("ok", &crate::t!("dialogs.ok"));
                    alert.set_default_response(Some("ok"));
                    alert.set_close_response("ok");
                    alert.choose(
                        Some(&root.clone().upcast::<gtk::Window>()),
                        None::<&gtk::gio::Cancellable>,
                        |_| {},
                    );
                }
            }
            // ── Window ──
            SettingsMsg::Close => {
                root.set_visible(false);
            }
        }
    }
}

// ── GStreamer helpers ───────────────────────────────────────────────────

fn gst_initial_status() -> managed_download_row::DownloadRowStatus {
    // Check managed installation (has version.txt)
    let gst_dir = download::runtimes_dir().join("gstreamer");
    let managed = std::fs::read_to_string(gst_dir.join("version.txt"))
        .ok()
        .and_then(|v| {
            let t = v.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });

    if let Some(ver) = managed {
        return managed_download_row::DownloadRowStatus {
            installed: true,
            managed: true,
            status_text: crate::tf!("settings.gstreamer.installed", "version" => &ver),
        };
    }

    // Homebrew GStreamer
    if let Ok(output) = std::process::Command::new("brew")
        .args(["--prefix", "gstreamer"])
        .output()
    {
        if output.status.success() {
            if let Ok(prefix) = String::from_utf8(output.stdout) {
                let p = std::path::Path::new(prefix.trim());
                if p.join("bin").join("gst-launch-1.0").exists() {
                    return managed_download_row::DownloadRowStatus {
                        installed: true,
                        managed: false,
                        status_text: crate::t!("settings.gstreamer.installed_system"),
                    };
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
            return managed_download_row::DownloadRowStatus {
                installed: true,
                managed: false,
                status_text: crate::t!("settings.gstreamer.installed_system"),
            };
        }
    }

    managed_download_row::DownloadRowStatus {
        installed: false,
        managed: false,
        status_text: crate::t!("settings.gstreamer.not_installed"),
    }
}
