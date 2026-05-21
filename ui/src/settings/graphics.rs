use adw::prelude::*;
use relm4::prelude::*;
use tracker;
use prefix::runtime;

// ── Model ────────────────────────────────────────────────────────────────

#[tracker::track]
pub struct GraphicsSettings {
    #[tracker::do_not_track]
    installed_group: adw::PreferencesGroup,
    #[tracker::do_not_track]
    rows: Vec<adw::ActionRow>,
}

// ── Messages ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GraphicsSettingsMsg {
    InstallBackend(String),
}

#[derive(Debug)]
pub enum GraphicsSettingsOutput {
    Changed,
}

// ── Component ────────────────────────────────────────────────────────────

#[relm4::component(pub, async)]
impl AsyncComponent for GraphicsSettings {
    type Init = ();
    type Input = GraphicsSettingsMsg;
    type Output = GraphicsSettingsOutput;
    type CommandOutput = ();
    type Widgets = GraphicsSettingsWidgets;

    view! {
        adw::NavigationPage {
            set_title: "Graphics Backends",
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let prefs_page = adw::PreferencesPage::new();

        // Installed group
        let installed_group = adw::PreferencesGroup::builder()
            .title("Installed")
            .build();
        let mut rows: Vec<adw::ActionRow> = Vec::new();
        refresh_graphics_list(&installed_group, &mut rows);
        prefs_page.add(&installed_group);

        // Available group
        let available_group = adw::PreferencesGroup::builder()
            .title("Available")
            .description("Translation layers that can improve DirectX performance")
            .build();
        build_available_graphics_rows(&available_group, &sender);
        prefs_page.add(&available_group);

        root.set_child(Some(&prefs_page));

        let widgets = view_output!();

        let model = GraphicsSettings {
            installed_group,
            rows,
            tracker: 0,
        };

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            GraphicsSettingsMsg::InstallBackend(_name) => {
                // TODO: wire up graphics backend download
                // Use runtime::graphics::fetch_dxmt_release() etc. once implemented
            }
        }
    }
}

// ── Graphics list helpers ────────────────────────────────────────────────

fn refresh_graphics_list(group: &adw::PreferencesGroup, rows: &mut Vec<adw::ActionRow>) {
    for row in rows.drain(..) {
        group.remove(&row);
    }

    let dir = runtime::graphics::graphics_dir();
    if !dir.is_dir() {
        let row = adw::ActionRow::builder()
            .title("No backends installed")
            .subtitle("Download graphics backends to improve DirectX performance")
            .activatable(false)
            .build();
        group.add(&row);
        rows.push(row);
        return;
    }

    let mut found = false;
    for entry in std::fs::read_dir(&dir).ok().into_iter().flatten() {
        let entry = match entry { Ok(e) => e, _ => continue };
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) { continue; }
        found = true;
        let name = entry.file_name().to_string_lossy().to_string();

        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(&format!("Installed in {}", entry.path().display()))
            .activatable(false)
            .build();

        let remove_btn = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove backend")
            .css_classes(["flat", "circular", "destructive-action"])
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&remove_btn);

        group.add(&row);
        rows.push(row);
    }

    if !found {
        let row = adw::ActionRow::builder()
            .title("No backends installed")
            .subtitle("Download from the Available section below")
            .activatable(false)
            .build();
        group.add(&row);
        rows.push(row);
    }
}

fn build_available_graphics_rows(
    group: &adw::PreferencesGroup,
    sender: &AsyncComponentSender<GraphicsSettings>,
) {
    #[cfg(target_os = "macos")]
    {
        let dxmt_row = adw::ActionRow::builder()
            .title("DXMT")
            .subtitle("DirectX → Metal translation layer (recommended)")
            .activatable(false)
            .build();
        let install_dxmt = gtk::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Install DXMT")
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .build();
        {
            let s = sender.clone();
            install_dxmt.connect_clicked(move |_| {
                s.input(GraphicsSettingsMsg::InstallBackend("dxmt".to_string()));
            });
        }
        dxmt_row.add_suffix(&install_dxmt);
        group.add(&dxmt_row);

        let d3d_row = adw::ActionRow::builder()
            .title("D3DMetal (via GPTK)")
            .subtitle("Apple's Game Porting Toolkit")
            .activatable(false)
            .build();
        let install_d3d = gtk::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Install D3DMetal")
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .build();
        d3d_row.add_suffix(&install_d3d);
        group.add(&d3d_row);
    }

    #[cfg(target_os = "linux")]
    {
        let dxvk_row = adw::ActionRow::builder()
            .title("DXVK + VKD3D")
            .subtitle("DirectX → Vulkan translation layers")
            .activatable(false)
            .build();
        let install_dxvk = gtk::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Install DXVK + VKD3D")
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .build();
        dxvk_row.add_suffix(&install_dxvk);
        group.add(&dxvk_row);
    }
}
