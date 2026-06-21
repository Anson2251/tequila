use crate::registry_editor::{RegistryEditorModel, RegistryEditorMsg};
use adw::prelude::*;
use prefix::config::PrefixConfig;
use prefix::runtime;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
    adw, gtk,
};
use service::AppService;
use std::path::PathBuf;
use tracker;

// ── Model ────────────────────────────────────────────────────────────────

#[derive(Debug)]
#[tracker::track]
pub struct PrefixConfigModel {
    prefix_path: PathBuf,
    config: PrefixConfig,
    saved_config: PrefixConfig,
    editing: bool,
    prefix_index: usize,
    wine_runtime_display: String,
    selected_graphics: u32,
    #[tracker::do_not_track]
    nav: adw::NavigationView,
    #[tracker::do_not_track]
    registry_ctrl: Controller<RegistryEditorModel>,
    #[tracker::do_not_track]
    description_buffer: gtk::TextBuffer,
    #[tracker::do_not_track]
    registry_page: adw::NavigationPage,
    #[tracker::do_not_track]
    description_text: gtk::TextView,
    #[tracker::do_not_track]
    back_btn: gtk::Button,
    #[tracker::do_not_track]
    graphics_items: gtk::StringList,
    #[tracker::do_not_track]
    graphics_backends: Vec<Option<prefix::base::GraphicsBackend>>,
    #[tracker::do_not_track]
    wine_runtime_items: gtk::StringList,
    #[tracker::do_not_track]
    wine_runtime_ids: Vec<String>,
    selected_wine_runtime: u32,
    #[tracker::do_not_track]
    parent_window: gtk::Window,
    #[tracker::do_not_track]
    reinitializing: bool,
    #[tracker::do_not_track]
    progress_bar: gtk::ProgressBar,
    #[tracker::do_not_track]
    pulse_id: Option<gtk::glib::SourceId>,
    #[tracker::do_not_track]
    progress_dialog: Option<gtk::Window>,
    edit_save_label: String,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum PrefixConfigMsg {
    ToggleEdit,
    SaveConfig,
    CancelEdit,
    UpdateName(String),
    UpdateDescription(String),
    ConfigUpdated(PrefixConfig),
    PrefixPathUpdated(PathBuf),
    SetPrefixIndex(usize),
    SetWineVersionDisplay(String),
    SelectWineVersion,
    WineVersionChanged(u32),
    SetProgressDialog(Option<gtk::Window>),
    SetPulseId(Option<gtk::glib::SourceId>),
    ReinitComplete(Result<(), String>),
    GraphicsBackendChanged(u32),
    ShowAdvancedRegistry,
    RegistryEditor(RegistryEditorMsg),
}

#[derive(Debug)]
pub enum PrefixConfigOutput {
    ConfigUpdated(PrefixConfig),
}

// ── Helper: build graphics dropdown items + mapping ──────────────────────

fn build_graphics_model() -> (gtk::StringList, Vec<Option<prefix::base::GraphicsBackend>>) {
    let backends = runtime::graphics::installed_backends();
    let mut items = vec!["WineD3D (built-in)"];
    let mut mapping: Vec<Option<prefix::base::GraphicsBackend>> = vec![None];
    for b in &backends {
        // Leak the string for a &'static str (the StringList holds its own copy)
        let label =
            Box::leak(format!("{} ({})", b.display_name(), b.version_string()).into_boxed_str());
        items.push(label);
        mapping.push(Some(b.clone()));
    }
    let list = gtk::StringList::new(&items);
    (list, mapping)
}

fn graphics_index_for_config(
    backends: &[Option<prefix::base::GraphicsBackend>],
    config: &PrefixConfig,
) -> u32 {
    config
        .graphics
        .as_ref()
        .and_then(|gfx| {
            backends.iter().position(|b| {
                b.as_ref()
                    .map(|be| be.label() == gfx.backend)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(0) as u32
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub)]
impl SimpleComponent for PrefixConfigModel {
    type Init = (PathBuf, PrefixConfig, gtk::Button, gtk::Window);
    type Input = PrefixConfigMsg;
    type Output = PrefixConfigOutput;
    type Widgets = PrefixConfigWidgets;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,

            #[name = "nav"]
            adw::NavigationView {
                set_hexpand: true,
                set_vexpand: true,

                push: root_page = &adw::NavigationPage {
                    set_title: &crate::t!("prefix.config.title"),
                    set_can_pop: false,
                    set_child: Some(&page_wrapper),
                },
            },
        },

        #[local_ref]
        page_wrapper -> gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[local_ref]
            prefs_page -> adw::PreferencesPage {
                // ══ General ══
                adw::PreferencesGroup {
                    set_title: &crate::t!("prefix.general"),

                    #[name = "name_row"]
                    adw::EntryRow {
                        set_title: &crate::t!("prefix.name"),
                        #[track = "model.changed(PrefixConfigModel::editing())"]
                        set_editable: model.editing,
                        #[track = "model.changed(PrefixConfigModel::config())"]
                        set_text: &model.config.name,
                    },

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.architecture"),
                        set_subtitle: &crate::t!("prefix.detail.arch"),
                        set_activatable: false,

                        add_suffix = &gtk::Label {
                            #[track = "model.changed(PrefixConfigModel::config())"]
                            set_label: &model.config.architecture,
                            set_css_classes: &["caption", "monospace"],
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.wine_version"),
                        set_subtitle: &crate::t!("prefix.detail.wine_version"),

                        add_suffix = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_valign: gtk::Align::Center,

                            gtk::Label {
                                #[track = "model.changed(PrefixConfigModel::wine_runtime_display())"]
                                set_label: &model.wine_runtime_display,
                                set_css_classes: &["caption"],
                                set_valign: gtk::Align::Center,
                            },

                            gtk::Button {
                                set_label: &crate::t!("prefix.detail.switch"),
                                set_css_classes: &["flat"],
                                set_valign: gtk::Align::Center,
                                connect_clicked[sender] => move |_| {
                                    sender.input(PrefixConfigMsg::SelectWineVersion);
                                },
                            },
                        },
                    },
                },

                // ══ Description (populated in init) ══
                #[name = "description_group"]
                adw::PreferencesGroup {
                    set_title: &crate::t!("prefix.description"),
                },

                // ══ Info ══
                adw::PreferencesGroup {
                    set_title: &crate::t!("prefix.info"),

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.detail.created"),
                        set_subtitle: &crate::t!("prefix.detail.created_sub"),
                        set_activatable: false,

                        add_suffix = &gtk::Label {
                            #[track = "model.changed(PrefixConfigModel::config())"]
                            set_label: &model.config.creation_date.format("%Y-%m-%d").to_string(),
                            set_css_classes: &["caption"],
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.detail.modified"),
                        set_subtitle: &crate::t!("prefix.detail.modified_sub"),
                        set_activatable: false,

                        add_suffix = &gtk::Label {
                            #[track = "model.changed(PrefixConfigModel::config())"]
                            set_label: &model.config.last_modified.format("%Y-%m-%d").to_string(),
                            set_css_classes: &["caption"],
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.detail.path"),
                        set_subtitle: &crate::t!("prefix.detail.path_sub"),
                        set_activatable: false,

                        add_suffix = &gtk::Label {
                            #[track = "model.changed(PrefixConfigModel::prefix_path())"]
                            set_label: &model.prefix_path.to_string_lossy(),
                            set_css_classes: &["caption", "monospace"],
                            set_hexpand: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::Start,
                            set_valign: gtk::Align::Center,
                            set_xalign: 1.0,
                        },
                    },
                },

                // ══ Graphics ══
                adw::PreferencesGroup {
                    set_visible: cfg!(not(target_os = "macos")),
                    set_title: &crate::t!("prefix.graphics"),

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.detail.graphics_backend"),
                        set_subtitle: &crate::t!("prefix.detail.graphics_sub"),

                        add_suffix = &gtk::DropDown {
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                            set_model: Some(&model.graphics_items),
                            #[track = "model.changed(PrefixConfigModel::selected_graphics())"]
                            set_selected: model.selected_graphics,
                            #[track = "model.changed(PrefixConfigModel::editing())"]
                            set_sensitive: model.editing,
                            connect_selected_notify[sender] => move |combo| {
                                sender.input(PrefixConfigMsg::GraphicsBackendChanged(
                                    combo.selected(),
                                ));
                            },
                        },
                    },
                },

                // ══ Tools ══
                adw::PreferencesGroup {
                    set_title: &crate::t!("prefix.tools"),

                    adw::ActionRow {
                        set_title: &crate::t!("prefix.detail.registry"),
                        set_subtitle: &crate::t!("prefix.detail.registry_sub"),
                        set_activatable: true,
                        connect_activated => PrefixConfigMsg::ShowAdvancedRegistry,
                    },
                },
            },

            // ══ Toolbar ══
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_halign: gtk::Align::End,
                set_margin_top: 18,
                set_margin_bottom: 12,
                set_margin_start: 12,
                set_margin_end: 12,

                gtk::Button {
                    #[track = "model.changed(PrefixConfigModel::editing())"]
                    set_label: &model.edit_save_label,
                    #[track = "model.changed(PrefixConfigModel::editing())"]
                    set_css_classes: if model.editing { &["suggested-action"] } else { &[] },
                    connect_clicked => PrefixConfigMsg::ToggleEdit,
                },

                gtk::Button {
                    set_label: &crate::t!("prefix.detail.cancel"),
                    #[track = "model.changed(PrefixConfigModel::editing())"]
                    set_visible: model.editing,
                    connect_clicked[sender] => move |_| {
                        sender.input(PrefixConfigMsg::CancelEdit);
                    },
                },
            },
        },
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefix_path, config, back_btn, parent_window) = init;

        // ── Build wine runtime dropdown ──
        let runtime_manager = AppService::global()
            .prefix_manager()
            .clone_runtime();
        let mut runtime_items: Vec<String> = Vec::new();
        let mut runtime_ids: Vec<String> = Vec::new();
        let mut selected_wine_runtime: u32 = 0;
        for (i, rt) in runtime_manager.runtimes.iter().enumerate() {
            runtime_ids.push(rt.id.clone());
            runtime_items.push(format!("{} ({})", rt.name, rt.wine_version));
            if Some(&rt.id) == config.wine_version.as_ref() {
                selected_wine_runtime = i as u32;
            }
        }
        // If no match, select "System Wine" (wine-system) if available, else first
        if runtime_ids.is_empty() {
            runtime_items.push(crate::t!("prefix.detail.no_runtimes"));
            runtime_ids.push(String::new());
        }
        let runtime_items_str: Vec<&str> = runtime_items.iter().map(|s| s.as_str()).collect();
        let wine_runtime_items = gtk::StringList::new(&runtime_items_str);

        // ── Registry editor (get dependencies from global) ──
        let prefix_store = AppService::global().prefix_store().clone();
        let process_tracker = AppService::global().process_tracker().clone();
        let registry_ctrl = RegistryEditorModel::builder()
            .launch((
                prefix_path.clone(),
                config.clone(),
                prefix_store,
                process_tracker,
                parent_window.clone(),
            ))
            .forward(sender.input_sender(), |output| {
                PrefixConfigMsg::RegistryEditor(output)
            });
        let registry_page = adw::NavigationPage::builder()
            .title(&crate::t!("prefix.detail.registry"))
            .child(registry_ctrl.widget())
            .build();

        // ── Graphics dropdown model ──
        let (graphics_items, graphics_backends) = build_graphics_model();
        let selected_graphics = graphics_index_for_config(&graphics_backends, &config);

        // ── Page wrapper ──
        let page_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let prefs_page = adw::PreferencesPage::new();
        page_wrapper.append(&prefs_page);

        let placeholder_nav = adw::NavigationView::new();
        let description_buffer = gtk::TextBuffer::new(None);
        let description_text = gtk::TextView::new();
        description_text.set_buffer(Some(&description_buffer));
        description_text.set_wrap_mode(gtk::WrapMode::WordChar);
        description_text.set_vexpand(false);
        description_text.set_margin_start(6);
        description_text.set_margin_end(6);
        description_text.set_margin_top(6);
        description_text.set_margin_bottom(6);
        description_text.set_css_classes(&["card", "view", "desc-text"]);

        let mut model = PrefixConfigModel {
            prefix_path: prefix_path.clone(),
            config: config.clone(),
            saved_config: config.clone(),
            editing: false,
            prefix_index: 0,
            wine_runtime_display: String::new(),
            selected_graphics,
            nav: placeholder_nav,
            registry_ctrl,
            description_buffer: description_buffer.clone(),
            registry_page,
            description_text,
            back_btn,
            graphics_items,
            graphics_backends,
            wine_runtime_items,
            wine_runtime_ids: runtime_ids,
            selected_wine_runtime,
            parent_window,
            reinitializing: false,
            progress_bar: gtk::ProgressBar::new(),
            pulse_id: None,
            progress_dialog: None,
            edit_save_label: crate::t!("prefix.detail.edit"),
            tracker: 0,
        };

        if let Some(ref desc) = model.config.description {
            model.description_buffer.set_text(desc);
        }

        let widgets = view_output!();
        model.nav = widgets.nav.clone();

        // ── Build description row (programmatic — needs ScrolledWindow) ──
        let desc_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .min_content_height(80)
            .max_content_height(160)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&model.description_text)
            .build();
        let desc_row = adw::ActionRow::new();
        desc_row.set_title("");
        desc_row.set_activatable(false);
        desc_row.set_child(Some(&desc_scroll));
        widgets.description_group.add(&desc_row);

        // ── Track description changes ──
        let s = sender.clone();
        let buf = model.description_buffer.clone();
        buf.connect_changed(move |_buf| {
            let (start, end) = _buf.bounds();
            s.input(PrefixConfigMsg::UpdateDescription(
                _buf.text(&start, &end, true).to_string(),
            ));
        });

        // ── Back button ──
        {
            let nav = widgets.nav.clone();
            let bb = model.back_btn.clone();
            bb.connect_clicked(move |_| {
                nav.pop();
            });
        }
        {
            let nav = widgets.nav.clone();
            let bb = model.back_btn.clone();
            let rp = widgets.root_page.clone();
            nav.connect_notify_local(Some("visible-page"), move |nav, _| {
                bb.set_visible(nav.visible_page().as_ref().map_or(false, |p| *p != rp));
            });
        }

        // ── Track name changes ──
        {
            let s = sender.clone();
            let nr = widgets.name_row.clone();
            nr.clone().connect_changed(move |_| {
                let text = nr.text().to_string();
                s.input(PrefixConfigMsg::UpdateName(text));
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            PrefixConfigMsg::ToggleEdit => {
                if self.editing {
                    sender.input(PrefixConfigMsg::SaveConfig);
                } else {
                    self.saved_config = self.config.clone();
                    self.set_editing(true);
                    self.set_edit_save_label(crate::t!("prefix.detail.save"));
                    self.description_text.set_editable(true);
                }
            }
            PrefixConfigMsg::SaveConfig => {
                self.save_config(sender);
            }
            PrefixConfigMsg::CancelEdit => {
                self.description_text.set_editable(false);
                self.description_buffer
                    .set_text(self.saved_config.description.as_deref().unwrap_or(""));
                self.set_config(self.saved_config.clone());
                self.set_editing(false);
                self.set_edit_save_label(crate::t!("prefix.detail.edit"));
                self.sync_selected_graphics();
            }
            PrefixConfigMsg::UpdateName(name) => self.config.name = name,
            PrefixConfigMsg::UpdateDescription(desc) => {
                self.config.description = if desc.is_empty() { None } else { Some(desc) };
            }
            PrefixConfigMsg::ConfigUpdated(config) => {
                if let Some(ref desc) = config.description {
                    self.description_buffer.set_text(desc);
                } else {
                    self.description_buffer.set_text("");
                }
                self.set_config(config.clone());
                self.saved_config = config;
                self.set_editing(false);
                self.set_edit_save_label(crate::t!("prefix.detail.edit"));
                self.description_text.set_editable(false);
                self.sync_selected_graphics();
                // Refresh runtime IDs so the display name reflects newly
                // downloaded runtimes (the cached list may be stale).
                self.refresh_runtime_cache();
                self.sync_wine_runtime_display();
                self.sync_wine_runtime_selection();
            }
            PrefixConfigMsg::PrefixPathUpdated(path) => {
                self.set_prefix_path(path.clone());
                if let Ok(Some(config)) = PrefixConfig::load_from_file(&path) {
                    self.set_config(config);
                    self.sync_selected_graphics();
                }
                self.registry_ctrl
                    .emit(RegistryEditorMsg::PrefixPathUpdated(path));
            }
            PrefixConfigMsg::SetPrefixIndex(index) => self.set_prefix_index(index),
            PrefixConfigMsg::SetWineVersionDisplay(d) => self.set_wine_runtime_display(d),
            PrefixConfigMsg::SelectWineVersion => {
                // Refresh runtime list from global state (user may have
                // downloaded new runtimes since this model was initialised).
                self.refresh_runtime_cache();

                let dropdown = gtk::DropDown::builder()
                    .model(&self.wine_runtime_items)
                    .selected(self.selected_wine_runtime)
                    .build();

                let alert = adw::AlertDialog::new(
                    Some(&crate::t!("prefix.detail.change_wine.title")),
                    Some(&crate::t!("prefix.detail.change_wine.desc")),
                );
                alert.set_extra_child(Some(&dropdown));
                alert.add_response("cancel", &crate::t!("prefix.detail.change_wine.cancel"));
                alert.add_response("change", &crate::t!("prefix.detail.change_wine.change"));
                alert.set_response_appearance("change", adw::ResponseAppearance::Destructive);
                alert.set_default_response(Some("cancel"));
                alert.set_close_response("cancel");

                let s = sender.clone();
                let pw = self.parent_window.clone();
                alert.choose(
                    Some(&self.parent_window),
                    None::<&gtk::gio::Cancellable>,
                    move |response| {
                        if response == "change" {
                            let idx = dropdown.selected();
                            let _ = s.input(PrefixConfigMsg::WineVersionChanged(idx));

                            // Show progress dialog
                            let pb = gtk::ProgressBar::new();
                            pb.pulse();
                            let id = gtk::glib::timeout_add_local(
                                std::time::Duration::from_millis(100),
                                {
                                    let pb = pb.clone();
                                    move || {
                                        pb.pulse();
                                        gtk::glib::ControlFlow::Continue
                                    }
                                },
                            );
                            let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
                            content.set_margin_top(20);
                            content.set_margin_end(20);
                            content.set_margin_bottom(20);
                            content.set_margin_start(20);
                            let label = gtk::Label::new(Some(&crate::t!("prefix.detail.change_wine.reinit")));
                            content.append(&label);
                            content.append(&pb);

                            let win = gtk::Window::builder()
                                .title(&crate::t!("prefix.detail.change_wine.progress_title"))
                                .modal(true)
                                .transient_for(&pw)
                                .resizable(false)
                                .default_width(350)
                                .child(&content)
                                .build();
                            win.present();
                            let _ = s.input(PrefixConfigMsg::SetProgressDialog(Some(win)));
                            let _ = s.input(PrefixConfigMsg::SetPulseId(Some(id)));
                        }
                    },
                );
            }
            PrefixConfigMsg::WineVersionChanged(idx) => {
                // Update config with selected runtime
                if let Some(id) = self.wine_runtime_ids.get(idx as usize) {
                    self.config.wine_version = if id.is_empty() {
                        None
                    } else {
                        Some(id.clone())
                    };
                }
                self.set_selected_wine_runtime(idx);

                // Save config via manager
                if let Err(e) = AppService::global()
                    .prefix_manager()
                    .update_config(&self.prefix_path, &self.config)
                {
                    log::error!("[prefix] failed to save config: {}", e);
                }
                self.saved_config = self.config.clone();
                let config = self.config.clone();
                let _ = sender.output(PrefixConfigOutput::ConfigUpdated(config));
            }
            PrefixConfigMsg::SetProgressDialog(dialog) => {
                self.progress_dialog = dialog;
            }
            PrefixConfigMsg::SetPulseId(id) => {
                self.pulse_id = id;
            }
            PrefixConfigMsg::ReinitComplete(result) => {
                self.reinitializing = false;
                if let Some(id) = self.pulse_id.take() {
                    id.remove();
                }
                if let Some(dialog) = self.progress_dialog.take() {
                    dialog.close();
                }

                if let Err(e) = result {
                    let alert = adw::AlertDialog::new(
                        Some(&crate::t!("prefix.detail.reinit_failed")),
                        Some(&crate::tf!("prefix.detail.reinit_failed_desc", "error" => &e)),
                    );
                    alert.add_response("ok", &crate::t!("dialogs.ok"));
                    alert.set_default_response(Some("ok"));
                    alert.set_close_response("ok");
                    alert.choose(
                        Some(&self.parent_window),
                        None::<&gtk::gio::Cancellable>,
                        |_| {},
                    );
                }
            }
            PrefixConfigMsg::GraphicsBackendChanged(idx) => {
                // Only update in-memory config — actual save happens on SaveConfig.
                let backend = self
                    .graphics_backends
                    .get(idx as usize)
                    .and_then(|b| b.clone());
                let new_gfx = backend.as_ref().map(|b| prefix::base::GraphicsConfig {
                    backend: b.label().to_string(),
                    version: b.version_string(),
                });
                self.config.graphics = new_gfx;
            }
            PrefixConfigMsg::ShowAdvancedRegistry => self.nav.push(&self.registry_page),
            PrefixConfigMsg::RegistryEditor(output) => {
                if let RegistryEditorMsg::ConfigUpdated(config) = output {
                    self.set_config(config.clone());
                    self.saved_config = config.clone();
                    self.set_editing(false);
                    self.set_edit_save_label(crate::t!("prefix.detail.edit"));
                    if let Some(ref desc) = config.description {
                        self.description_buffer.set_text(desc);
                    } else {
                        self.description_buffer.set_text("");
                    }
                    self.sync_selected_graphics();
                    let _ = sender.output(PrefixConfigOutput::ConfigUpdated(config));
                }
            }
        }
    }
}

// ── Impl ─────────────────────────────────────────────────────────────────

impl PrefixConfigModel {
    fn save_config(&mut self, sender: ComponentSender<Self>) {
        let (start, end) = self.description_buffer.bounds();
        let text = self.description_buffer.text(&start, &end, true);
        self.config.description = if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        };
        self.set_editing(false);
        self.set_edit_save_label(crate::t!("prefix.detail.edit"));
        self.description_text.set_editable(false);
        if let Err(e) = AppService::global()
            .prefix_manager()
            .update_config(&self.prefix_path, &self.config)
        {
            log::error!("[prefix] failed to save config: {}", e);
        }
        let _ = sender.output(PrefixConfigOutput::ConfigUpdated(self.config.clone()));
    }

    fn sync_selected_graphics(&mut self) {
        let idx = graphics_index_for_config(&self.graphics_backends, &self.config);
        // set_selected with same value is a no-op in GTK4,
        // so no infinite loop when ConfigUpdated comes back from our own change.
        self.set_selected_graphics(idx);
    }

    /// Re-read the runtime list from the global singleton and update
    /// the cached dropdown model + selection index.
    ///
    /// Must be called before showing the dropdown or after a
    /// `ConfigUpdated` to keep display names in sync with newly
    /// downloaded runtimes.
    fn refresh_runtime_cache(&mut self) {
        let svc = AppService::global();
        let pm = svc.prefix_manager();
        let rm = &*pm.read_runtime();
        self.wine_runtime_ids = rm.runtimes.iter().map(|rt| rt.id.clone()).collect();
        let fresh_items: Vec<String> = rm
            .runtimes
            .iter()
            .map(|rt| format!("{} ({})", rt.name, rt.wine_version))
            .collect();
        self.selected_wine_runtime = self
            .config
            .wine_version
            .as_ref()
            .and_then(|id| self.wine_runtime_ids.iter().position(|rid| rid == id))
            .unwrap_or(0) as u32;
        let str_refs: Vec<&str> = fresh_items.iter().map(|s| s.as_str()).collect();
        self.wine_runtime_items = gtk::StringList::new(&str_refs);
    }

    fn sync_wine_runtime_display(&mut self) {
        let display = self
            .config
            .wine_version
            .as_ref()
            .and_then(|id| {
                self.wine_runtime_ids
                    .iter()
                    .position(|rid| rid == id)
                    .map(|i| i as u32)
                    .and_then(|i| self.wine_runtime_items.string(i))
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| {
                self.config
                    .wine_version
                    .as_deref()
                    .unwrap_or("Unknown")
                    .to_string()
            });
        self.set_wine_runtime_display(display);
    }

    fn sync_wine_runtime_selection(&mut self) {
        let idx = self
            .wine_runtime_ids
            .iter()
            .position(|id| Some(id.as_str()) == self.config.wine_version.as_deref())
            .unwrap_or(0) as u32;
        self.set_selected_wine_runtime(idx);
    }
}
