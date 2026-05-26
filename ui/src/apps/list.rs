use gtk::prelude::*;
use prefix::config::RegisteredExecutable;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::{
    RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct RegisteredAppsListModel {
    #[tracker::do_not_track]
    executables: FactoryVecDeque<RegisteredExecutableItem>,
    registered_executables: Vec<RegisteredExecutable>,
    #[tracker::do_not_track]
    selection_handler_id: Option<gtk::glib::SignalHandlerId>,
}

#[derive(Debug)]
pub enum RegisteredAppsListMsg {
    UpdateExecutables(Vec<RegisteredExecutable>),
    SetRunningPaths(std::collections::HashSet<std::path::PathBuf>),
    SelectionChanged,
}

#[derive(Debug)]
pub enum RegisteredAppsListOutput {
    Selected(usize),
    Launch(usize),
    Remove(usize),
    ShowInfo(usize),
}

impl Drop for RegisteredAppsListModel {
    fn drop(&mut self) {
        // Disconnect the signal before the factory VecDeque clears children during drop,
        // which would trigger selected_children_changed and panic-in-drop.
        if let Some(h) = self.selection_handler_id.take() {
            self.executables.widget().disconnect(h);
        }
    }
}

// Grid-based factory component for registered executables
#[derive(Debug)]
struct RegisteredExecutableItem {
    executable: RegisteredExecutable,
    #[allow(dead_code)]
    index: usize,
    is_running: bool,
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
            set_spacing: 6,
            set_margin_all: 8,
            set_width_request: 64,
            set_height_request: 64,
            set_focusable: true,

            #[watch]
            set_css_classes: if self.is_running { &["app-item", "running"] } else { &["app-item"] },

                // Icon from file, or fallback default
                gtk::Box {
                    set_width_request: 48,
                    set_height_request: 48,
                    set_halign: gtk::Align::Center,
                    add_css_class: "icon-bg",

                    gtk::Image {
                        set_pixel_size: 48,
                        #[watch]
                        set_from_file: self.executable.icon_path.as_deref(),
                        #[watch]
                        set_visible: self.executable.icon_path.is_some(),
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                    },
                    gtk::Image {
                        set_pixel_size: 48,
                        set_icon_name: Some("application-x-executable"),
                        #[watch]
                        set_visible: self.executable.icon_path.is_none(),
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                    },
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

        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let (executable, idx) = init;
        Self {
            executable,
            index: idx,
            is_running: false,
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
            set_vexpand: true,



            gtk::ScrolledWindow {
                #[watch]
                set_visible: model.registered_executables.len() != 0,

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
                },
            },

            gtk::Label {
                #[watch]
                set_visible: model.registered_executables.len() != 0,
                #[watch]
                set_label: &format!("{} applications registered", model.registered_executables.len()),
                add_css_class: "caption",
            },

            gtk::Label {
                set_label: "No registered applications\nAdd applications from left panel",
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_wrap: true,
                #[watch]
                set_visible: model.registered_executables.len() == 0,
                add_css_class: "dim-label",
                set_hexpand: true,
                set_vexpand: true,
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
            selection_handler_id: None,
            tracker: 0,
        };

        // Initialize with provided executables
        {
            let mut guard = model.executables.guard();
            for (idx, exe) in init.iter().enumerate() {
                guard.push_back((exe.clone(), idx));
            }
        }

        let registered_grid = model.executables.widget();
        let widgets = view_output!();

        // Connect selection-changed on the factory's FlowBox so we can block it during clear
        let handler_id = registered_grid.connect_selected_children_changed({
            let sender = sender.clone();
            move |_| {
                let _ = sender.input(RegisteredAppsListMsg::SelectionChanged);
            }
        });
        model.selection_handler_id = Some(handler_id);

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

                // Block selection-changed signal during clear to avoid panic
                {
                    let grid = self.executables.widget();
                    if let Some(ref h) = self.selection_handler_id {
                        grid.block_signal(h);
                    }
                }

                let mut guard = self.executables.guard();
                guard.clear();
                for (idx, exe) in executables.iter().enumerate() {
                    guard.push_back((exe.clone(), idx));
                }
                drop(guard);

                {
                    let grid = self.executables.widget();
                    if let Some(ref h) = self.selection_handler_id {
                        grid.unblock_signal(h);
                    }
                }
            }
            RegisteredAppsListMsg::SetRunningPaths(paths) => {
                let mut guard = self.executables.guard();
                for item in guard.iter_mut() {
                    item.is_running = paths.contains(&item.executable.executable_path);
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
                        let _ = sender.output(RegisteredAppsListOutput::Selected(index));
                    }
                }
            }
        }
    }
}
