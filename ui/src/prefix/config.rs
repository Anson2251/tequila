use crate::registry_editor::{RegistryEditorModel, RegistryEditorMsg};
use adw::prelude::*;
use prefix::ProcessTracker;
use prefix::config::PrefixConfig;
use prefix::runtime;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
    adw, gtk,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
    let mut items = vec!["None"];
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
    type Init = (
        PathBuf,
        PrefixConfig,
        Arc<prefix::PrefixStore>,
        Arc<Mutex<ProcessTracker>>,
        gtk::Button,
    );
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
                    set_title: "Prefix Config",
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
                    set_title: "General",

                    #[name = "name_row"]
                    adw::EntryRow {
                        set_title: "Name",
                        #[track = "model.changed(PrefixConfigModel::editing())"]
                        set_editable: model.editing,
                        #[track = "model.changed(PrefixConfigModel::config())"]
                        set_text: &model.config.name,
                    },

                    adw::ActionRow {
                        set_title: "Architecture",
                        #[track = "model.changed(PrefixConfigModel::config())"]
                        set_subtitle: &model.config.architecture,
                        set_activatable: false,
                    },

                    adw::ActionRow {
                        set_title: "Wine Version",
                        #[track = "model.changed(PrefixConfigModel::wine_runtime_display())"]
                        set_subtitle: &model.wine_runtime_display,
                        set_activatable: false,
                    },
                },

                // ══ Description (populated in init) ══
                #[name = "description_group"]
                adw::PreferencesGroup {
                    set_title: "Description",
                },

                // ══ Info ══
                adw::PreferencesGroup {
                    set_title: "Info",

                    adw::ActionRow {
                        set_title: "Created",
                        #[track = "model.changed(PrefixConfigModel::config())"]
                        set_subtitle: &model.config.creation_date.format("%Y-%m-%d %H:%M:%S").to_string(),
                        set_activatable: false,
                    },

                    adw::ActionRow {
                        set_title: "Last Modified",
                        #[track = "model.changed(PrefixConfigModel::config())"]
                        set_subtitle: &model.config.last_modified.format("%Y-%m-%d %H:%M:%S").to_string(),
                        set_activatable: false,
                    },

                    adw::ActionRow {
                        set_title: "Path",
                        #[track = "model.changed(PrefixConfigModel::prefix_path())"]
                        set_subtitle: &model.prefix_path.to_string_lossy(),
                        set_activatable: false,
                        add_css_class: "monospace",
                    },
                },

                // ══ Graphics ══
                adw::PreferencesGroup {
                    set_title: "Graphics",

                    adw::ActionRow {
                        set_title: "Backend",
                        set_subtitle: "Graphics translation layer for Direct3D",

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
                    set_title: "Tools",

                    adw::ActionRow {
                        set_title: "Advanced Registry Settings",
                        set_subtitle: "Edit Wine registry keys (version, audio, graphics, windowing)",
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
                    set_label: if model.editing { "Save" } else { "Edit" },
                    #[track = "model.changed(PrefixConfigModel::editing())"]
                    set_css_classes: if model.editing { &["suggested-action"] } else { &[] },
                    connect_clicked => PrefixConfigMsg::ToggleEdit,
                },

                gtk::Button {
                    set_label: "Cancel",
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
        let (prefix_path, config, prefix_store, process_tracker, back_btn) = init;

        // ── Registry editor ──
        let registry_ctrl = RegistryEditorModel::builder()
            .launch((
                prefix_path.clone(),
                config.clone(),
                Arc::clone(&prefix_store),
                Arc::clone(&process_tracker),
            ))
            .forward(sender.input_sender(), |output| {
                PrefixConfigMsg::RegistryEditor(output)
            });
        let registry_page = adw::NavigationPage::builder()
            .title("Advanced Registry Settings")
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
                self.description_text.set_editable(false);
                self.sync_selected_graphics();
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
        self.description_text.set_editable(false);
        self.config.update_last_modified();
        if let Err(e) = self.config.save_to_file(&self.prefix_path) {
            eprintln!("Failed to save config: {}", e);
        }
        let _ = sender.output(PrefixConfigOutput::ConfigUpdated(self.config.clone()));
    }

    fn sync_selected_graphics(&mut self) {
        let idx = graphics_index_for_config(&self.graphics_backends, &self.config);
        // set_selected with same value is a no-op in GTK4,
        // so no infinite loop when ConfigUpdated comes back from our own change.
        self.set_selected_graphics(idx);
    }
}
