use relm4::{
    gtk, RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
};
use relm4::factory::{FactoryComponent, FactorySender, FactoryVecDeque, DynamicIndex};
use gtk::prelude::*;
use crate::prefix::config::RegisteredExecutable;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AddAppPopoverModel {
    #[tracker::do_not_track]
    available_executables: FactoryVecDeque<AvailableExecutable>,
    available_apps: Vec<RegisteredExecutable>,
    selected_indices: std::collections::HashSet<usize>,
    is_visible: bool,
    #[tracker::do_not_track]
    is_processing_selection: bool,
}

#[derive(Debug)]
pub enum AddAppPopoverMsg {
    Show,
    Hide,
    UpdateAvailableApps(Vec<RegisteredExecutable>),
    SelectApp(usize),
    AddSelected,
    ResetProcessingFlag,
}

#[derive(Debug)]
pub enum AddAppPopoverOutput {
    AddApp(Vec<usize>),
    Close,
}

// Factory component for available executables
#[derive(Debug)]
struct AvailableExecutable {
    executable: RegisteredExecutable,
    index: usize,
    selected: bool,
}

#[derive(Debug)]
enum AvailableExecutableMsg {
    Select,
}

#[derive(Debug)]
enum AvailableExecutableOutput {
    Selected(usize),
}

#[relm4::factory]
impl FactoryComponent for AvailableExecutable {
    type Init = (RegisteredExecutable, usize);
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
                    connect_toggled[sender, index] => move |check| {
                        // Always send the output - we'll handle deduplication at the parent level
                        sender.output(AvailableExecutableOutput::Selected(index.current_index()));
                    },
                },

                // Icon or placeholder
                gtk::Image {
                    set_pixel_size: 24,
                    // #[watch]
                    // set_from_file: self.executable.icon_path.as_ref(),
                    set_icon_name: Some("application-x-executable"),
                },

                // Executable info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 2,
                    set_hexpand: true,

                    gtk::Label {
                        #[watch]
                        set_label: &self.executable.name,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
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

    fn init_model(
        init: Self::Init,
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let (executable, index) = init;
        Self {
            executable,
            index,
            selected: false,
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
    type Init = gtk::Button;
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

                gtk::ScrolledWindow {
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
                    #[watch]
                    set_label: &format!("{} applications found", model.available_apps.len()),
                    add_css_class: "caption",
                    set_halign: gtk::Align::Center,
                    #[watch]
                    set_visible: model.available_apps.len() > 0,
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

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_halign: gtk::Align::End,
                    set_margin_top: 10,

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
            is_processing_selection: false,
            tracker: 0
        };

        // Get references to the factory widgets
        let available_list_box = model.available_executables.widget();

        let widgets = view_output!();

        // Initialize the popover with available apps if any
        sender.input(AddAppPopoverMsg::UpdateAvailableApps(Vec::new()));

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
            AddAppPopoverMsg::UpdateAvailableApps(apps) => {
                self.available_apps = apps.clone();
                
                // Update factory
                {
                    let mut guard = self.available_executables.guard();
                    guard.clear();
                    for (index, executable) in apps.iter().enumerate() {
                        guard.push_back((executable.clone(), index));
                    }
                }
            }
            AddAppPopoverMsg::SelectApp(index) => {
                println!("DEBUG: SelectApp called with index: {}", index);
                
                // Prevent recursive calls
                if self.is_processing_selection {
                    println!("DEBUG: Skipping recursive SelectApp call");
                    return;
                }
                
                // Check if this index is actually different from current state to prevent loops
                let currently_selected = self.selected_indices.contains(&index);
                
                // Toggle selection for the clicked index
                if currently_selected {
                    self.selected_indices.remove(&index);
                    println!("DEBUG: Deselected index: {}", index);
                } else {
                    self.selected_indices.insert(index);
                    println!("DEBUG: Selected index: {}", index);
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
                
                println!("DEBUG: Current selected indices: {:?}", self.selected_indices);
            }
            AddAppPopoverMsg::ResetProcessingFlag => {
                self.is_processing_selection = false;
                println!("DEBUG: Reset processing flag");
            }
            AddAppPopoverMsg::AddSelected => {
                if !self.selected_indices.is_empty() {
                    let selected_vec: Vec<usize> = self.selected_indices.iter().copied().collect();
                    let _ = sender.output(AddAppPopoverOutput::AddApp(selected_vec));
                    self.set_is_visible(false);
                    widgets.popdown();
                    
                    // Clear selection after adding
                    self.selected_indices.clear();
                    
                    // Notify tracker that selected_indices has changed
                    self.set_selected_indices(self.selected_indices.clone());
                    
                    // Set flag to prevent recursive calls during cleanup
                    self.is_processing_selection = true;
                    
                    // Update selection state in factory without rebuilding
                    {
                        let mut guard = self.available_executables.guard();
                        for item in guard.iter_mut() {
                            item.selected = false;
                        }
                    }
                    
                    // Reset flag after a short delay
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