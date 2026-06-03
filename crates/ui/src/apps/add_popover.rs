use gtk::prelude::*;
use prefix::IconCache;
use prefix::config::RegisteredExecutable;
use prefix::resolve_or_extract_icon;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::{
    RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use std::path::PathBuf;
use std::sync::Arc;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AddAppPopoverModel {
    #[tracker::do_not_track]
    available_executables: FactoryVecDeque<AvailableExecutable>,
    available_apps: Vec<RegisteredExecutable>,
    selected_indices: std::collections::HashSet<usize>,
    is_visible: bool,
    is_scanning: bool,
    #[tracker::do_not_track]
    is_processing_selection: bool,
    #[tracker::do_not_track]
    prefix_path: PathBuf,
    #[tracker::do_not_track]
    icon_cache: Arc<IconCache>,
}

#[derive(Debug)]
pub enum AddAppPopoverMsg {
    Show,
    Hide,
    UpdateAvailableApps(Vec<RegisteredExecutable>, String), // exes, prefix_arch
    SelectApp(usize),
    AddSelected,
    Scan,
    ResetProcessingFlag,
    SetScanning(bool),
    PrefixPathUpdated(PathBuf),
}

#[derive(Debug)]
pub enum AddAppPopoverOutput {
    AddApp(Vec<usize>),
    Scan,
    Close,
}

// Factory component for available executables
#[derive(Debug)]
struct AvailableExecutable {
    executable: RegisteredExecutable,
    #[allow(dead_code)]
    index: usize,
    selected: bool,
    arch_label: String,
    resolved_icon: Option<PathBuf>,
}

#[derive(Debug)]
enum AvailableExecutableMsg {
    #[allow(dead_code)]
    Select,
}

#[derive(Debug)]
enum AvailableExecutableOutput {
    Selected(usize),
}

#[relm4::factory]
impl FactoryComponent for AvailableExecutable {
    type Init = (RegisteredExecutable, usize, String, Option<PathBuf>); // exe, index, arch_label, resolved_icon
    type Input = AvailableExecutableMsg;
    type Output = AvailableExecutableOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        gtk::ListBoxRow {
            set_selectable: false,
            set_activatable: true,

            #[watch]
            set_css_classes: if self.selected { &["activatable", "selected-row"] } else { &["activatable"] },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                set_margin_top: 8,
                set_margin_bottom: 8,
                set_margin_start: 8,
                set_margin_end: 8,

                // Checkbox for selection
                gtk::CheckButton {
                    #[watch]
                    set_active: self.selected,
                    connect_toggled[sender, index] => move |_check| {
                        // Always send the output - we'll handle deduplication at the parent level
                        let _ = sender.output(AvailableExecutableOutput::Selected(index.current_index()));
                    },
                },

                // Icon or fallback
                gtk::Box {
                    set_width_request: 24,
                    set_height_request: 24,
                    add_css_class: "icon-bg",

                    gtk::Image {
                        set_pixel_size: 24,
                        #[watch]
                        set_from_file: self.resolved_icon.as_deref(),
                        #[watch]
                        set_visible: self.resolved_icon.is_some(),
                    },
                    gtk::Image {
                        set_pixel_size: 24,
                        set_icon_name: Some("application-x-executable"),
                        #[watch]
                        set_visible: self.resolved_icon.is_none(),
                    },
                },

                // Executable info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 2,
                    set_hexpand: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,

                        gtk::Label {
                            #[watch]
                            set_label: &self.executable.name,
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &self.arch_label,
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &self.executable.description.as_deref().unwrap_or(""),
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        add_css_class: "caption",
                        #[watch]
                        set_visible: self.executable.description.is_some(),
                    },
                },
            }
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let (executable, index, arch_label, resolved_icon) = init;
        Self {
            executable,
            index,
            selected: false,
            arch_label,
            resolved_icon,
        }
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            AvailableExecutableMsg::Select => {
                self.selected = true;
            }
        }
    }
}

#[relm4::component(pub, async)]
impl AsyncComponent for AddAppPopoverModel {
    type Init = (gtk::Button, PathBuf, Arc<IconCache>);
    type Input = AddAppPopoverMsg;
    type Output = AddAppPopoverOutput;
    type CommandOutput = ();
    type Widgets = AddAppPopoverWidgets;

    view! {
        #[name = "popover"]
        gtk::Popover {
            #[watch]
            set_visible: model.is_visible,
            connect_closed[sender] => move |_| {
                sender.input(AddAppPopoverMsg::Hide);
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,
                set_width_request: 400,
                set_height_request: 300,

                gtk::Label {
                    set_label: "Available Applications",
                    add_css_class: "heading",
                    set_halign: gtk::Align::Center,
                    set_margin_bottom: 10,
                },

                // Conditional: spinner during scan, list otherwise — no layout shift
                #[transition = "Crossfade"]
                match model.is_scanning {
                    true => {
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 10,
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_vexpand: true,
                            set_height_request: 200,

                            gtk::Spinner {
                                set_spinning: true,
                                set_halign: gtk::Align::Center,
                            },
                            gtk::Label {
                                set_label: "Scanning for applications...",
                                set_halign: gtk::Align::Center,
                                add_css_class: "dim-label",
                            },
                        }
                    }
                    false => {
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 10,
                            set_vexpand: true,

                            gtk::ScrolledWindow {
                                #[watch]
                                set_visible: model.available_apps.len() > 0,
                                set_vexpand: true,
                                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                                set_min_content_height: 200,

                                #[local_ref]
                                available_list_box -> gtk::ListBox {
                                    set_css_classes: &["boxed-list"],
                                    set_selection_mode: gtk::SelectionMode::None,
                                },
                            },

                            gtk::Label {
                                set_label: "No available applications found\nScan for applications first",
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Center,
                                set_wrap: true,
                                #[watch]
                                set_visible: model.available_apps.len() == 0,
                                add_css_class: "dim-label",
                            },

                            gtk::Label {
                                #[watch]
                                set_label: &format!("{} applications found", model.available_apps.len()),
                                add_css_class: "caption",
                                set_halign: gtk::Align::Center,
                                #[watch]
                                set_visible: model.available_apps.len() > 0,
                            },
                        }
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_margin_top: 10,

                    gtk::Button {
                        #[watch]
                        set_label: if model.is_scanning { "Scanning..." } else { "Scan" },
                        #[watch]
                        set_sensitive: !model.is_scanning,
                        set_tooltip_text: Some("Scan prefix for executables"),
                        connect_clicked[sender] => move |_| {
                            sender.input(AddAppPopoverMsg::Scan);
                        },
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    gtk::Button {
                        set_label: "Cancel",
                        connect_clicked[sender] => move |_| {
                            sender.input(AddAppPopoverMsg::Hide);
                        },
                    },

                    gtk::Button {
                        set_label: "Add",
                        #[watch]
                        set_sensitive: !model.selected_indices.is_empty(),
                        add_css_class: "suggested-action",
                        connect_clicked[sender] => move |_| {
                            sender.input(AddAppPopoverMsg::AddSelected);
                        },
                    },
                },
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (_button, prefix_path, icon_cache) = init;

        // Initialize factory for available executables
        let available_executables = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                AvailableExecutableOutput::Selected(index) => AddAppPopoverMsg::SelectApp(index),
            });

        let model = AddAppPopoverModel {
            available_executables,
            available_apps: Vec::new(),
            selected_indices: std::collections::HashSet::new(),
            is_visible: false,
            is_scanning: false,
            is_processing_selection: false,
            prefix_path,
            icon_cache,
            tracker: 0,
        };

        // Get references to the factory widgets
        let available_list_box = model.available_executables.widget();

        let widgets = view_output!();

        // Initialize the popover with available apps if any
        sender.input(AddAppPopoverMsg::UpdateAvailableApps(
            Vec::new(),
            "win64".to_string(),
        ));

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        widgets: &gtk::Popover,
    ) {
        self.reset();
        match msg {
            AddAppPopoverMsg::Show => {
                self.set_is_visible(true);
                // Make sure the popover is properly realized before popping up
                if !widgets.is_visible() {
                    self.set_selected_indices(std::collections::HashSet::new());
                    widgets.popup();
                }
            }
            AddAppPopoverMsg::Hide => {
                self.set_is_visible(false);
                widgets.popdown();
                let _ = sender.output(AddAppPopoverOutput::Close);
            }
            AddAppPopoverMsg::UpdateAvailableApps(apps, prefix_arch) => {
                self.available_apps = apps.clone();
                self.selected_indices.clear();
                self.set_selected_indices(self.selected_indices.clone());

                // Compute arch label for each executable
                let arch_labels: Vec<String> = apps
                    .iter()
                    .map(|exe| compute_arch_label(&exe.executable_path, &prefix_arch))
                    .collect();

                // Resolve (or extract) icons for display
                let prefix_path = self.prefix_path.clone();
                let icon_cache = Arc::clone(&self.icon_cache);
                let resolved_icons: Vec<Option<PathBuf>> = apps
                    .iter()
                    .map(|exe| resolve_or_extract_icon(exe, &prefix_path, &icon_cache))
                    .collect();

                // Update factory
                {
                    let mut guard = self.available_executables.guard();
                    guard.clear();
                    for (index, executable) in apps.iter().enumerate() {
                        guard.push_back((
                            executable.clone(),
                            index,
                            arch_labels[index].clone(),
                            resolved_icons[index].clone(),
                        ));
                    }
                }
            }
            AddAppPopoverMsg::SelectApp(index) => {
                // println!("DEBUG: SelectApp called with index: {}", index);

                // Prevent recursive calls
                if self.is_processing_selection {
                    // println!("DEBUG: Skipping recursive SelectApp call");
                    return;
                }

                // Check if this index is actually different from current state to prevent loops
                let currently_selected = self.selected_indices.contains(&index);

                // Toggle selection for the clicked index
                if currently_selected {
                    self.selected_indices.remove(&index);
                    log::debug!("[apps] deselected index: {}", index);
                } else {
                    self.selected_indices.insert(index);
                    log::debug!("[apps] selected index: {}", index);
                }

                // Set flag to prevent recursive calls
                self.is_processing_selection = true;

                // Update the factory to reflect the new selection state
                {
                    let mut guard = self.available_executables.guard();
                    for (idx, item) in guard.iter_mut().enumerate() {
                        let new_selected = self.selected_indices.contains(&idx);
                        if item.selected != new_selected {
                            item.selected = new_selected;
                        }
                    }
                }

                // Notify tracker that selected_indices has changed AFTER updating factory
                // This ensures UI updates properly
                self.set_selected_indices(self.selected_indices.clone());

                // Reset flag after a short delay to allow UI to update
                let sender = sender.clone();
                relm4::spawn(async move {
                    relm4::tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    sender.input(AddAppPopoverMsg::ResetProcessingFlag);
                });

                log::debug!(
                    "[apps] current selected indices: {:?}",
                    self.selected_indices
                );
            }
            AddAppPopoverMsg::Scan => {
                let _ = sender.output(AddAppPopoverOutput::Scan);
            }
            AddAppPopoverMsg::SetScanning(scanning) => {
                self.set_is_scanning(scanning);
            }
            AddAppPopoverMsg::ResetProcessingFlag => {
                self.is_processing_selection = false;
                log::debug!("[apps] reset processing flag");
            }
            AddAppPopoverMsg::PrefixPathUpdated(prefix_path) => {
                if self.prefix_path == prefix_path {
                    return;
                }
                self.prefix_path = prefix_path;

                // Re-resolve icons with the new prefix location
                let prefix_path = self.prefix_path.clone();
                let icon_cache = Arc::clone(&self.icon_cache);
                let mut guard = self.available_executables.guard();
                for item in guard.iter_mut() {
                    item.resolved_icon =
                        resolve_or_extract_icon(&item.executable, &prefix_path, &icon_cache);
                }
            }
            AddAppPopoverMsg::AddSelected => {
                if !self.selected_indices.is_empty() {
                    let selected_vec: Vec<usize> = self.selected_indices.iter().copied().collect();

                    // Clear factory state BEFORE closing popover (avoids SIGSEGV on destroyed widgets)
                    {
                        let mut guard = self.available_executables.guard();
                        for item in guard.iter_mut() {
                            item.selected = false;
                        }
                    }
                    self.selected_indices.clear();
                    self.set_selected_indices(self.selected_indices.clone());
                    self.is_processing_selection = true;

                    // Emit add and close
                    let _ = sender.output(AddAppPopoverOutput::AddApp(selected_vec));
                    self.set_is_visible(false);
                    widgets.popdown();

                    let sender = sender.clone();
                    relm4::spawn(async move {
                        relm4::tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        sender.input(AddAppPopoverMsg::ResetProcessingFlag);
                    });
                }
            }
        }
    }
}

/// Determine x86/x64 label for an executable based on its path and prefix architecture.
fn compute_arch_label(path: &std::path::Path, prefix_arch: &str) -> String {
    let path_lower = path.to_string_lossy().to_lowercase();
    if path_lower.contains("program files (x86)") {
        "x86".to_string()
    } else if path_lower.contains("program files") && prefix_arch == "win64" {
        "x64".to_string()
    } else if prefix_arch == "win64" {
        "x64".to_string()
    } else {
        "x86".to_string()
    }
}
