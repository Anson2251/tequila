use gtk::gio;
use prefix::WinePrefix;
use relm4::RelmWidgetExt;
use relm4::adw::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

#[derive(Debug)]
pub struct PrefixListModel {
    prefixes: Vec<WinePrefix>,
    selected_prefix: Option<usize>,
    prefix_items: FactoryVecDeque<PrefixItem>,
}

#[derive(Debug)]
pub enum PrefixListMsg {
    SelectPrefix(usize),
    SetPrefixes(Vec<WinePrefix>),
    Export(usize),
    OpenInFileManager(usize),
    OpenInTerminal(usize),
    Delete(usize),
}

#[derive(Debug)]
pub enum PrefixListOutput {
    SelectPrefix(usize),
    DeselectPrefix,
    DeletePrefix(usize),
    ExportPrefix(usize),
    OpenInFileManager(usize),
    OpenInTerminal(usize),
}

#[relm4::component(pub)]
impl SimpleComponent for PrefixListModel {
    type Init = (Vec<WinePrefix>, Option<usize>);
    type Input = PrefixListMsg;
    type Output = PrefixListOutput;
    type Widgets = PrefixListWidgets;

    #[rustfmt::skip]
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            add_css_class: "prefix-sidebar",

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                #[watch]
                set_visible: !model.prefixes.is_empty(),

                #[local_ref]
                prefix_list_box -> gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::Single,
                    set_margin_all: 6,
                },
            },

            gtk::Label {
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_vexpand: true,
                set_margin_top: 40,
                set_wrap: true,
                set_css_classes: &["dim-label", "body"],
                set_label: &crate::t!("sidebar.no_prefixes"),
                #[watch]
                set_visible: model.prefixes.is_empty(),
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefixes, selected_prefix) = init;

        let mut model = PrefixListModel {
            prefixes: prefixes.clone(),
            selected_prefix,
            prefix_items: FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |msg| match msg {
                    PrefixItemOutput::Export(index) => PrefixListMsg::Export(index),
                    PrefixItemOutput::OpenInFileManager(index) => {
                        PrefixListMsg::OpenInFileManager(index)
                    }
                    PrefixItemOutput::OpenInTerminal(index) => PrefixListMsg::OpenInTerminal(index),
                    PrefixItemOutput::Delete(index) => PrefixListMsg::Delete(index),
                }),
        };

        let prefix_list_box = model.prefix_items.widget();
        let widgets = view_output!();

        // Row activation (left click / Enter) → toggle selection
        let sender_clone = sender.clone();
        prefix_list_box.connect_row_activated(move |_, row| {
            let index = row.index();
            if index >= 0 {
                sender_clone.input(PrefixListMsg::SelectPrefix(index as usize));
            }
        });

        model.sync_items(&prefixes);

        // Auto-select first prefix if there's exactly one
        if model.prefixes.len() == 1 {
            model.selected_prefix = Some(0);
            model.sync_selection();
            let _ = sender.output(PrefixListOutput::SelectPrefix(0));
        } else {
            model.sync_selection();
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PrefixListMsg::SetPrefixes(prefixes) => {
                log::debug!("[list] set_prefixes received: {} items", prefixes.len());
                self.prefixes = prefixes.clone();
                self.selected_prefix = None;
                self.sync_items(&prefixes);
                self.sync_selection();

                // Auto-select first prefix if there's exactly one
                if prefixes.len() == 1 {
                    self.selected_prefix = Some(0);
                    self.sync_selection();
                    let _ = sender.output(PrefixListOutput::SelectPrefix(0));
                }
            }
            PrefixListMsg::SelectPrefix(index) => {
                if self.selected_prefix == Some(index) {
                    self.selected_prefix = None;
                    let _ = sender.output(PrefixListOutput::DeselectPrefix);
                } else {
                    self.selected_prefix = Some(index);
                    let _ = sender.output(PrefixListOutput::SelectPrefix(index));
                }
                self.sync_selection();
            }
            PrefixListMsg::Export(index) => {
                let _ = sender.output(PrefixListOutput::ExportPrefix(index));
            }
            PrefixListMsg::OpenInFileManager(index) => {
                let _ = sender.output(PrefixListOutput::OpenInFileManager(index));
            }
            PrefixListMsg::OpenInTerminal(index) => {
                let _ = sender.output(PrefixListOutput::OpenInTerminal(index));
            }
            PrefixListMsg::Delete(index) => {
                let _ = sender.output(PrefixListOutput::DeletePrefix(index));
            }
        }
    }
}

impl PrefixListModel {
    fn sync_items(&mut self, prefixes: &[WinePrefix]) {
        let mut guard = self.prefix_items.guard();
        guard.clear();
        for prefix in prefixes {
            guard.push_back(prefix.clone());
        }
    }

    /// Keep the ListBox's visual selection in sync with `selected_prefix`.
    fn sync_selection(&self) {
        let list_box = self.prefix_items.widget();
        match self.selected_prefix {
            Some(index) => {
                if let Some(row) = list_box.row_at_index(index as i32) {
                    list_box.select_row(Some(&row));
                }
            }
            None => list_box.unselect_all(),
        }
    }
}

#[derive(Debug)]
struct PrefixItem {
    prefix: WinePrefix,
    index: DynamicIndex,
}

#[derive(Debug)]
enum PrefixItemInput {
    Export,
    OpenInFileManager,
    OpenInTerminal,
    Delete,
}

#[derive(Debug)]
enum PrefixItemOutput {
    Export(usize),
    OpenInFileManager(usize),
    OpenInTerminal(usize),
    Delete(usize),
}

impl PrefixItem {
    fn detail(&self) -> String {
        format!(
            "{} · {} apps",
            self.prefix.config.architecture,
            self.prefix.config.registered_executables.len()
        )
    }
}

#[relm4::factory]
impl FactoryComponent for PrefixItem {
    type Init = WinePrefix;
    type Input = PrefixItemInput;
    type Output = PrefixItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    #[rustfmt::skip]
    view! {
        #[root]
        gtk::ListBoxRow {
            set_selectable: true,
            set_activatable: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 6,
                set_hexpand: true,
                set_margin_top: 6,
                set_margin_bottom: 6,
                set_margin_start: 12,
                set_margin_end: 12,

                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                    set_xalign: 0.0,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 22,
                    set_single_line_mode: true,
                    set_css_classes: &["heading"],
                    #[watch]
                    set_label: &self.prefix.name,
                    #[watch]
                    set_tooltip_text: Some(&self.prefix.name),
                },

                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                    set_xalign: 0.0,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 22,
                    set_single_line_mode: true,
                    set_css_classes: &["caption", "dim-label"],
                    #[watch]
                    set_label: &self.detail(),
                },
            },
        }
    }

    fn init_model(init: Self::Init, index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            prefix: init,
            index: index.clone(),
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: Self::Root,
        _returned_widget: &gtk::ListBoxRow,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        // Right-click → context menu
        let widget = root.upcast_ref::<gtk::Widget>().clone();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3); // right button
        let s = sender.clone();
        gesture.connect_pressed(move |_gesture, _n_press, x, y| {
            show_context_menu(&widget, x, y, &s);
        });
        root.add_controller(gesture);

        widgets
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        let index = self.index.current_index();
        match msg {
            PrefixItemInput::Export => {
                let _ = sender.output(PrefixItemOutput::Export(index));
            }
            PrefixItemInput::OpenInFileManager => {
                let _ = sender.output(PrefixItemOutput::OpenInFileManager(index));
            }
            PrefixItemInput::OpenInTerminal => {
                let _ = sender.output(PrefixItemOutput::OpenInTerminal(index));
            }
            PrefixItemInput::Delete => {
                let name = self.prefix.name.clone();
                let alert = adw::AlertDialog::new(
                    Some(&crate::t!("prefix.delete.title")),
                    Some(&crate::tf!("prefix.delete.confirm", "name" => &name)),
                );
                alert.add_response("cancel", &crate::t!("prefix.delete.cancel"));
                alert.add_response("delete", &crate::t!("prefix.delete.confirm_btn"));
                alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                alert.set_default_response(Some("cancel"));
                alert.set_close_response("cancel");

                let index = self.index.clone();
                let s = sender.clone();
                alert.choose(
                    None::<&gtk::Window>,
                    None::<&gio::Cancellable>,
                    move |response| {
                        if response == "delete" {
                            let _ = s.output(PrefixItemOutput::Delete(index.current_index()));
                        }
                    },
                );
            }
        }
    }
}

fn show_context_menu(widget: &gtk::Widget, x: f64, y: f64, sender: &FactorySender<PrefixItem>) {
    let export_action = gio::SimpleAction::new("export", None);
    let open_fm_action = gio::SimpleAction::new("open-fm", None);
    let open_term_action = gio::SimpleAction::new("open-term", None);
    let delete_action = gio::SimpleAction::new("delete", None);
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&open_fm_action);
    actions.add_action(&open_term_action);
    actions.add_action(&export_action);
    actions.add_action(&delete_action);
    widget.insert_action_group("pref", Some(&actions));

    let menu = gio::Menu::new();
    menu.append(
        Some(&crate::t!("prefix.context.open_fm")),
        Some("pref.open-fm"),
    );
    menu.append(
        Some(&crate::t!("prefix.context.open_term")),
        Some("pref.open-term"),
    );
    menu.append(
        Some(&crate::t!("prefix.context.export")),
        Some("pref.export"),
    );
    menu.append(
        Some(&crate::t!("prefix.context.delete")),
        Some("pref.delete"),
    );

    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(widget);
    let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
    popover.set_pointing_to(Some(&rect));

    let s = sender.clone();
    open_fm_action.connect_activate(move |_, _| {
        s.input(PrefixItemInput::OpenInFileManager);
    });

    let s = sender.clone();
    open_term_action.connect_activate(move |_, _| {
        s.input(PrefixItemInput::OpenInTerminal);
    });

    let popover_clone = popover.clone();
    let s = sender.clone();
    export_action.connect_activate(move |_, _| {
        popover_clone.popdown();
        s.input(PrefixItemInput::Export);
    });

    let popover_clone = popover.clone();
    let s = sender.clone();
    delete_action.connect_activate(move |_, _| {
        popover_clone.popdown();
        s.input(PrefixItemInput::Delete);
    });

    popover.popup();
}
