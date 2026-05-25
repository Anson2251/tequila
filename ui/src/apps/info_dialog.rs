use std::collections::HashMap;
use std::path::PathBuf;
use relm4::{
    gtk, adw, RelmWidgetExt, Component, Controller,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    ComponentParts, ComponentSender, SimpleComponent,
};
use adw::prelude::*;
use prefix::config::RegisteredExecutable;

#[derive(Debug)]
#[tracker::track]
pub struct ExecutableInfoDialogModel {
    executable: Option<RegisteredExecutable>,
    visible: bool,
    #[tracker::do_not_track]
    prefix_path: PathBuf,
    #[tracker::do_not_track]
    cwd_entry_row: adw::EntryRow,
    #[tracker::do_not_track]
    env_vars_editor: Option<Controller<EnvVarsEditor>>,
}

#[derive(Debug)]
pub enum ExecutableInfoDialogMsg {
    ShowInfo(RegisteredExecutable, PathBuf),
    Hide,
    SaveChanges,
    BrowseCwd,
    EditEnvVars,
    EnvVarsEdited(HashMap<String, String>),
}

#[derive(Debug)]
pub enum ExecutableInfoDialogOutput {
    ExecutableUpdated(RegisteredExecutable),
}

fn env_vars_to_text(vars: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = vars.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_env_vars(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let trimmed = line.trim();
            let mut parts = trimmed.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) if !key.is_empty() => {
                    Some((key.trim().to_string(), value.trim().to_string()))
                }
                _ => None,
            }
        })
        .collect()
}

fn env_vars_subtitle(executable: Option<&RegisteredExecutable>) -> String {
    match executable.and_then(|e| {
        if e.env_vars.is_empty() {
            None
        } else {
            Some(e.env_vars.len())
        }
    }) {
        Some(count) => format!("{} variable{} set", count, if count == 1 { "" } else { "s" }),
        None => "No environment variables set".to_string(),
    }
}

// ── EnvVarsEditor: popup component (like create_dialog.rs) ───────────────

#[derive(Debug)]
pub struct EnvVarsEditor {
    text_view: gtk::TextView,
    dialog: gtk::Window,
}

#[derive(Debug)]
pub enum EnvVarsEditorMsg {
    Apply,
    Cancel,
}

#[relm4::component(pub)]
impl SimpleComponent for EnvVarsEditor {
    type Init = (gtk::Window, String);
    type Input = EnvVarsEditorMsg;
    type Output = HashMap<String, String>;

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some("Edit Environment Variables"),
            set_modal: true,
            set_default_width: 420,
            set_default_height: 350,
            set_transient_for: Some(&parent),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 0,

                gtk::Label {
                    set_label: "One variable per line in KEY=VALUE format",
                    set_halign: gtk::Align::Start,
                    set_margin_start: 12,
                    set_margin_top: 12,
                    set_margin_end: 12,
                    set_margin_bottom: 4,
                },

                gtk::Label {
                    set_label: "Example:\n  WINEDLLOVERRIDES=winemenubuilder.exe=d\n  DXVK_HUD=1",
                    set_halign: gtk::Align::Start,
                    set_margin_start: 12,
                    set_margin_end: 12,
                    set_margin_bottom: 8,
                    add_css_class: "caption",
                },

                #[name = "text_view"]
                gtk::TextView {
                    set_editable: true,
                    set_wrap_mode: gtk::WrapMode::Word,
                    set_monospace: true,
                    set_margin_start: 8,
                    set_margin_end: 8,
                    set_margin_bottom: 8,
                    set_vexpand: true,
                },
            },
        }
    }

    fn init(
        (parent, initial_text): Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Build header bar manually (view! macro can't pack buttons into HeaderBar)
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        let apply_btn = gtk::Button::builder()
            .label("Apply")
            .icon_name("object-select-symbolic")
            .css_classes(["suggested-action"])
            .build();
        let s = sender.clone();
        apply_btn.connect_clicked(move |_| {
            let _ = s.input(EnvVarsEditorMsg::Apply);
        });

        #[cfg(target_os = "macos")]
        header_bar.pack_end(&apply_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&apply_btn);

        let widgets = view_output!();

        // Set initial text in the text view
        let buffer = widgets.text_view.buffer();
        buffer.set_text(&initial_text);

        widgets.dialog.set_titlebar(Some(&header_bar));

        let model = EnvVarsEditor {
            text_view: widgets.text_view.clone(),
            dialog: widgets.dialog.clone(),
        };

        widgets.dialog.present();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            EnvVarsEditorMsg::Apply => {
                let buffer = self.text_view.buffer();
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                let parsed = parse_env_vars(&text);
                let _ = sender.output(parsed);
                self.dialog.close();
            }
            EnvVarsEditorMsg::Cancel => {
                self.dialog.close();
            }
        }
    }
}

// ── Main dialog component ───────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for ExecutableInfoDialogModel {
    type Init = PathBuf;
    type Input = ExecutableInfoDialogMsg;
    type Output = ExecutableInfoDialogOutput;
    type CommandOutput = ();
    type Widgets = ExecutableInfoDialogWidgets;

    view! {
        #[name = "dialog"]
        gtk::Window {
            set_title: Some("Executable Information"),
            set_default_width: 520,
            set_default_height: 720,
            set_modal: true,
            set_resizable: true,
            #[watch]
            set_visible: model.visible,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 15,
                    set_margin_all: 20,

                    // ── Header with icon and info ──
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        // Icon or fallback
                        gtk::Box {
                            set_width_request: 64,
                            set_height_request: 64,
                            add_css_class: "icon-bg",
                            set_valign: gtk::Align::Center,

                            gtk::Image {
                                set_pixel_size: 64,
                                #[watch]
                                set_from_file: model.executable.as_ref().and_then(|e| e.icon_path.as_deref()),
                                #[watch]
                                set_visible: model.executable.as_ref().and_then(|e| e.icon_path.as_ref()).is_some(),
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Center,
                            },
                            gtk::Image {
                                set_pixel_size: 64,
                                set_icon_name: Some("application-x-executable"),
                                #[watch]
                                set_visible: model.executable.as_ref().and_then(|e| e.icon_path.as_ref()).is_none(),
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Center,
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 5,
                            set_hexpand: true,
                            set_valign: gtk::Align::Start,

                            gtk::Label {
                                #[watch]
                                set_label: model.executable.as_ref()
                                    .and_then(|e| e.product_name.as_deref())
                                    .unwrap_or(model.executable.as_ref().map(|e| e.name.as_str()).unwrap_or("")),
                                add_css_class: "title-2",
                                set_halign: gtk::Align::Start,
                                set_wrap: true,
                                set_wrap_mode: gtk::pango::WrapMode::WordChar,
                            },

                            gtk::Label {
                                #[watch]
                                set_label: model.executable.as_ref()
                                    .and_then(|e| e.description.as_deref())
                                    .unwrap_or("No description available"),
                                add_css_class: "body",
                                set_halign: gtk::Align::Start,
                                set_wrap: true,
                                set_wrap_mode: gtk::pango::WrapMode::WordChar,
                            },

                            // File Version
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_hexpand: true,
                                set_spacing: 8,

                                gtk::Label {
                                    set_label: "File Version:",
                                    set_halign: gtk::Align::Start,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: model.executable.as_ref()
                                        .and_then(|e| e.file_version.as_deref())
                                        .unwrap_or("N/A"),
                                    set_halign: gtk::Align::End,
                                    set_selectable: true,
                                    set_hexpand: true,
                                },
                            },

                            // Product Version
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 15,

                                gtk::Label {
                                    set_label: "Product Version:",
                                    set_halign: gtk::Align::Start,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: model.executable.as_ref()
                                        .and_then(|e| e.product_version.as_deref())
                                        .unwrap_or("N/A"),
                                    set_halign: gtk::Align::End,
                                    set_selectable: true,
                                    set_hexpand: true,
                                },
                            },

                            // Company Name
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 15,

                                gtk::Label {
                                    set_label: "Company:",
                                    set_halign: gtk::Align::Start,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: model.executable.as_ref()
                                        .and_then(|e| e.company_name.as_deref())
                                        .unwrap_or("N/A"),
                                    set_halign: gtk::Align::End,
                                    set_selectable: true,
                                    set_hexpand: true,
                                },
                            },
                        }
                    },

                    // ── Executable Path ──
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        gtk::Label {
                            set_label: "Path:",
                            set_halign: gtk::Align::Start,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.executable.as_ref()
                                .map(|e| e.executable_path.display().to_string())
                                .unwrap_or_else(|| "N/A".to_string()),
                            set_halign: gtk::Align::End,
                            set_selectable: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::Middle,
                            set_hexpand: true,
                        },
                    },
                },

                gtk::Separator {},

                // ── Preferences page ──
                adw::PreferencesPage {
                    set_vexpand: true,
                    set_hexpand: true,

                    // File Description
                    adw::PreferencesGroup {
                        set_title: "File Description",
                        #[watch]
                        set_description: Some(model.executable.as_ref()
                            .and_then(|e| e.file_description.as_deref())
                            .unwrap_or("N/A")),
                    },

                    // Imported Modules
                    adw::PreferencesGroup {
                        set_title: "Imported Modules",
                        #[watch]
                        set_visible: model.executable.as_ref()
                            .map(|e| !e.imported_modules.is_empty())
                            .unwrap_or(false),

                        adw::ActionRow {
                            set_subtitle_lines: 0,
                            set_subtitle_selectable: true,
                            #[watch]
                            set_subtitle: &model.executable.as_ref()
                                .map(|e| e.imported_modules.iter()
                                    .map(|m| format!("\u{2022} {}", m))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                    .to_uppercase())
                                .unwrap_or_default(),
                        },
                    },

                    // Execution Settings
                    adw::PreferencesGroup {
                        set_title: "Execution Settings",

                        // Working Directory
                        #[name = "cwd_entry_row"]
                        adw::EntryRow {
                            set_title: "Working Directory",

                            add_suffix = &gtk::Button {
                                set_icon_name: "folder-open-symbolic",
                                set_valign: gtk::Align::Center,
                                set_tooltip_text: Some("Choose a working directory"),
                                connect_clicked[sender] => move |_| {
                                    sender.input(ExecutableInfoDialogMsg::BrowseCwd);
                                },
                            },
                        },

                        // Environment Variables
                        adw::ActionRow {
                            set_title: "Environment Variables",
                            set_activatable: true,
                            #[watch]
                            set_subtitle: &env_vars_subtitle(model.executable.as_ref()),
                            connect_activated[sender] => move |_| {
                                sender.input(ExecutableInfoDialogMsg::EditEnvVars);
                            },
                        },
                    },
                },


            },

            connect_close_request[sender] => move |_| {
                sender.input(ExecutableInfoDialogMsg::Hide);
                gtk::glib::Propagation::Stop
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);

        // Save button in header bar
        let save_btn = gtk::Button::builder()
            .icon_name("object-select-symbolic")
            .tooltip_text("Save execution settings (env vars and working directory)")
            .css_classes(["suggested-action"])
            .build();
        let s = sender.clone();
        save_btn.connect_clicked(move |_| {
            let _ = s.input(ExecutableInfoDialogMsg::SaveChanges);
        });

        #[cfg(target_os = "macos")]
        header_bar.pack_end(&save_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&save_btn);

        // Close button (non-macOS only)
        #[cfg(not(target_os = "macos"))]
        {
            let close_btn = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text("Close")
                .build();
            let s = sender.clone();
            close_btn.connect_clicked(move |_| {
                let _ = s.input(ExecutableInfoDialogMsg::Hide);
            });
            header_bar.pack_end(&close_btn);
        }

        root.set_titlebar(Some(&header_bar));

        let mut model = ExecutableInfoDialogModel {
            executable: None,
            visible: false,
            prefix_path: init.canonicalize().unwrap_or(init),
            cwd_entry_row: adw::EntryRow::new(),
            env_vars_editor: None,
            tracker: 0,
        };

        let widgets = view_output!();

        model.cwd_entry_row = widgets.cwd_entry_row.clone();

        // Set tooltip text on the entry (can't be expressed as a GObject property in view! macro)
        widgets
            .cwd_entry_row
            .set_tooltip_text(Some("Custom working directory for the executable (e.g., /path/to/game)"));

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _widgets: &Self::Root,
    ) {
        self.reset();
        match msg {
            ExecutableInfoDialogMsg::ShowInfo(executable, prefix_path) => {
                let cwd_str = executable.cwd.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.cwd_entry_row.set_text(&cwd_str);
                self.prefix_path = prefix_path;
                self.set_executable(Some(executable));
                self.set_visible(true);
            }
            ExecutableInfoDialogMsg::Hide => {
                self.set_visible(false);
            }
            ExecutableInfoDialogMsg::SaveChanges => {
                if let Some(mut exec) = self.executable.clone() {
                    let cwd_text = self.cwd_entry_row.text().to_string();
                    exec.cwd = if cwd_text.trim().is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(cwd_text.trim()))
                    };
                    self.set_executable(Some(exec.clone()));
                    let _ = sender.output(ExecutableInfoDialogOutput::ExecutableUpdated(exec));
                }
                self.set_visible(false);
            }
            ExecutableInfoDialogMsg::BrowseCwd => {
                let parent_root = self.cwd_entry_row.root();
                if let Some(parent) = parent_root {
                    let entry = self.cwd_entry_row.clone();
                    let _current = self.cwd_entry_row.text().to_string();
                    let parent_window = parent.downcast::<gtk::Window>().unwrap();

                    crate::dialogs::pick_folder(
                        &parent_window,
                        Some(&(&self.prefix_path.join("drive_c")).to_string_lossy().to_string()),
                        move |path| {
                            entry.set_text(&path);
                        },
                    );
                }
            }
            ExecutableInfoDialogMsg::EditEnvVars => {
                let parent_root = self.cwd_entry_row.root();
                if let Some(parent) = parent_root {
                    let parent_window = parent.downcast::<gtk::Window>().unwrap();
                    let current_text = self.executable.as_ref()
                        .map(|e| env_vars_to_text(&e.env_vars))
                        .unwrap_or_default();

                    let editor = EnvVarsEditor::builder()
                        .launch((parent_window, current_text))
                        .forward(sender.input_sender(), |output| {
                            ExecutableInfoDialogMsg::EnvVarsEdited(output)
                        });
                    self.env_vars_editor = Some(editor);
                }
            }
            ExecutableInfoDialogMsg::EnvVarsEdited(vars) => {
                if let Some(mut exec) = self.executable.clone() {
                    exec.env_vars = vars;
                    self.set_executable(Some(exec));
                }
            }
        }
    }
}
