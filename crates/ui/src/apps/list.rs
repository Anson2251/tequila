use gtk::gio;
use gtk::prelude::*;
use prefix::config::RegisteredExecutable;
use prefix::{IconCache, resolve_or_extract_icon};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::{
    RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk, adw
};
use std::path::PathBuf;
use std::sync::Arc;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct RegisteredAppsListModel {
    #[tracker::do_not_track]
    executables: FactoryVecDeque<RegisteredExecutableItem>,
    registered_executables: Vec<RegisteredExecutable>,
    #[tracker::do_not_track]
    selection_handler_id: Option<gtk::glib::SignalHandlerId>,
    #[tracker::do_not_track]
    prefix_path: PathBuf,
    #[tracker::do_not_track]
    icon_cache: Arc<IconCache>,
}

#[derive(Debug)]
pub enum RegisteredAppsListMsg {
    UpdateExecutables(Vec<RegisteredExecutable>),
    SetRunningPaths(std::collections::HashSet<std::path::PathBuf>),
    PrefixPathUpdated(PathBuf),
    SelectionChanged,
    ContextLaunch(usize),
    ContextLaunchDebug(usize),
    ContextShowInfo(usize),
    ContextRemove(usize),
    ContextToggleDesktop(usize),
}

#[derive(Debug)]
pub enum RegisteredAppsListOutput {
    Selected(usize),
    Launch(usize),
    LaunchDebug(usize),
    Remove(usize),
    ShowInfo(usize),
    ToggleDesktop(usize),
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
    resolved_icon: Option<PathBuf>,
}

#[derive(Debug)]
pub enum RegisteredExecutableItemOutput {
    Launch(usize),
    LaunchDebug(usize),
    ShowInfo(usize),
    Remove(usize),
    ToggleDesktop(usize),
}

#[relm4::factory]
impl FactoryComponent for RegisteredExecutableItem {
    type Init = (RegisteredExecutable, usize, Option<PathBuf>);
    type Input = ();
    type Output = RegisteredExecutableItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    #[rustfmt::skip]
    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 6,
            set_margin_all: 12,
            set_width_request: 64,
            set_height_request: 64,
            set_focusable: true,

            #[watch]
            set_css_classes: if self.is_running { &["app-item", "running"] } else { &["app-item"] },

            // Icon from file, or fallback default
            gtk::Box {
                set_halign: gtk::Align::Center,

                adw::Clamp {
                    set_width_request: 64,
                    set_height_request: 64,
                    add_css_class: "icon-bg",

                    gtk::Box {
                        set_margin_all: 12,

                        gtk::Image {
                            set_pixel_size: 48,
                            #[watch]
                            set_from_file: self.resolved_icon.as_deref(),
                            #[watch]
                            set_visible: self.resolved_icon.is_some(),
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_vexpand: true,
                        },
                        gtk::Image {
                            set_pixel_size: 48,
                            set_icon_name: Some("application-x-executable"),
                            #[watch]
                            set_visible: self.resolved_icon.is_none(),
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            set_vexpand: true,
                        },
                    },
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
        let (executable, idx, resolved_icon) = init;
        Self {
            executable,
            index: idx,
            is_running: false,
            resolved_icon,
        }
    }

    fn update(&mut self, _msg: Self::Input, _sender: FactorySender<Self>) {}

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: Self::Root,
        _returned_widget: &gtk::FlowBoxChild,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();
        let widget: gtk::Widget = root.clone().upcast();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let idx = self.index;
        let name = self.executable.name.clone();
        let s = sender.clone();
        gesture.connect_pressed(move |_, _, x, y| {
            show_app_context_menu(&widget, x, y, idx, &name, &s);
        });
        root.add_controller(gesture);
        widgets
    }
}

fn show_app_context_menu(
    widget: &gtk::Widget,
    x: f64,
    y: f64,
    idx: usize,
    name: &str,
    sender: &FactorySender<RegisteredExecutableItem>,
) {
    let launch = gio::SimpleAction::new("launch", None);
    let launch_debug = gio::SimpleAction::new("launch-debug", None);
    let info = gio::SimpleAction::new("info", None);
    let remove = gio::SimpleAction::new("remove", None);
    let desktop = gio::SimpleAction::new("desktop", None);
    let group = gio::SimpleActionGroup::new();
    group.add_action(&launch);
    group.add_action(&launch_debug);
    group.add_action(&info);
    group.add_action(&remove);
    group.add_action(&desktop);
    widget.insert_action_group("app", Some(&group));

    let menu = gio::Menu::new();
    menu.append(Some(&format!("Launch \"{}\"", name)), Some("app.launch"));
    menu.append(Some("Launch with Debug"), Some("app.launch-debug"));
    menu.append(Some("Show Info"), Some("app.info"));
    menu.append(Some("Toggle Desktop Shortcut"), Some("app.desktop"));
    menu.append(Some("Remove"), Some("app.remove"));

    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(widget);
    let rect = gtk::gdk::Rectangle::new(0, 0, 1, 1);
    popover.set_pointing_to(Some(&rect));
    let _ = (x, y);

    let s = sender.clone();
    launch.connect_activate(move |_, _| {
        let _ = s.output(RegisteredExecutableItemOutput::Launch(idx));
    });
    let s = sender.clone();
    launch_debug.connect_activate(move |_, _| {
        let _ = s.output(RegisteredExecutableItemOutput::LaunchDebug(idx));
    });
    let s = sender.clone();
    info.connect_activate(move |_, _| {
        let _ = s.output(RegisteredExecutableItemOutput::ShowInfo(idx));
    });
    let s = sender.clone();
    remove.connect_activate(move |_, _| {
        let _ = s.output(RegisteredExecutableItemOutput::Remove(idx));
    });
    let s = sender.clone();
    desktop.connect_activate(move |_, _| {
        let _ = s.output(RegisteredExecutableItemOutput::ToggleDesktop(idx));
    });

    popover.popup();
}

#[relm4::component(pub, async)]
impl AsyncComponent for RegisteredAppsListModel {
    type Init = (Vec<RegisteredExecutable>, PathBuf, Arc<IconCache>);
    type Input = RegisteredAppsListMsg;
    type Output = RegisteredAppsListOutput;
    type CommandOutput = ();
    type Widgets = RegisteredAppsListWidgets;

    #[rustfmt::skip]
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 6,
            set_margin_all: 12,
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
                    set_margin_all: 12,
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
                set_label: &crate::tf!("apps.registered_count", "count" => &model.registered_executables.len().to_string()),
                add_css_class: "caption",
            },

            gtk::Label {
                set_label: &crate::t!("apps.no_registered"),
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
        let (executables_init, prefix_path, icon_cache) = init;

        let executables = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |o| match o {
                RegisteredExecutableItemOutput::Launch(i) => RegisteredAppsListMsg::ContextLaunch(i),
                RegisteredExecutableItemOutput::LaunchDebug(i) => {
                    RegisteredAppsListMsg::ContextLaunchDebug(i)
                }
                RegisteredExecutableItemOutput::ShowInfo(i) => {
                    RegisteredAppsListMsg::ContextShowInfo(i)
                }
                RegisteredExecutableItemOutput::Remove(i) => RegisteredAppsListMsg::ContextRemove(i),
                RegisteredExecutableItemOutput::ToggleDesktop(i) => {
                    RegisteredAppsListMsg::ContextToggleDesktop(i)
                }
            });

        let prefix_path_for_init = prefix_path.clone();
        let icon_cache_for_init = Arc::clone(&icon_cache);
        let mut model = RegisteredAppsListModel {
            executables,
            registered_executables: executables_init.clone(),
            selection_handler_id: None,
            prefix_path,
            icon_cache,
            tracker: 0,
        };

        // Initialize with provided executables — resolve + extract fallback
        {
            let mut guard = model.executables.guard();
            for (idx, exe) in executables_init.iter().enumerate() {
                let resolved =
                    resolve_or_extract_icon(exe, &prefix_path_for_init, &icon_cache_for_init);
                guard.push_back((exe.clone(), idx, resolved));
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

                let prefix_path = self.prefix_path.clone();
                let icon_cache = Arc::clone(&self.icon_cache);
                let mut guard = self.executables.guard();
                guard.clear();
                for (idx, exe) in executables.iter().enumerate() {
                    let resolved = resolve_or_extract_icon(exe, &prefix_path, &icon_cache);
                    guard.push_back((exe.clone(), idx, resolved));
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
            RegisteredAppsListMsg::PrefixPathUpdated(prefix_path) => {
                if self.prefix_path == prefix_path {
                    return;
                }
                self.prefix_path = prefix_path;

                // Re-resolve icons in case the prefix location changed
                let prefix_path = self.prefix_path.clone();
                let icon_cache = Arc::clone(&self.icon_cache);
                let mut guard = self.executables.guard();
                for item in guard.iter_mut() {
                    item.resolved_icon =
                        resolve_or_extract_icon(&item.executable, &prefix_path, &icon_cache);
                }
            }
            RegisteredAppsListMsg::SelectionChanged => {
                let flowbox = self.executables.widget();
                let selected_children = flowbox.selected_children();

                if let Some(child) = selected_children.first() {
                    let index = child.index() as usize;
                    if index < self.registered_executables.len() {
                        let _ = sender.output(RegisteredAppsListOutput::Selected(index));
                    }
                }
            }
            RegisteredAppsListMsg::ContextLaunch(i) => {
                let _ = sender.output(RegisteredAppsListOutput::Launch(i));
            }
            RegisteredAppsListMsg::ContextLaunchDebug(i) => {
                let _ = sender.output(RegisteredAppsListOutput::LaunchDebug(i));
            }
            RegisteredAppsListMsg::ContextShowInfo(i) => {
                let _ = sender.output(RegisteredAppsListOutput::ShowInfo(i));
            }
            RegisteredAppsListMsg::ContextRemove(i) => {
                let _ = sender.output(RegisteredAppsListOutput::Remove(i));
            }
            RegisteredAppsListMsg::ContextToggleDesktop(i) => {
                let _ = sender.output(RegisteredAppsListOutput::ToggleDesktop(i));
            }
        }
    }
}
