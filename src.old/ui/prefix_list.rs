use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use gtk::prelude::*;
use gtk::gio;
use crate::prefix::WinePrefix;

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
        widgets.prefix_list_box.connect_row_activated(move |_, row| {
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

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PrefixListMsg::SetPrefixes(prefixes) => {
                println!("SetPrefixes received: {} items", prefixes.len());
                self.prefixes = prefixes.clone();
                populate(&self.prefixes, &self.list_box, &sender);
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

    println!("populate: {} prefixes", prefixes.len());
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
            &gtk::ListBoxRow::builder().selectable(false).child(&label).build(),
        );
        return;
    }

    for (i, prefix) in prefixes.iter().enumerate() {
        let name = gtk::Label::builder()
            .label(&prefix.name).halign(gtk::Align::Start)
            .css_classes(["heading"]).build();

        let detail = gtk::Label::builder()
            .label(&format!("{} · {} apps",
                prefix.config.architecture,
                prefix.config.registered_executables.len()))
            .halign(gtk::Align::Start)
            .css_classes(["caption", "dim-label"]).build();

        let box_ = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical).spacing(1)
            .hexpand(true)
            .margin_top(3).margin_bottom(3)
            .margin_start(8).margin_end(8)
            .build();
        box_.append(&name);
        box_.append(&detail);

        let row = gtk::ListBoxRow::builder()
            .selectable(true).activatable(true).child(&box_).build();

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
            let action_name = format!("delete-{}", i);
            let action = gio::SimpleAction::new(&action_name, None);
            let actions = gio::SimpleActionGroup::new();
            actions.add_action(&action);
            row_ref.insert_action_group("pref", Some(&actions));

            let menu = gio::Menu::new();
            menu.append(Some("Delete Prefix"), Some(&format!("pref.{}", action_name)));

            let popover = gtk::PopoverMenu::from_model(Some(&menu));
            popover.set_has_arrow(false);
            popover.set_halign(gtk::Align::Start);
            popover.set_parent(&row_ref);
            let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));

            let popover_clone = popover.clone();
            let s = s.clone();
            let name = prefix_name.clone();
            action.connect_activate(move |_, _| {
                popover_clone.popdown();

                let dlg = gtk::Dialog::builder()
                    .title("Delete Prefix")
                    .modal(true)
                    .build();
                dlg.add_button("Cancel", gtk::ResponseType::Cancel);
                dlg.add_button("Delete", gtk::ResponseType::Ok);

                let content = dlg.content_area();
                content.append(&gtk::Label::builder()
                    .label(&format!(
                        "Are you sure you want to delete the prefix \"{}\"?\n\nThis will permanently remove all files in the prefix directory.",
                        name
                    ))
                    .wrap(true)
                    .halign(gtk::Align::Start)
                    .margin_top(16).margin_bottom(16)
                    .margin_start(16).margin_end(16)
                    .build());

                if let Some(btn) = dlg.widget_for_response(gtk::ResponseType::Ok) {
                    btn.add_css_class("destructive-action");
                }

                let s = s.clone();
                dlg.connect_response(move |dlg, response| {
                    if response == gtk::ResponseType::Ok {
                        let _ = s.output(PrefixListOutput::DeletePrefix(i));
                    }
                    dlg.close();
                });

                dlg.present();
            });

            popover.popup();
        });
        row.add_controller(gesture);
        list_box.append(&row);
    }
}
