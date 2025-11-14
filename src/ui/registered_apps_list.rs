use relm4::{
    gtk, ComponentParts, ComponentSender, RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    view
};
use relm4::factory::{FactoryComponent, FactorySender, FactoryVecDeque, DynamicIndex};
use gtk::prelude::*;
use crate::prefix::config::RegisteredExecutable;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct RegisteredAppsListModel {
    #[tracker::do_not_track]
    executables: FactoryVecDeque<RegisteredExecutableItem>,
    registered_executables: Vec<RegisteredExecutable>,
    selected_index: Option<usize>,
}

#[derive(Debug)]
pub enum RegisteredAppsListMsg {
    UpdateExecutables(Vec<RegisteredExecutable>),
    SelectExecutable(usize),
    ClearSelection,
}

#[derive(Debug)]
pub enum RegisteredAppsListOutput {
    Selected(usize),
    Launch(usize),
    Remove(usize),
    ShowInfo(usize),
}

// Grid-based factory component for registered executables
#[derive(Debug)]
struct RegisteredExecutableItem {
    executable: RegisteredExecutable,
    index: usize,
    selected: bool,
}

#[derive(Debug)]
enum RegisteredExecutableMsg {
    Select,
    Launch,
    Remove,
    ShowInfo,
}

#[derive(Debug)]
enum RegisteredExecutableOutput {
    Selected(usize),
    Launch(usize),
    Remove(usize),
    ShowInfo(usize),
}

#[relm4::factory]
impl FactoryComponent for RegisteredExecutableItem {
    type Init = (RegisteredExecutable, usize);
    type Input = RegisteredExecutableMsg;
    type Output = RegisteredExecutableOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_all: 10,
            set_width_request: 120,
            set_height_request: 120,
            set_focusable: true,
            
            // Selection indicator
            #[watch]
            set_css_classes: if self.selected { &["card", "selected"] } else { &["card"] },
            
            // Clickable area for selection
            gtk::Button {
                add_css_class: "flat",
                add_css_class: "selection-area",
                set_cursor_from_name: Some("pointer"),
                connect_clicked[sender, index = self.index] => move |_| {
                    sender.input(RegisteredExecutableMsg::Select);
                    sender.output(RegisteredExecutableOutput::Selected(index)).unwrap();
                },
                
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    
                    gtk::Image {
                        set_pixel_size: 48,
                        #[watch]
                        set_from_file: self.executable.icon_path.as_ref(),
                        set_icon_name: Some("application-x-executable"),
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &self.executable.name,
                        set_halign: gtk::Align::Center,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        set_max_width_chars: 15,
                        set_lines: 2,
                        set_wrap: true,
                        set_wrap_mode: gtk::pango::WrapMode::WordChar,
                    },
                },
            },

            // Action buttons (visible on hover/selection)
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 4,
                set_halign: gtk::Align::Center,
                set_margin_top: 5,
                #[watch]
                set_visible: self.selected,

                gtk::Button {
                    set_icon_name: "media-playback-start-symbolic",
                    set_tooltip_text: Some("Launch"),
                    add_css_class: "circular",
                    add_css_class: "suggested-action",
                    connect_clicked[sender, index = self.index] => move |_| {
                        sender.output(RegisteredExecutableOutput::Launch(index)).unwrap();
                    },
                },

                gtk::Button {
                    set_icon_name: "dialog-information-symbolic",
                    set_tooltip_text: Some("Info"),
                    add_css_class: "circular",
                    connect_clicked[sender, index = self.index] => move |_| {
                        sender.output(RegisteredExecutableOutput::ShowInfo(index)).unwrap();
                    },
                },

                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    set_tooltip_text: Some("Remove"),
                    add_css_class: "circular",
                    add_css_class: "destructive-action",
                    connect_clicked[sender, index = self.index] => move |_| {
                        sender.output(RegisteredExecutableOutput::Remove(index)).unwrap();
                    },
                },
            },
        }
    }

    fn init_model(
        init: Self::Init,
        index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let (executable, idx) = init;
        Self {
            executable,
            index: idx,
            selected: false,
        }
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            RegisteredExecutableMsg::Select => {
                self.selected = true;
            }
            _ => {}
        }
    }
}

#[relm4::component(pub, async)]
impl AsyncComponent for RegisteredAppsListModel {
    type Init = Vec<RegisteredExecutable>;
    type Input = RegisteredAppsListMsg;
    type Output = RegisteredAppsListOutput;
    type CommandOutput = ();
    type Widgets = RegisteredAppsListWidgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 5,
            set_margin_all: 10,

            gtk::Label {
                #[watch]
                set_label: &format!("{} applications registered", model.registered_executables.len()),
                add_css_class: "caption",
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                set_min_content_height: 200,

                #[local_ref]
                registered_grid -> gtk::FlowBox {
                    set_row_spacing: 15,
                    set_column_spacing: 15,
                    set_margin_all: 10,
                    set_max_children_per_line: 5,
                    set_min_children_per_line: 3,
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_valign: gtk::Align::Start,
                    set_halign: gtk::Align::Fill,
                },
            },

            gtk::Label {
                set_label: "No registered applications\nAdd applications from left panel",
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_wrap: true,
                #[watch]
                set_visible: model.registered_executables.len() == 0,
                add_css_class: "dim-label",
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Initialize factory for registered executables (grid layout)
        let executables = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.clone().input_sender(), move |output| match output {
                RegisteredExecutableOutput::Selected(index) => {
                    // Handle selection internally and also forward to parent
                    RegisteredAppsListMsg::SelectExecutable(index)
                }
                RegisteredExecutableOutput::Launch(index) => {
                    // Forward launch action directly to parent
                    RegisteredAppsListMsg::SelectExecutable(index)
                }
                RegisteredExecutableOutput::Remove(index) => {
                    // Forward remove action directly to parent
                    RegisteredAppsListMsg::SelectExecutable(index)
                }
                RegisteredExecutableOutput::ShowInfo(index) => {
                    // Forward show info action directly to parent
                    sender.output(RegisteredAppsListOutput::ShowInfo(index)).unwrap();
                    RegisteredAppsListMsg::SelectExecutable(index)
                }
            });

        let mut model = RegisteredAppsListModel {
            executables,
            registered_executables: init.clone(),
            selected_index: None,
            tracker: 0
        };

        // Initialize with provided executables
        {
            let mut guard = model.executables.guard();
            for (idx, exe) in init.iter().enumerate() {
                guard.push_back((exe.clone(), idx));
            }
        }

        // Get references to the factory widgets
        let registered_grid = model.executables.widget();

        let widgets = view_output!();

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
            RegisteredAppsListMsg::UpdateExecutables(executables) => {
                self.registered_executables = executables.clone();
                
                // Update factory
                let mut guard = self.executables.guard();
                guard.clear();
                for (idx, exe) in executables.iter().enumerate() {
                    guard.push_back((exe.clone(), idx));
                }
            }
            RegisteredAppsListMsg::SelectExecutable(index) => {
                self.set_selected_index(Some(index));
                
                // Update selection state in factory without rebuilding the entire list
                let mut guard = self.executables.guard();
                for (idx, item) in guard.iter_mut().enumerate() {
                    item.selected = Some(idx) == self.selected_index;
                }
                
                sender.output(RegisteredAppsListOutput::Selected(index));
            }
            RegisteredAppsListMsg::ClearSelection => {
                println!("DEBUG: ClearSelection called");
                self.set_selected_index(None);
                
                // Clear selection state in factory without rebuilding entire list
                let mut guard = self.executables.guard();
                for item in guard.iter_mut() {
                    item.selected = false;
                }
            }
        }
    }
}