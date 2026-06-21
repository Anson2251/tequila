use adw::prelude::*;
use prefix::runtime::{RuntimeManager, RuntimeSource};
use relm4::{
    RelmWidgetExt, adw,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use service::AppService;
use std::path::PathBuf;
use tracker;

#[tracker::track]
pub struct RuntimeManagerModel {
    #[tracker::do_not_track]
    list_box: gtk::ListBox,
    #[tracker::do_not_track]
    add_popover: gtk::Popover,
    #[tracker::do_not_track]
    menu_btn: gtk::MenuButton,
    #[tracker::do_not_track]
    count_label: gtk::Label,
}

#[derive(Debug)]
pub enum RuntimeManagerMsg {
    Refresh,
    SetDefault(String),
    RemoveRuntime(String),
    ImportRuntime,
    ImportFromPath(PathBuf),
    Close,
}

#[derive(Debug)]
pub enum RuntimeManagerOutput {
    RuntimesUpdated(RuntimeManager),
}

#[relm4::component(pub, async)]
impl AsyncComponent for RuntimeManagerModel {
    type Init = ();
    type Input = RuntimeManagerMsg;
    type Output = RuntimeManagerOutput;
    type CommandOutput = ();
    type Widgets = RuntimeManagerWidgets;

    view! {
        #[root]
        gtk::Window {
            set_title: Some(&crate::t!("settings.runtime.title")),
            set_default_width: 520,
            set_default_height: 420,
            set_modal: true,
            set_hide_on_close: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                #[name = "header_bar"]
                gtk::HeaderBar {
                    add_css_class: "flat",
                },

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hexpand: true,

                    #[name = "list_box"]
                    gtk::ListBox {
                        set_selection_mode: gtk::SelectionMode::None,
                        add_css_class: "rich-list",
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    set_margin_all: 8,

                    #[name = "menu_btn"]
                    gtk::MenuButton {
                        set_icon_name: "list-add-symbolic",
                        set_tooltip_text: Some(&crate::t!("settings.runtime.add")),
                    },

                    gtk::Box {
                        set_hexpand: true,

                        #[name = "count_label"]
                        gtk::Label {
                            set_halign: gtk::Align::End,
                            set_valign: gtk::Align::Center,
                            add_css_class: "caption",
                        },
                    },
                },
            },
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let widgets = view_output!();

        // Title widget
        widgets
            .header_bar
            .set_title_widget(Some(&adw::WindowTitle::new(&crate::t!("settings.runtime.installed"), "")));

        // Close button
        {
            let close_btn = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .build();
            let s = sender.clone();
            close_btn.connect_clicked(move |_| {
                s.input(RuntimeManagerMsg::Close);
            });
            widgets.header_bar.pack_end(&close_btn);
        }

        // Build Add popover with Stack
        let add_popover = build_add_popover(sender.clone());
        widgets.menu_btn.set_popover(Some(&add_popover));

        // Connect close-request to hide instead of destroy
        {
            let s = sender.clone();
            root.connect_close_request(move |_win| {
                let _ = s.input(RuntimeManagerMsg::Close);
                gtk::glib::Propagation::Stop
            });
        }

        let mut model = RuntimeManagerModel {
            list_box: widgets.list_box.clone(),
            add_popover,
            count_label: widgets.count_label.clone(),
            menu_btn: widgets.menu_btn.clone(),
            tracker: 0,
        };

        let rm = AppService::global()
            .prefix_manager()
            .clone_runtime();
        populate_runtime_list(&model.list_box, &rm, sender.clone());
        update_count_label(&model.count_label, &rm);

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
            RuntimeManagerMsg::Refresh => {
                let rm = AppService::global()
                    .prefix_manager()
                    .clone_runtime();
                populate_runtime_list(&self.list_box, &rm, sender.clone());
                update_count_label(&self.count_label, &rm);
            }
            RuntimeManagerMsg::SetDefault(id) => {
                let svc = AppService::global();
                let pm = svc.prefix_manager_mut();
                pm.set_default_runtime(&id);
                pm.save_runtime_state();
                let rm = pm.clone_runtime();
                drop(pm);
                populate_runtime_list(&self.list_box, &rm, sender.clone());
                emit_updated(&rm, &sender);
            }
            RuntimeManagerMsg::RemoveRuntime(id) => {
                if id != "wine-system" {
                    let svc = AppService::global();
                    let pm = svc.prefix_manager_mut();
                    pm.remove_runtime(&id);
                    let rm = pm.clone_runtime();
                    drop(pm);
                    populate_runtime_list(&self.list_box, &rm, sender.clone());
                    update_count_label(&self.count_label, &rm);
                    emit_updated(&rm, &sender);
                }
            }
            RuntimeManagerMsg::ImportRuntime => {
                self.add_popover.popdown();

                let file_dialog = gtk::FileDialog::builder()
                    .title(&crate::t!("settings.runtime.select_wine"))
                    .build();
                let s = sender.clone();
                file_dialog.select_folder(
                    Some(root),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let Ok(file) = result {
                            if let Some(path) = file.path() {
                                let _ = s.input(RuntimeManagerMsg::ImportFromPath(path));
                            }
                        }
                    },
                );
            }
            RuntimeManagerMsg::ImportFromPath(path) => {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("imported")
                    .to_string();

                let svc = AppService::global();
                let pm_import = svc.prefix_manager_mut();
                match pm_import.import_runtime(&path, &dir_name) {
                    Ok(_runtime) => {
                        pm_import.save_runtime_state();
                        let rm = pm_import.clone_runtime();
                        drop(pm_import);
                        populate_runtime_list(&self.list_box, &rm, sender.clone());
                        update_count_label(&self.count_label, &rm);
                        emit_updated(&rm, &sender);
                    }
                    Err(e) => {
                        log::error!("[runtime] import failed: {}", e);
                        let msg = crate::tf!("settings.runtime.import_failed_desc", "error" => &e.to_string());
                        let alert = adw::AlertDialog::new(
                            Some(&crate::t!("settings.runtime.import_failed")),
                            Some(&msg),
                        );
                        alert.add_response("ok", &crate::t!("dialogs.ok"));
                        alert.set_default_response(Some("ok"));
                        alert.set_close_response("ok");
                        alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
                    }
                }
            }
            RuntimeManagerMsg::Close => {
                root.set_visible(false);
            }
        }
    }
}

// ── Widget construction helpers ────────────────────────────────────────

fn build_add_popover(sender: AsyncComponentSender<RuntimeManagerModel>) -> gtk::Popover {
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();
    box_.append(
        &gtk::Label::builder()
            .label("Add Wine Runtime")
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .margin_bottom(6)
            .build(),
    );

    let btn = gtk::Button::builder()
        .label("Import from Disk")
        .halign(gtk::Align::Fill)
        .build();
    btn.connect_clicked(move |_| {
        sender.input(RuntimeManagerMsg::ImportRuntime);
    });
    box_.append(&btn);

    let popover = gtk::Popover::new();
    popover.set_width_request(280);
    popover.set_child(Some(&box_));

    popover
}

// ── List population ────────────────────────────────────────────────────

fn populate_runtime_list(
    list_box: &gtk::ListBox,
    rm: &RuntimeManager,
    sender: AsyncComponentSender<RuntimeManagerModel>,
) {
    while let Some(row) = list_box.first_child() {
        list_box.remove(&row);
    }

    for runtime in &rm.runtimes {
        let is_default = runtime.id == rm.default_id;

        let name_label = gtk::Label::builder()
            .label(&runtime.name)
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build();

        let version_label = gtk::Label::builder()
            .label(&format!("Version: {}", runtime.wine_version))
            .halign(gtk::Align::Start)
            .css_classes(["caption"])
            .build();

        let source_str = match &runtime.source {
            RuntimeSource::System => "System (PATH)".to_string(),
            RuntimeSource::ManagedVersion {
                source_url,
                version,
            } => {
                let label = prefix::runtime::managed_source_label(&source_url);
                if version.is_empty() {
                    format!("Managed ({})", label)
                } else {
                    format!("{} ({})", label, version)
                }
            }
            RuntimeSource::Imported {
                label,
                original_path,
            } => {
                format!("Imported: {} ({})", label, original_path.display())
            }
        };

        let source_label = gtk::Label::builder()
            .label(&source_str)
            .halign(gtk::Align::Start)
            .css_classes(["caption", "dim-label"])
            .build();

        let date_label = gtk::Label::builder()
            .label(&format!("Installed: {}", &runtime.installed_at[..10]))
            .halign(gtk::Align::Start)
            .css_classes(["caption", "dim-label"])
            .build();

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .margin_start(8)
            .margin_end(8)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        info_box.append(&name_label);
        info_box.append(&version_label);
        info_box.append(&source_label);
        info_box.append(&date_label);

        let default_btn = gtk::Button::builder()
            .label(if is_default { "●" } else { "○" })
            .tooltip_text("Set as default runtime")
            .css_classes(["flat", "circular"])
            .build();

        let actions_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();

        if is_default {
            let badge = gtk::Label::builder()
                .label("Default")
                .css_classes(["caption", "accent"])
                .halign(gtk::Align::Start)
                .margin_start(4)
                .build();
            actions_box.append(&badge);
        }

        if !matches!(runtime.source, RuntimeSource::System) {
            let id = runtime.id.clone();
            let s = sender.clone();
            let remove_btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove Runtime")
                .css_classes(["flat", "destructive-action"])
                .build();
            remove_btn.connect_clicked(move |_| {
                s.input(RuntimeManagerMsg::RemoveRuntime(id.clone()));
            });
            actions_box.append(&remove_btn);
        }

        let row_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(4)
            .margin_end(4)
            .build();
        row_box.append(&default_btn);
        row_box.append(&info_box);
        row_box.append(&actions_box);

        {
            let id = runtime.id.clone();
            let s = sender.clone();
            default_btn.connect_clicked(move |_| {
                s.input(RuntimeManagerMsg::SetDefault(id.clone()));
            });
        }

        let row = gtk::ListBoxRow::builder()
            .selectable(false)
            .activatable(false)
            .child(&row_box)
            .build();

        list_box.append(&row);
    }
}

fn update_count_label(label: &gtk::Label, rm: &RuntimeManager) {
    let count = rm.runtimes.len();
    label.set_label(&format!(
        "{} runtime{}",
        count,
        if count == 1 { "" } else { "s" }
    ));
}

fn emit_updated(rm: &RuntimeManager, sender: &AsyncComponentSender<RuntimeManagerModel>) {
    let _ = sender.output(RuntimeManagerOutput::RuntimesUpdated(rm.clone()));
}
