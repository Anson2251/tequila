use adw::prelude::*;
use relm4::prelude::*;
use tracker;
use std::path::PathBuf;
use prefix::{
    Manager as PrefixManager,
    runtime::{self, RuntimeManager, Channel, RuntimeSource},
    GraphicsBackend,
};

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct RuntimeSettings {
    pub prefix_manager: PrefixManager,
    parent: gtk::Window,

    downloading: bool,
    download_progress: f64,

    #[tracker::do_not_track]
    progress_bar: gtk::ProgressBar,
    #[tracker::do_not_track]
    progress_label: gtk::Label,
    #[tracker::do_not_track]
    add_menu: gtk::Popover,
    #[tracker::do_not_track]
    channel_combo: gtk::ComboBoxText,
    #[tracker::do_not_track]
    list_group: adw::PreferencesGroup,
    #[tracker::do_not_track]
    rows: Vec<adw::ActionRow>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum RuntimeSettingsMsg {
    RefreshRuntimes,
    SetDefault(String),
    RemoveRuntime(String),
    ShowAddMenu,
    StartDownload(Channel),
    DownloadProgress(u64, u64),
    DownloadComplete(RuntimeManager),
    DownloadFailed(String),
    ImportRuntime,
    ImportFromPath(PathBuf),
}

#[derive(Debug)]
pub enum RuntimeSettingsOutput {
    RuntimesUpdated(RuntimeManager),
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for RuntimeSettings {
    type Init = (PrefixManager, gtk::Window);
    type Input = RuntimeSettingsMsg;
    type Output = RuntimeSettingsOutput;
    type CommandOutput = ();
    type Widgets = RuntimeSettingsWidgets;

    view! {
        adw::NavigationPage {
            set_title: "Wine Runtime",
        }
    }

    async fn init(
        (prefix_manager, parent): Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let prefs_page = adw::PreferencesPage::new();

        // ── Installed runtimes group ──
        let list_group = adw::PreferencesGroup::builder()
            .title("Installed Runtimes")
            .build();
        let mut rows: Vec<adw::ActionRow> = Vec::new();
        refresh_runtime_list(&list_group, prefix_manager.runtime_manager(), &sender, &mut rows);
        prefs_page.add(&list_group);

        // ── Add runtime group ──
        let add_group = adw::PreferencesGroup::builder()
            .title("Add Runtime")
            .build();

        let (add_menu, channel_combo, progress_bar, progress_label) =
            build_download_popover(&sender);

        let download_row = adw::ActionRow::builder()
            .title("Download from Homebrew")
            .subtitle("Stable, Devel, or Staging channel")
            .activatable(true)
            .build();
        {
            let s = sender.clone();
            download_row.connect_activated(move |_| {
                s.input(RuntimeSettingsMsg::ShowAddMenu);
            });
        }

        let import_row = adw::ActionRow::builder()
            .title("Import from Disk")
            .subtitle("Select an existing Wine installation folder")
            .activatable(true)
            .build();
        {
            let s = sender.clone();
            import_row.connect_activated(move |_| {
                s.input(RuntimeSettingsMsg::ImportRuntime);
            });
        }

        let menu_btn = gtk::MenuButton::builder()
            .icon_name("go-down-symbolic")
            .css_classes(["flat"])
            .popover(&add_menu)
            .build();
        download_row.add_suffix(&menu_btn);

        add_group.add(&download_row);
        add_group.add(&import_row);
        prefs_page.add(&add_group);

        root.set_child(Some(&prefs_page));

        let widgets = view_output!();

        let model = RuntimeSettings {
            prefix_manager,
            parent,
            downloading: false,
            download_progress: 0.0,
            progress_bar,
            progress_label,
            add_menu,
            channel_combo,
            list_group,
            rows,
            tracker: 0,
        };

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.reset();
        match msg {
            RuntimeSettingsMsg::RefreshRuntimes => {
                refresh_runtime_list(&self.list_group, self.prefix_manager.runtime_manager(), &sender, &mut self.rows);
            }
            RuntimeSettingsMsg::SetDefault(id) => {
                self.prefix_manager.set_default_runtime(&id);
                self.prefix_manager.save_runtime_state();
                refresh_runtime_list(&self.list_group, self.prefix_manager.runtime_manager(), &sender, &mut self.rows);
                emit_runtimes_updated(&self.prefix_manager, &sender);
            }
            RuntimeSettingsMsg::RemoveRuntime(id) => {
                if id != "wine-system" {
                    self.prefix_manager.remove_runtime(&id);
                    refresh_runtime_list(&self.list_group, self.prefix_manager.runtime_manager(), &sender, &mut self.rows);
                    emit_runtimes_updated(&self.prefix_manager, &sender);
                }
            }
            RuntimeSettingsMsg::ShowAddMenu => {
                self.add_menu.popup();
            }
            RuntimeSettingsMsg::StartDownload(channel) => {
                self.set_downloading(true);
                self.progress_bar.set_visible(true);
                self.progress_label.set_visible(true);
                self.progress_label.set_label("Starting download...");
                self.progress_bar.set_fraction(0.0);
                self.add_menu.popdown();

                let mut pm = self.prefix_manager.clone();
                let s = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let progress: runtime::download::ProgressFn = Box::new({
                        let s = s.clone();
                        move |downloaded, total| {
                            let _ = s.input(RuntimeSettingsMsg::DownloadProgress(downloaded, total));
                        }
                    });
                    match pm.download_channel_runtime(channel, progress).await {
                        Ok(_runtime) => {
                            let rm = pm.runtime_manager().clone();
                            let _ = s.input(RuntimeSettingsMsg::DownloadComplete(rm));
                        }
                        Err(e) => {
                            let _ = s.input(RuntimeSettingsMsg::DownloadFailed(e.to_string()));
                        }
                    }
                });
            }
            RuntimeSettingsMsg::DownloadProgress(downloaded, total) => {
                if total > 0 {
                    let frac = downloaded as f64 / total as f64;
                    self.set_download_progress(frac);
                    self.progress_bar.set_fraction(frac);
                    let mb = |b: u64| b as f64 / 1_048_576.0;
                    self.progress_label.set_label(&format!(
                        "{:.1} / {:.1} MB", mb(downloaded), mb(total)
                    ));
                }
            }
            RuntimeSettingsMsg::DownloadComplete(updated_rm) => {
                let rm_ref = self.prefix_manager.runtime_manager_mut();
                let _old = std::mem::replace(rm_ref, updated_rm);
                self.prefix_manager.save_runtime_state();

                self.set_downloading(false);
                self.set_download_progress(0.0);
                self.progress_bar.set_visible(false);
                self.progress_label.set_visible(false);

                refresh_runtime_list(&self.list_group, self.prefix_manager.runtime_manager(), &sender, &mut self.rows);
                emit_runtimes_updated(&self.prefix_manager, &sender);
            }
            RuntimeSettingsMsg::DownloadFailed(err) => {
                self.set_downloading(false);
                self.set_download_progress(0.0);
                self.progress_bar.set_visible(false);
                self.progress_label.set_visible(false);

                let alert = adw::AlertDialog::new(Some("Download Failed"), Some(&err));
                alert.add_response("ok", "OK");
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.choose(Some(&self.parent), None::<&gtk::gio::Cancellable>, |_| {});
            }
            RuntimeSettingsMsg::ImportRuntime => {
                #[cfg(target_os = "macos")]
                macos_import_dialog(&sender);
                #[cfg(not(target_os = "macos"))]
                {
                    let dialog = gtk::FileChooserDialog::builder()
                        .title("Select Wine Installation")
                        .action(gtk::FileChooserAction::SelectFolder)
                        .modal(true).build();
                    dialog.set_transient_for(Some(&self.parent));
                    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
                    dialog.add_button("Select", gtk::ResponseType::Accept);

                    let s = sender.clone();
                    dialog.connect_response(move |dlg, response| {
                        if response == gtk::ResponseType::Accept {
                            if let Some(path) = dlg.file().and_then(|f| f.path()) {
                                let _ = s.input(RuntimeSettingsMsg::ImportFromPath(path));
                            }
                        }
                        dlg.close();
                    });
                    dialog.present();
                }
            }
            RuntimeSettingsMsg::ImportFromPath(path) => {
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str()).unwrap_or("imported").to_string();
                match self.prefix_manager.import_runtime(&path, &dir_name) {
                    Ok(_runtime) => {
                        self.prefix_manager.save_runtime_state();
                        refresh_runtime_list(&self.list_group, self.prefix_manager.runtime_manager(), &sender, &mut self.rows);
                        emit_runtimes_updated(&self.prefix_manager, &sender);
                    }
                    Err(e) => {
                        let alert = adw::AlertDialog::new(
                            Some("Import Failed"),
                            Some(&format!("Failed to import Wine runtime:\n{}", e)),
                        );
                        alert.add_response("ok", "OK");
                        alert.set_default_response(Some("ok"));
                        alert.set_close_response("ok");
                        alert.choose(Some(&self.parent), None::<&gtk::gio::Cancellable>, |_| {});
                    }
                }
            }
        }
    }
}

// ── Popover builder ──────────────────────────────────────────────────────

fn build_download_popover(
    sender: &AsyncComponentSender<RuntimeSettings>,
) -> (gtk::Popover, gtk::ComboBoxText, gtk::ProgressBar, gtk::Label) {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(10).margin_end(10)
        .width_request(280)
        .build();

    content.append(
        &gtk::Label::builder()
            .label("Download Wine Runtime")
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label("Channel:")
            .halign(gtk::Align::Start)
            .build(),
    );

    let channel_combo = gtk::ComboBoxText::builder()
        .hexpand(true)
        .build();
    channel_combo.append_text("Stable (wine-stable)");
    channel_combo.append_text("Devel (wine@devel)");
    channel_combo.append_text("Staging (wine@staging)");
    channel_combo.set_active(Some(0));
    content.append(&channel_combo);

    let progress_bar = gtk::ProgressBar::builder()
        .visible(false)
        .hexpand(true)
        .build();
    content.append(&progress_bar);

    let progress_label = gtk::Label::builder()
        .visible(false)
        .halign(gtk::Align::Center)
        .css_classes(["caption"])
        .build();
    content.append(&progress_label);

    let download_btn = gtk::Button::builder()
        .label("Download")
        .css_classes(["suggested-action"])
        .halign(gtk::Align::End)
        .margin_top(6)
        .build();
    {
        let combo = channel_combo.clone();
        let s = sender.clone();
        download_btn.connect_clicked(move |_| {
            let channel = match combo.active() {
                Some(0) => Channel::Stable,
                Some(1) => Channel::Devel,
                Some(2) => Channel::Staging,
                _ => Channel::Stable,
            };
            s.input(RuntimeSettingsMsg::StartDownload(channel));
        });
    }
    content.append(&download_btn);

    let popover = gtk::Popover::new();
    popover.set_child(Some(&content));

    (popover, channel_combo, progress_bar, progress_label)
}

// ── Runtime list helpers ─────────────────────────────────────────────────

fn refresh_runtime_list(
    group: &adw::PreferencesGroup,
    rm: &RuntimeManager,
    sender: &AsyncComponentSender<RuntimeSettings>,
    rows: &mut Vec<adw::ActionRow>,
) {
    for row in rows.drain(..) {
        group.remove(&row);
    }

    for runtime in &rm.runtimes {
        let is_default = runtime.id == rm.default_id;
        let is_system = matches!(runtime.source, RuntimeSource::System);

        let source = match &runtime.source {
            RuntimeSource::System => "System (PATH)".to_string(),
            RuntimeSource::ManagedChannel { channel, installed_cask_version } => {
                format!("Homebrew {} — cask {}", channel.display_name(), installed_cask_version)
            }
            RuntimeSource::ManagedVersion { source_url: _ } => "Managed (versioned)".to_string(),
            RuntimeSource::Imported { label, original_path } => {
                format!("Imported: {} ({})", label, original_path.display())
            }
        };

        let mut subtitle = format!("{} · Installed {}", runtime.wine_version, &runtime.installed_at[..10]);

        let gfx_names: Vec<&str> = runtime.graphics.iter().map(|g| match g {
            GraphicsBackend::Dxmt { .. } => "DXMT",
            GraphicsBackend::D3DMetal { .. } => "D3DMetal",
            GraphicsBackend::DxvkVkd3d { .. } => "DXVK+VKD3D",
        }).collect();
        if !gfx_names.is_empty() {
            subtitle.push_str(&format!(" · {}", gfx_names.join(", ")));
        }

        let row = adw::ActionRow::builder()
            .title(&runtime.name)
            .subtitle(&subtitle)
            .activatable(true)
            .build();

        let radio = gtk::CheckButton::builder()
            .css_classes(["selection-mode"])
            .active(is_default)
            .valign(gtk::Align::Center)
            .build();
        row.add_prefix(&radio);
        {
            let id = runtime.id.clone();
            let s = sender.clone();
            radio.connect_toggled(move |r| {
                if r.is_active() {
                    s.input(RuntimeSettingsMsg::SetDefault(id.clone()));
                }
            });
        }

        if is_default {
            let badge = gtk::Label::builder()
                .label("Default")
                .css_classes(["badge", "accent"])
                .valign(gtk::Align::Center)
                .margin_end(4)
                .build();
            row.add_suffix(&badge);
        }

        if !is_system {
            let id = runtime.id.clone();
            let s = sender.clone();
            let remove_btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove Runtime")
                .css_classes(["flat", "circular", "destructive-action"])
                .valign(gtk::Align::Center)
                .build();
            remove_btn.connect_clicked(move |_| {
                s.input(RuntimeSettingsMsg::RemoveRuntime(id.clone()));
            });
            row.add_suffix(&remove_btn);
        }

        group.add(&row);
        rows.push(row);
    }
}

fn emit_runtimes_updated(
    pm: &PrefixManager,
    sender: &AsyncComponentSender<RuntimeSettings>,
) {
    let _ = sender.output(RuntimeSettingsOutput::RuntimesUpdated(
        pm.runtime_manager().clone(),
    ));
}

// ── Native file dialog (macOS) ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_import_dialog(sender: &AsyncComponentSender<RuntimeSettings>) {
    use objc2::MainThreadMarker;
    use objc2_foundation::NSString;
    use objc2_app_kit::{NSOpenPanel, NSModalResponse, NSModalResponseOK};
    use block2::RcBlock;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&NSString::from_str("Select Wine Installation")));

    let panel_for_block = panel.clone();
    let s = sender.clone();
    let block = RcBlock::new(move |result: NSModalResponse| {
        if result == NSModalResponseOK {
            let urls = panel_for_block.URLs();
            if let Some(url) = urls.firstObject() {
                if let Some(path_str) = url.path() {
                    let path: String = path_str.to_string();
                    let _ = s.input(RuntimeSettingsMsg::ImportFromPath(PathBuf::from(path)));
                }
            }
        }
    });

    panel.beginWithCompletionHandler(&block);
}
