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
    selected_index: Option<usize>,
    is_visible: bool,
}

#[derive(Debug)]
pub enum AddAppPopoverMsg {
    Show,
    Hide,
    UpdateAvailableApps(Vec<RegisteredExecutable>),
    SelectApp(usize),
    AddSelected,
}

#[derive(Debug)]
pub enum AddAppPopoverOutput {
    AddApp(usize),
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
            set_selectable: true,
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

                // Icon or placeholder
                gtk::Image {
                    set_pixel_size: 24,
                    #[watch]
                    set_from_file: self.executable.icon_path.as_ref(),
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
                        set_selection_mode: gtk::SelectionMode::Single,
                        connect_row_selected[sender] => move |_, row| {
                            
                            if let Some(row) = row {
                                
                                let index = row.index();
                                println!("Row selected: {:?}", index);
                                sender.input(AddAppPopoverMsg::SelectApp(index as usize));
                            }
                        },
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
                        #[track = "model.changed(AddAppPopoverModel::selected_index())"]
                        set_sensitive: model.selected_index.is_some(),
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
            selected_index: None,
            is_visible: false,
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
                self.set_selected_index(Some(index));
                
                // Update selection state in factory without clearing all items
                {
                    let mut guard = self.available_executables.guard();
                    for (idx, item) in guard.iter_mut().enumerate() {
                        item.selected = (idx == index);
                    }
                }
            }
            AddAppPopoverMsg::AddSelected => {
                if let Some(index) = self.selected_index {
                    let _ = sender.output(AddAppPopoverOutput::AddApp(index));
                    self.set_is_visible(false);
                    widgets.popdown();
                }
            }
        }
    }
}