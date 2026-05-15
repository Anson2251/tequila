use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use gtk::prelude::*;
use crate::prefix::WinePrefix;

#[derive(Debug)]
pub struct PrefixListModel {
    prefixes: Vec<WinePrefix>,
    selected_prefix: Option<usize>,
}

#[derive(Debug)]
pub enum PrefixListMsg {
    SelectPrefix(usize),
    ShowPrefixDetails(usize),
}

#[derive(Debug)]
pub enum PrefixListOutput {
    SelectPrefix(usize),
    ShowPrefixDetails(usize),
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

        let model = PrefixListModel {
            prefixes: prefixes.clone(),
            selected_prefix,
        };

        let widgets = view_output!();

        let sender_clone = sender.clone();
        widgets.prefix_list_box.connect_row_activated(move |list_box, row| {
            if let Some(idx) = row.index().checked_sub(0) {
                if idx >= 0 {
                    sender_clone.input(PrefixListMsg::SelectPrefix(idx as usize));
                }
            }
        });

        populate(&model.prefixes, &widgets.prefix_list_box, &sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PrefixListMsg::SelectPrefix(index) => {
                self.selected_prefix = Some(index);
                let _ = sender.output(PrefixListOutput::SelectPrefix(index));
            }
            PrefixListMsg::ShowPrefixDetails(index) => {
                let _ = sender.output(PrefixListOutput::ShowPrefixDetails(index));
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

        let s = sender.clone();
        row.connect_activate(move |_| s.input(PrefixListMsg::SelectPrefix(i)));
        list_box.append(&row);
    }
}
