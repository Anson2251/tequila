use adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, adw, gtk};
use tracker;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontSubstituteEntry {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Default)]
pub struct FontsSettings {
    pub system_font: Option<String>,
    pub shell_dlg_font: Option<String>,
    pub shell_dlg_2_font: Option<String>,
    pub substitutions: Vec<FontSubstituteEntry>,
}

#[derive(Debug)]
#[tracker::track]
pub struct FontsTabModel {
    editing: bool,
    system_font: String,
    #[tracker::do_not_track]
    system_font_entry: gtk::Entry,
    #[tracker::do_not_track]
    shell_dlg_font: Option<String>,
    #[tracker::do_not_track]
    shell_dlg_2_font: Option<String>,
    #[tracker::do_not_track]
    mismatch_dialog_shown: bool,
    #[tracker::do_not_track]
    draft_source: String,
    #[tracker::do_not_track]
    draft_target: String,
    #[tracker::do_not_track]
    substitutions: Vec<FontSubstituteEntry>,
    #[tracker::do_not_track]
    repl_group: adw::PreferencesGroup,
    #[tracker::do_not_track]
    rows: Vec<gtk::ListBoxRow>,
    #[tracker::do_not_track]
    root_widget: gtk::ScrolledWindow,
}

#[derive(Debug)]
pub enum FontsTabInput {
    SetEditing(bool),
    LoadSettings(FontsSettings),
    CommitSystemFont,
    UpdateSystemFont(String),
    UpdateDraftSource(String),
    UpdateDraftTarget(String),
    UpdateSubstitutionSource(usize, String),
    UpdateSubstitutionTarget(usize, String),
    AddSubstitution,
    RemoveSubstitution(usize),
}

#[derive(Debug)]
pub enum FontsTabOutput {
    SettingChanged(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for FontsTabModel {
    type Init = FontsSettings;
    type Input = FontsTabInput;
    type Output = FontsTabOutput;
    type Widgets = FontsTabWidgets;

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_vexpand: true,
            set_hexpand: true,
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vscrollbar_policy: gtk::PolicyType::Automatic,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_all: 18,
                set_spacing: 18,
                set_vexpand: true,
                set_hexpand: true,

                adw::PreferencesGroup {
                    set_title: &crate::t!("registry.fonts.title"),
                    set_description: Some(&crate::t!("registry.fonts.desc")),

                    adw::ActionRow {
                        set_title: &crate::t!("registry.fonts.system_font"),
                        set_subtitle: &crate::t!("registry.fonts.system_font_sub"),

                        #[name = "system_font_entry"]
                        add_suffix = &gtk::Entry {
                            set_width_chars: 20,
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                            #[track = "model.changed(FontsTabModel::editing())"]
                            set_editable: model.editing,
                            #[track = "model.changed(FontsTabModel::editing())"]
                            set_sensitive: model.editing,
                            connect_activate => FontsTabInput::CommitSystemFont,
                            connect_has_focus_notify[sender] => move |entry| {
                                if entry.is_editable() && !entry.has_focus() {
                                    sender.input(FontsTabInput::CommitSystemFont);
                                }
                            },
                        },
                    },
                },

                #[name = "repl_group"]
                adw::PreferencesGroup {
                    set_title: &crate::t!("registry.fonts.substitutes"),
                    set_description: Some(&crate::t!("registry.fonts.substitutes_sub")),
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = FontsTabModel {
            editing: false,
            system_font: init.system_font.clone().unwrap_or_default(),
            system_font_entry: gtk::Entry::new(),
            shell_dlg_font: init.shell_dlg_font,
            shell_dlg_2_font: init.shell_dlg_2_font,
            mismatch_dialog_shown: false,
            draft_source: String::new(),
            draft_target: String::new(),
            substitutions: init.substitutions,
            repl_group: adw::PreferencesGroup::new(),
            rows: Vec::new(),
            root_widget: root.clone(),
            tracker: 0,
        };

        let widgets = view_output!();
        model.repl_group = widgets.repl_group.clone();
        model.system_font_entry = widgets.system_font_entry.clone();
        model.system_font_entry.set_text(&model.system_font);
        model.refresh_list(&sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            FontsTabInput::SetEditing(v) => {
                self.set_editing(v);
                self.refresh_list(&sender);
                if v {
                    self.mismatch_dialog_shown = false;
                    maybe_show_shell_dlg_mismatch_dialog(self, self.root_widget.as_ref(), sender.clone());
                }
            }
            FontsTabInput::LoadSettings(s) => {
                self.set_system_font(s.system_font.clone().unwrap_or_default());
                self.system_font_entry
                    .set_text(s.system_font.as_deref().unwrap_or_default());
                self.shell_dlg_font = s.shell_dlg_font;
                self.shell_dlg_2_font = s.shell_dlg_2_font;
                self.draft_source.clear();
                self.draft_target.clear();
                self.substitutions = s.substitutions;
                self.refresh_list(&sender);
            }
            FontsTabInput::CommitSystemFont => {
                let value = self.system_font_entry.text().to_string();
                sender.input(FontsTabInput::UpdateSystemFont(value));
            }
            FontsTabInput::UpdateSystemFont(value) => {
                let normalized = value.trim().to_string();
                if normalized == self.system_font {
                    return;
                }
                self.set_system_font(normalized.clone());
                self.system_font_entry.set_text(&normalized);
                self.shell_dlg_font = if normalized.is_empty() { None } else { Some(normalized.clone()) };
                self.shell_dlg_2_font = if normalized.is_empty() { None } else { Some(normalized.clone()) };
                self.mismatch_dialog_shown = true;
                emit_font_setting(&sender, "MS Shell Dlg", normalized.as_str());
                emit_font_setting(&sender, "MS Shell Dlg 2", normalized.as_str());
            }
            FontsTabInput::UpdateDraftSource(value) => {
                self.draft_source = value;
            }
            FontsTabInput::UpdateDraftTarget(value) => {
                self.draft_target = value;
            }
            FontsTabInput::UpdateSubstitutionSource(idx, value) => {
                if let Some(item) = self.substitutions.get_mut(idx) {
                    let old_source = item.source.trim().to_string();
                    item.source = value;
                    let new_source = item.source.trim().to_string();
                    if !old_source.is_empty() && old_source != new_source {
                        emit_delete_setting(&sender, &old_source);
                    }
                    emit_substitution_for_index(&sender, &self.substitutions, idx);
                }
            }
            FontsTabInput::UpdateSubstitutionTarget(idx, value) => {
                if let Some(item) = self.substitutions.get_mut(idx) {
                    item.target = value;
                    emit_substitution_for_index(&sender, &self.substitutions, idx);
                }
            }
            FontsTabInput::AddSubstitution => {
                let source = self.draft_source.trim().to_string();
                let target = self.draft_target.trim().to_string();
                if source.is_empty() {
                    return;
                }

                self.substitutions.insert(0, FontSubstituteEntry {
                    source: source.clone(),
                    target: target.clone(),
                });
                self.draft_source.clear();
                self.draft_target.clear();
                let idx = self.substitutions.len() - 1;
                emit_substitution_for_index(&sender, &self.substitutions, idx);
                self.refresh_list(&sender);
            }
            FontsTabInput::RemoveSubstitution(idx) => {
                if idx < self.substitutions.len() {
                    let removed = self.substitutions.remove(idx);
                    let source = removed.source.trim();
                    if !source.is_empty() {
                        emit_delete_setting(&sender, source);
                    }
                    self.refresh_list(&sender);
                }
            }
        }
    }
}

impl FontsTabModel {
    fn refresh_list(&mut self, sender: &ComponentSender<Self>) {
        for row in self.rows.drain(..) {
            self.repl_group.remove(&row);
        }

        if self.editing {
            let add_btn = gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text(crate::t!("registry.fonts.add_substitute"))
                .visible(self.editing)
                .sensitive(self.editing)
                .build();
            {
                let s = sender.clone();
                add_btn.connect_clicked(move |_| {
                    s.input(FontsTabInput::AddSubstitution);
                });
            }

            let draft_row = gtk::ListBoxRow::new();
            draft_row.set_selectable(false);
            draft_row.set_activatable(false);

            let draft_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            draft_box.set_margin_start(12);
            draft_box.set_margin_end(12);
            draft_box.set_margin_top(6);
            draft_box.set_margin_bottom(6);

            let draft_source_entry = gtk::Entry::builder()
                .hexpand(true)
                .placeholder_text(crate::t!("registry.fonts.source_placeholder"))
                .text(&self.draft_source)
                .editable(self.editing)
                .sensitive(self.editing)
                .build();
            {
                let s = sender.clone();
                draft_source_entry.connect_changed(move |entry| {
                    s.input(FontsTabInput::UpdateDraftSource(entry.text().to_string()));
                });
            }

            let draft_target_entry = gtk::Entry::builder()
                .hexpand(true)
                .placeholder_text(crate::t!("registry.fonts.target_placeholder"))
                .text(&self.draft_target)
                .editable(self.editing)
                .sensitive(self.editing)
                .build();
            {
                let s = sender.clone();
                draft_target_entry.connect_changed(move |entry| {
                    s.input(FontsTabInput::UpdateDraftTarget(entry.text().to_string()));
                });
            }

            draft_box.append(&draft_source_entry);
            draft_box.append(&draft_target_entry);
            draft_box.append(&add_btn);
            draft_row.set_child(Some(&draft_box));
            self.repl_group.add(&draft_row);
            self.rows.push(draft_row);
        }

        for (idx, entry) in self.substitutions.iter().enumerate() {
            let row = gtk::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);

            let source_entry = gtk::Entry::builder()
                .hexpand(true)
                .placeholder_text(crate::t!("registry.fonts.source_placeholder"))
                .text(&entry.source)
                .editable(self.editing)
                .sensitive(self.editing)
                .build();
            {
                let s = sender.clone();
                source_entry.connect_changed(move |entry| {
                    s.input(FontsTabInput::UpdateSubstitutionSource(
                        idx,
                        entry.text().to_string(),
                    ));
                });
            }

            let target_entry = gtk::Entry::builder()
                .hexpand(true)
                .placeholder_text(crate::t!("registry.fonts.target_placeholder"))
                .text(&entry.target)
                .editable(self.editing)
                .sensitive(self.editing)
                .build();
            {
                let s = sender.clone();
                target_entry.connect_changed(move |entry| {
                    s.input(FontsTabInput::UpdateSubstitutionTarget(
                        idx,
                        entry.text().to_string(),
                    ));
                });
            }

            let remove_btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(crate::t!("registry.fonts.remove_substitute"))
                .visible(self.editing)
                .sensitive(self.editing)
                .css_classes(["destructive-action"])
                .build();
            {
                let s = sender.clone();
                remove_btn.connect_clicked(move |_| {
                    s.input(FontsTabInput::RemoveSubstitution(idx));
                });
            }

            hbox.append(&source_entry);
            hbox.append(&target_entry);
            hbox.append(&remove_btn);
            row.set_child(Some(&hbox));

            self.repl_group.add(&row);
            self.rows.push(row);
        }
    }
}

fn emit_font_setting(sender: &ComponentSender<FontsTabModel>, key: &str, value: &str) {
    let _ = sender.output(FontsTabOutput::SettingChanged(
        "Software\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes".into(),
        format!("{}={}", key, value.trim()),
    ));
}

fn maybe_show_shell_dlg_mismatch_dialog(
    model: &FontsTabModel,
    parent: &gtk::Widget,
    sender: ComponentSender<FontsTabModel>,
) {
    if model.mismatch_dialog_shown {
        return;
    }

    let Some(shell_dlg) = model.shell_dlg_font.clone() else {
        return;
    };
    let Some(shell_dlg_2) = model.shell_dlg_2_font.clone() else {
        return;
    };

    if shell_dlg == shell_dlg_2 {
        return;
    }

    let parent_window = parent
        .ancestor(gtk::Window::static_type())
        .and_then(|w| w.downcast::<gtk::Window>().ok());

    let alert = adw::AlertDialog::new(
        Some(&crate::t!("registry.fonts.mismatch.title")),
        Some(&crate::tf!(
            "registry.fonts.mismatch.body",
            "shell_dlg" => &shell_dlg,
            "shell_dlg_2" => &shell_dlg_2
        )),
    );
    alert.add_response("ok", &crate::t!("dialogs.ok"));
    alert.add_response("unify", &crate::tf!(
        "registry.fonts.mismatch.unify",
        "shell_dlg" => &shell_dlg,
    ));
    alert.set_response_appearance("unify", adw::ResponseAppearance::Suggested);
    alert.set_default_response(Some("ok"));
    alert.set_close_response("ok");

    let s = sender.clone();
    let font_val = shell_dlg.clone();
    alert.choose(
        parent_window.as_ref(),
        None::<&gtk::gio::Cancellable>,
        move |response| {
            if response == "unify" {
                s.input(FontsTabInput::UpdateSystemFont(font_val.clone()));
            }
        },
    );
}

fn emit_delete_setting(sender: &ComponentSender<FontsTabModel>, key: &str) {
    let _ = sender.output(FontsTabOutput::SettingChanged(
        "Software\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes".into(),
        format!("{}=", key),
    ));
}

fn emit_substitution_for_index(
    sender: &ComponentSender<FontsTabModel>,
    substitutions: &[FontSubstituteEntry],
    idx: usize,
) {
    if let Some(item) = substitutions.get(idx) {
        let source = item.source.trim();
        let target = item.target.trim();
        if source.is_empty() {
            return;
        }
        let _ = sender.output(FontsTabOutput::SettingChanged(
            "Software\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes".into(),
            format!("{}={}", source, target),
        ));
    }
}
