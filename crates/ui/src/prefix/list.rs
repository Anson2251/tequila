use gtk::gio;
use log::info;
use prefix::WinePrefix;
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, adw, gtk};

#[derive(Debug)]
pub struct PrefixListModel {
    prefixes: Vec<WinePrefix>,
    selected_prefix: Option<usize>,
    list_box: gtk::ListBox,
}

#[derive(Debug)]
pub enum PrefixListMsg {
    SelectPrefix(usize),
    SetPrefixes(Vec<WinePrefix>),
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

    view! {
        gtk::ScrolledWindow {
            set_vexpand: true,

            #[name = "prefix_list_box"]
            gtk::ListBox {
                set_selection_mode: gtk::SelectionMode::Single,
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (prefixes, selected_prefix) = init;

        let widgets = view_output!();

        let sender_clone = sender.clone();
        widgets
            .prefix_list_box
            .connect_row_activated(move |_, row| {
                if let Some(idx) = row.index().checked_sub(0) {
                    if idx >= 0 {
                        sender_clone.input(PrefixListMsg::SelectPrefix(idx as usize));
                    }
                }
            });

        let model = PrefixListModel {
            prefixes: prefixes.clone(),
            selected_prefix,
            list_box: widgets.prefix_list_box.clone(),
        };

        populate(&model.prefixes, &model.list_box, &sender);

        // Auto-select first prefix if there's exactly one
        if model.prefixes.len() == 1 {
            let _ = sender.output(PrefixListOutput::SelectPrefix(0));
        } else {
            let lb = model.list_box.clone();
            gtk::glib::idle_add_local(move || {
                lb.unselect_all();
                gtk::glib::ControlFlow::Break
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PrefixListMsg::SetPrefixes(prefixes) => {
                log::debug!("[list] set_prefixes received: {} items", prefixes.len());
                self.prefixes = prefixes.clone();
                populate(&self.prefixes, &self.list_box, &sender);

                // Auto-select first prefix if there's exactly one
                if prefixes.len() == 1 {
                    let _ = sender.output(PrefixListOutput::SelectPrefix(0));
                }
            }
            PrefixListMsg::SelectPrefix(index) => {
                if self.selected_prefix == Some(index) {
                    self.selected_prefix = None;
                    self.list_box.unselect_all();
                    let _ = sender.output(PrefixListOutput::DeselectPrefix);
                } else {
                    self.selected_prefix = Some(index);
                    let _ = sender.output(PrefixListOutput::SelectPrefix(index));
                }
            }
        }
    }
}

fn populate(
    prefixes: &[WinePrefix],
    list_box: &gtk::ListBox,
    sender: &ComponentSender<PrefixListModel>,
) {
    while let Some(row) = list_box.first_child() {
        list_box.remove(&row);
    }

    log::debug!("[list] populate: {} prefixes", prefixes.len());
    if prefixes.is_empty() {
        let label = gtk::Label::builder()
            .label("No Wine prefixes found")
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(40)
            .wrap(true)
            .css_classes(["dim-label", "body"])
            .build();
        list_box.append(
            &gtk::ListBoxRow::builder()
                .selectable(false)
                .child(&label)
                .build(),
        );
        return;
    }

    for (i, prefix) in prefixes.iter().enumerate() {
        let name = gtk::Label::builder()
            .label(&prefix.name)
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build();

        let detail = gtk::Label::builder()
            .label(&format!(
                "{} · {} apps",
                prefix.config.architecture,
                prefix.config.registered_executables.len()
            ))
            .halign(gtk::Align::Start)
            .css_classes(["caption", "dim-label"])
            .build();

        let box_ = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(1)
            .hexpand(true)
            .margin_top(3)
            .margin_bottom(3)
            .margin_start(8)
            .margin_end(8)
            .build();
        box_.append(&name);
        box_.append(&detail);

        let row = gtk::ListBoxRow::builder()
            .selectable(true)
            .activatable(true)
            .child(&box_)
            .build();

        // Left-click → select
        let s = sender.clone();
        row.connect_activate(move |_| s.input(PrefixListMsg::SelectPrefix(i)));

        // Right-click → context menu
        let s = sender.clone();
        let prefix_name = prefix.name.clone();
        let row_ref = row.clone();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3); // right button
        gesture.connect_pressed(move |_gesture, _n_press, x, y| {
            let prefix_idx = i;

            let export_action = gio::SimpleAction::new("export", None);
            let open_fm_action = gio::SimpleAction::new("open-fm", None);
            let open_term_action = gio::SimpleAction::new("open-term", None);
            let delete_action = gio::SimpleAction::new("delete", None);
            let actions = gio::SimpleActionGroup::new();
            actions.add_action(&open_fm_action);
            actions.add_action(&open_term_action);
            actions.add_action(&export_action);
            actions.add_action(&delete_action);
            row_ref.insert_action_group("pref", Some(&actions));

            let menu = gio::Menu::new();
            menu.append(Some("Open in File Manager"), Some("pref.open-fm"));
            menu.append(Some("Open in Terminal"), Some("pref.open-term"));
            menu.append(Some("Export Prefix"), Some("pref.export"));
            menu.append(Some("Delete Prefix"), Some("pref.delete"));

            let popover = gtk::PopoverMenu::from_model(Some(&menu));
            popover.set_has_arrow(false);
            popover.set_halign(gtk::Align::Start);
            popover.set_parent(&row_ref);
            let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));

            let popover_clone = popover.clone();
            let s_export = s.clone();
            export_action.connect_activate(move |_, _| {
                popover_clone.popdown();
                let _ = s_export.output(PrefixListOutput::ExportPrefix(prefix_idx));
            });

            let s_fm = s.clone();
            open_fm_action.connect_activate(move |_, _| {
                let _ = s_fm.output(PrefixListOutput::OpenInFileManager(prefix_idx));
            });

            let s_term = s.clone();
            open_term_action.connect_activate(move |_, _| {
                let _ = s_term.output(PrefixListOutput::OpenInTerminal(prefix_idx));
            });

            let popover_clone2 = popover.clone();
            let s_del = s.clone();
            let name = prefix_name.clone();
            delete_action.connect_activate(move |_, _| {
                popover_clone2.popdown();

                let alert = adw::AlertDialog::new(
                    Some("Delete Prefix"),
                    Some(&format!(
                        "Are you sure you want to delete the prefix \"{}\"?\n\nThis will permanently remove all files in the prefix directory.",
                        name
                    )),
                );
                alert.add_response("cancel", "Cancel");
                alert.add_response("delete", "Delete");
                alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                alert.set_default_response(Some("cancel"));
                alert.set_close_response("cancel");
                let s = s_del.clone();
                alert.choose(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, move |response| {
                    if response == "delete" {
                        let _ = s.output(PrefixListOutput::DeletePrefix(prefix_idx));
                    }
                });
            });

            popover.popup();
        });
        row.add_controller(gesture);
        list_box.append(&row);
    }
    // Unselect all to prevent auto-selecting the first row
    list_box.unselect_all();
}
