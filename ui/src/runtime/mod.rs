use relm4::{
    gtk, adw,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender}, RelmWidgetExt,
};
use gtk::prelude::*;
use adw::prelude::*;
use prefix::{
    Manager as PrefixManager,
    runtime::{RuntimeSource, Channel, RuntimeManager},
};
use tracker;
use std::path::PathBuf;

#[tracker::track]
pub struct RuntimeManagerModel {
    downloading: bool,
    download_progress: f64,
    #[tracker::do_not_track]
    prefix_manager: PrefixManager,
    #[tracker::do_not_track]
    list_box: gtk::ListBox,
    #[tracker::do_not_track]
    add_popover: gtk::Popover,
    #[tracker::do_not_track]
    #[allow(dead_code)]
    channel_combo: gtk::DropDown,
    #[tracker::do_not_track]
    progress_bar: gtk::ProgressBar,
    #[tracker::do_not_track]
    progress_label: gtk::Label,
    #[tracker::do_not_track]
    #[allow(dead_code)]
    download_button: gtk::Button,
    #[tracker::do_not_track]
    download_stack: gtk::Stack,
    #[tracker::do_not_track]
    count_label: gtk::Label,
    #[tracker::do_not_track]
    menu_btn: gtk::MenuButton,
}

#[derive(Debug)]
pub enum RuntimeManagerMsg {
    Refresh,
    SetDefault(String),
    RemoveRuntime(String),
    ShowAddMenu,
    StartDownload(Channel),
    DownloadProgress(u64, u64),
    DownloadComplete(RuntimeManager),
    DownloadFailed(String),
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
    type Init = PrefixManager;
    type Input = RuntimeManagerMsg;
    type Output = RuntimeManagerOutput;
    type CommandOutput = ();
    type Widgets = RuntimeManagerWidgets;

    view! {
        #[root]
        gtk::Window {
            set_title: Some("Wine Runtimes"),
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
                        set_tooltip_text: Some("Add Runtime"),
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
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let prefix_manager = init;
        let widgets = view_output!();

        // Title widget
        widgets.header_bar.set_title_widget(Some(&adw::WindowTitle::new("Runtimes", "")));

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

        // Extract named widgets from the popover for model
        let download_stack = add_popover
            .child()
            .and_then(|c| c.downcast::<gtk::Stack>().ok())
            .unwrap_or_else(|| gtk::Stack::new());

        // Find channel_combo, progress_bar, progress_label, download_button in download stack
        let (channel_combo, progress_bar, progress_label, download_button) =
            extract_download_widgets(&download_stack);

        let model = RuntimeManagerModel {
            downloading: false,
            download_progress: 0.0,
            prefix_manager,
            list_box: widgets.list_box.clone(),
            add_popover,
            channel_combo,
            progress_bar,
            progress_label,
            download_button,
            download_stack,
            count_label: widgets.count_label.clone(),
            menu_btn: widgets.menu_btn.clone(),
            tracker: 0,
        };

        populate_runtime_list(
            &model.list_box,
            &model.prefix_manager.runtime_manager(),
            sender.clone(),
        );
        update_count_label(&model.count_label, &model.prefix_manager.runtime_manager());

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
                let rm = self.prefix_manager.runtime_manager().clone();
                populate_runtime_list(&self.list_box, &rm, sender.clone());
                update_count_label(&self.count_label, &rm);
            }
            RuntimeManagerMsg::SetDefault(id) => {
                self.prefix_manager.set_default_runtime(&id);
                self.prefix_manager.save_runtime_state();
                let rm = self.prefix_manager.runtime_manager().clone();
                populate_runtime_list(&self.list_box, &rm, sender.clone());
                emit_updated(&self.prefix_manager, &sender);
            }
            RuntimeManagerMsg::RemoveRuntime(id) => {
                if id != "wine-system" {
                    self.prefix_manager.remove_runtime(&id);
                    let rm = self.prefix_manager.runtime_manager().clone();
                    populate_runtime_list(&self.list_box, &rm, sender.clone());
                    update_count_label(&self.count_label, &rm);
                    emit_updated(&self.prefix_manager, &sender);
                }
            }
            RuntimeManagerMsg::ShowAddMenu => {
                self.download_stack.set_visible_child_name("download");
            }
            RuntimeManagerMsg::StartDownload(channel) => {
                self.set_downloading(true);
                self.menu_btn.set_sensitive(false);
                self.progress_bar.set_visible(true);
                self.progress_label.set_visible(true);
                self.progress_label.set_label("Starting download...");
                self.progress_bar.set_fraction(0.0);

                let mut pm = self.prefix_manager.clone();
                let s = sender.clone();

                gtk::glib::spawn_future_local(async move {
                    let progress: prefix::download::ProgressFn = Box::new({
                        let s = s.clone();
                        move |downloaded, total| {
                            let _ = s.input(RuntimeManagerMsg::DownloadProgress(downloaded, total));
                        }
                    });

                    match pm.download_channel_runtime(channel, progress).await {
                        Ok(_runtime) => {
                            let rm = pm.runtime_manager().clone();
                            let _ = s.input(RuntimeManagerMsg::DownloadComplete(rm));
                        }
                        Err(e) => {
                            let _ = s.input(RuntimeManagerMsg::DownloadFailed(e.to_string()));
                        }
                    }
                });
            }
            RuntimeManagerMsg::DownloadProgress(downloaded, total) => {
                if total > 0 {
                    let frac = downloaded as f64 / total as f64;
                    self.set_download_progress(frac);
                    self.progress_bar.set_fraction(frac);
                    let mb = |b: u64| b as f64 / 1_048_576.0;
                    self.progress_label.set_label(&format!(
                        "{:.1} / {:.1} MB",
                        mb(downloaded),
                        mb(total)
                    ));
                }
            }
            RuntimeManagerMsg::DownloadComplete(updated_rm) => {
                let rm_ref = self.prefix_manager.runtime_manager_mut();
                let _old = std::mem::replace(rm_ref, updated_rm);
                self.prefix_manager.save_runtime_state();

                self.set_downloading(false);
                self.set_download_progress(0.0);
                self.menu_btn.set_sensitive(true);
                self.progress_bar.set_visible(false);
                self.progress_label.set_visible(false);
                self.download_stack.set_visible_child_name("menu");
                self.add_popover.popdown();

                let rm = self.prefix_manager.runtime_manager().clone();
                populate_runtime_list(&self.list_box, &rm, sender.clone());
                update_count_label(&self.count_label, &rm);
                emit_updated(&self.prefix_manager, &sender);
            }
            RuntimeManagerMsg::DownloadFailed(err) => {
                self.set_downloading(false);
                self.set_download_progress(0.0);
                self.menu_btn.set_sensitive(true);
                self.progress_bar.set_visible(false);
                self.progress_label.set_visible(false);
                self.progress_label.set_label(&format!("Error: {}", err));
                self.download_stack.set_visible_child_name("menu");

                let alert = adw::AlertDialog::new(Some("Download Failed"), Some(&err));
                alert.add_response("ok", "OK");
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.choose(Some(root), None::<&gtk::gio::Cancellable>, |_| {});
            }
            RuntimeManagerMsg::ImportRuntime => {
                self.add_popover.popdown();

                let file_dialog = gtk::FileDialog::builder()
                    .title("Select Wine Installation")
                    .build();
                let s = sender.clone();
                file_dialog.select_folder(Some(root), None::<&gtk::gio::Cancellable>, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            let _ = s.input(RuntimeManagerMsg::ImportFromPath(path));
                        }
                    }
                });
            }
            RuntimeManagerMsg::ImportFromPath(path) => {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("imported")
                    .to_string();

                match self.prefix_manager.import_runtime(&path, &dir_name) {
                    Ok(_runtime) => {
                        self.prefix_manager.save_runtime_state();
                        let rm = self.prefix_manager.runtime_manager().clone();
                        populate_runtime_list(&self.list_box, &rm, sender.clone());
                        update_count_label(&self.count_label, &rm);
                        emit_updated(&self.prefix_manager, &sender);
                    }
                    Err(e) => {
                        eprintln!("Import failed: {}", e);
                        let alert = adw::AlertDialog::new(
                            Some("Import Failed"),
                            Some(&format!("Failed to import Wine runtime:\n{}", e)),
                        );
                        alert.add_response("ok", "OK");
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
    // Menu page
    let menu_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(10).margin_bottom(10)
        .margin_start(10).margin_end(10)
        .build();
    menu_page.append(
        &gtk::Label::builder()
            .label("Add Wine Runtime")
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .margin_bottom(6)
            .build(),
    );

    {
        let s = sender.clone();
        let btn = gtk::Button::builder()
            .label("Download from Homebrew")
            .halign(gtk::Align::Fill)
            .css_classes(["suggested-action"])
            .build();
        btn.connect_clicked(move |_| {
            s.input(RuntimeManagerMsg::ShowAddMenu);
        });
        menu_page.append(&btn);
    }

    {
        let s = sender.clone();
        let btn = gtk::Button::builder()
            .label("Import from Disk")
            .halign(gtk::Align::Fill)
            .build();
        btn.connect_clicked(move |_| {
            s.input(RuntimeManagerMsg::ImportRuntime);
        });
        menu_page.append(&btn);
    }

    // Download page
    let download_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(10).margin_end(10)
        .build();
    download_page.append(
        &gtk::Label::builder()
            .label("Download Wine Runtime")
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );
    download_page.append(
        &gtk::Label::builder()
            .label("Channel:")
            .halign(gtk::Align::Start)
            .build(),
    );

    let channel_combo = gtk::DropDown::from_strings(&[
        "Stable (wine-stable)", "Devel (wine@devel)", "Staging (wine@staging)",
    ]);
    channel_combo.set_hexpand(true);
    channel_combo.set_selected(0);
    download_page.append(&channel_combo);

    let progress_bar = gtk::ProgressBar::builder()
        .visible(false)
        .hexpand(true)
        .build();
    download_page.append(&progress_bar);

    let progress_label = gtk::Label::builder()
        .visible(false)
        .halign(gtk::Align::Center)
        .css_classes(["caption"])
        .build();
    download_page.append(&progress_label);

    let button_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .margin_top(6)
        .build();

    {
        let _stack_ref = gtk::Stack::new(); // placeholder, will find parent
        let back_btn = gtk::Button::with_label("Back");
        back_btn.connect_clicked(move |btn| {
            // Walk up to find the Stack and switch to menu page
            if let Some(stack) = btn
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.downcast::<gtk::Stack>().ok())
            {
                stack.set_visible_child_name("menu");
            }
        });
        button_box.append(&back_btn);
    }

    {
        let s = sender.clone();
        let combo = channel_combo.clone();
        let download_btn = gtk::Button::builder()
            .label("Download")
            .css_classes(["suggested-action"])
            .build();
        download_btn.connect_clicked(move |_| {
            let channel = match combo.selected() {
                0 => Channel::Stable,
                1 => Channel::Devel,
                2 => Channel::Staging,
                _ => Channel::Stable,
            };
            s.input(RuntimeManagerMsg::StartDownload(channel));
        });
        button_box.append(&download_btn);
    }

    download_page.append(&button_box);

    // Stack holding both pages
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    stack.add_named(&menu_page, Some("menu"));
    stack.add_named(&download_page, Some("download"));
    stack.set_visible_child_name("menu");

    let popover = gtk::Popover::new();
    popover.set_width_request(280);
    popover.set_child(Some(&stack));

    popover
}

/// Extract widget references from the download stack page.
fn extract_download_widgets(
    stack: &gtk::Stack,
) -> (gtk::DropDown, gtk::ProgressBar, gtk::Label, gtk::Button) {
    // Get the download page (second child of stack)
    let download_page = stack
        .first_child()
        .and_then(|c| c.next_sibling())
        .and_then(|c| c.downcast::<gtk::Box>().ok())
        .unwrap_or_else(|| gtk::Box::default());

    let mut children = Vec::new();
    let mut child = download_page.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        children.push(c);
    }

    // children[0] = heading label, [1] = "Channel:" label, [2] = channel_combo,
    // [3] = progress_bar, [4] = progress_label, [5] = button_box
    let channel_combo = children
        .get(2)
        .and_then(|c| c.clone().downcast::<gtk::DropDown>().ok())
        .unwrap_or_else(|| gtk::DropDown::from_strings(&["Stable", "Devel", "Staging"]));
    let progress_bar = children
        .get(3)
        .and_then(|c| c.clone().downcast::<gtk::ProgressBar>().ok())
        .unwrap_or_else(|| gtk::ProgressBar::new());
    let progress_label = children
        .get(4)
        .and_then(|c| c.clone().downcast::<gtk::Label>().ok())
        .unwrap_or_else(|| gtk::Label::new(None));

    // button_box children: [0] = back button, [1] = download button
    let download_button = children
        .get(5)
        .and_then(|c| c.clone().downcast::<gtk::Box>().ok())
        .and_then(|b| {
            let mut btn = b.first_child();
            // Skip back button
            if let Some(ref first) = btn {
                btn = first.next_sibling();
            }
            btn
        })
        .and_then(|c| c.downcast::<gtk::Button>().ok())
        .unwrap_or_else(|| gtk::Button::new());

    (channel_combo, progress_bar, progress_label, download_button)
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
            RuntimeSource::ManagedChannel { channel, installed_cask_version } => {
                format!("Homebrew {} — cask {}", channel.display_name(), installed_cask_version)
            }
            RuntimeSource::ManagedVersion { source_url: _ } => {
                "Managed (versioned)".to_string()
            }
            RuntimeSource::Imported { label, original_path } => {
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
            .margin_start(8).margin_end(8)
            .margin_top(4).margin_bottom(4)
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
            .margin_top(4).margin_bottom(4)
            .margin_start(4).margin_end(4)
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
    label.set_label(&format!("{} runtime{}", count, if count == 1 { "" } else { "s" }));
}

fn emit_updated(
    pm: &PrefixManager,
    sender: &AsyncComponentSender<RuntimeManagerModel>,
) {
    let _ = sender.output(RuntimeManagerOutput::RuntimesUpdated(
        pm.runtime_manager().clone(),
    ));
}
