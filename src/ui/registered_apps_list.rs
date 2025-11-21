use relm4::{
    gtk,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    view,
    RelmWidgetExt,
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
}

#[derive(Debug)]
pub enum RegisteredAppsListMsg {
    UpdateExecutables(Vec<RegisteredExecutable>),
    SelectionChanged,
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
}

#[relm4::factory]
impl FactoryComponent for RegisteredExecutableItem {
    type Init = (RegisteredExecutable, usize);
    type Input = ();
    type Output = ();
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

            // Use FlowBox's built-in selection
            add_css_class: "card",

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
                    set_hexpand: true,
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
        }
    }

    fn init_model(
        init: Self::Init,
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let (executable, idx) = init;
        Self {
            executable,
            index: idx,
        }
    }

    fn update(&mut self, _msg: Self::Input, _sender: FactorySender<Self>) {
        // No messages to handle - selection is handled by FlowBox
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
                    set_selection_mode: gtk::SelectionMode::Single,
                    set_homogeneous: true,
                    set_valign: gtk::Align::Start,
                    set_halign: gtk::Align::Fill,
                    connect_selected_children_changed[sender] => move |_| {
                        sender.input(RegisteredAppsListMsg::SelectionChanged);
                    },
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
            .detach();

        let mut model = RegisteredAppsListModel {
            executables,
            registered_executables: init.clone(),
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
            RegisteredAppsListMsg::SelectionChanged => {
                // Get the FlowBox widget to query selected children
                let flowbox = self.executables.widget();
                let selected_children = flowbox.selected_children();

                if let Some(child) = selected_children.first() {
                    // Get the index of the selected child
                    let index = child.index() as usize;
                    if index < self.registered_executables.len() {
                        sender.output(RegisteredAppsListOutput::Selected(index));
                    }
                }
            }
        }
    }
}