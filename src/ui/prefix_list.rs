use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use gtk::prelude::*;
use crate::prefix::WinePrefix;

#[derive(Debug, Clone, PartialEq)]
pub struct PrefixListModel {
    prefixes: Vec<WinePrefix>,
    selected_prefix: Option<usize>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum PrefixListMsg {
    SelectPrefix(usize),
    ShowPrefixDetails(usize),
    // ShowAppManager(usize),
}

#[relm4::component(pub)]
impl SimpleComponent for PrefixListModel {
    type Init = (Vec<WinePrefix>, Option<usize>);
    type Input = PrefixListMsg;
    type Output = PrefixListMsg;
    // type Root = gtk::ScrolledWindow;
    type Widgets = PrefixListWidgets;

    view! {
        gtk::ScrolledWindow {
            set_vexpand: true,
            set_hexpand: true,
            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
            set_width_request: 128,

            #[name = "prefix_list_box"]
            gtk::ListBox {
                set_css_classes: &["boxed-list"],
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
        widgets.prefix_list_box.connect_row_activated(move |_, row| {
            let index = row.index();
            if index >= 0 {
                sender_clone.input(PrefixListMsg::SelectPrefix(index as usize));
            }
        });

        // Populate the prefix list with enhanced information
        Self::populate_prefix_list(&model, &widgets.prefix_list_box, &sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PrefixListMsg::SelectPrefix(index) => {
                self.selected_prefix = Some(index);
                println!("Selected prefix: {}", self.prefixes[index].name);
                let _ = sender.output(PrefixListMsg::SelectPrefix(index));
            }
            PrefixListMsg::ShowPrefixDetails(index) => {
                let _ = sender.output(PrefixListMsg::ShowPrefixDetails(index));
            }
            // PrefixListMsg::ShowAppManager(index) => {
            //     let _ = sender.output(PrefixListMsg::ShowAppManager(index));
            // }
        }
    }
}

impl PrefixListModel {
    fn populate_prefix_list(
        model: &PrefixListModel,
        list_box: &gtk::ListBox,
        sender: &ComponentSender<PrefixListModel>,
    ) {
        // Clear existing items
        while let Some(row) = list_box.first_child() {
            list_box.remove(&row);
        }

        // Add prefixes to the list with enhanced information
        for (index, prefix) in model.prefixes.iter().enumerate() {
            let row = gtk::ListBoxRow::builder()
                .selectable(true)
                .build();

            let main_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(10)
                .margin_end(10)
                .build();

            // Left side - prefix information
            let info_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .hexpand(true)
                .build();

            // Name and architecture badge
            let name_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(6)
                .build();

            let name_label = gtk::Label::builder()
                .label(&prefix.name)
                .halign(gtk::Align::Start)
                .build();
            name_label.add_css_class("heading");

            let arch_badge = gtk::Label::builder()
                .label(&prefix.config.architecture)
                .halign(gtk::Align::Start)
                .build();
            arch_badge.add_css_class("badge");
            arch_badge.add_css_class("dim-label");

            name_box.append(&name_label);
            name_box.append(&arch_badge);

            info_box.append(&name_box);

            // Wine version and app count
            let details_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .build();

            let wine_version = prefix.config.wine_version.as_deref().unwrap_or("Unknown");
            let version_label = gtk::Label::builder()
                .label(&format!("Wine: {}", wine_version))
                .halign(gtk::Align::Start)
                .build();
            version_label.add_css_class("caption");

            let app_count = prefix.config.registered_executables.len();
            let count_label = gtk::Label::builder()
                .label(&format!("Apps: {}", app_count))
                .halign(gtk::Align::Start)
                .build();
            count_label.add_css_class("caption");

            details_box.append(&version_label);
            details_box.append(&count_label);

            info_box.append(&details_box);

            // Path
            let path_label = gtk::Label::builder()
                .label(&prefix.path.display().to_string())
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::Start)
                .build();
            path_label.add_css_class("caption");
            path_label.add_css_class("dim-label");

            info_box.append(&path_label);

            main_box.append(&info_box);

            // Right side - action buttons
            // let actions_box = gtk::Box::builder()
            //     .orientation(gtk::Orientation::Vertical)
            //     .spacing(4)
            //     .halign(gtk::Align::End)
            //     .build();

            // Details button
            let details_btn = gtk::Button::builder()
                .label("Details")
                .build();
            details_btn.add_css_class("flat");
            details_btn.set_tooltip_text(Some("View prefix details"));

            let sender_clone = sender.clone();
            let details_index = index;
            details_btn.connect_clicked(move |_| {
                sender_clone.input(PrefixListMsg::ShowPrefixDetails(details_index));
            });

            row.set_child(Some(&main_box));

            // Handle row selection
            let sender_clone = sender.clone();
            row.connect_activate(move |_| {
                sender_clone.input(PrefixListMsg::SelectPrefix(index));
            });

            list_box.append(&row);
        }

        if model.prefixes.is_empty() {
            let empty_label = gtk::Label::builder()
                .label("No Wine prefixes found\nClick 'Create New Prefix' to get started")
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .margin_top(40)
                .wrap(true)
                .build();
            empty_label.add_css_class("dim-label");
            empty_label.add_css_class("body");

            let row = gtk::ListBoxRow::builder()
                .selectable(false)
                .child(&empty_label)
                .build();

            list_box.append(&row);
        }
    }
}