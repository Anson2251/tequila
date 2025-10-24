use gtk::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};
use std::path::PathBuf;
use std::fs;

struct AppModel {
    prefixes: Vec<WinePrefix>,
    wine_dir: PathBuf,
    selected_prefix: Option<usize>,
}

#[derive(Debug, Clone)]
struct WinePrefix {
    name: String,
    path: PathBuf,
}

#[derive(Debug)]
enum AppMsg {
    CreatePrefix,
    DeletePrefix(usize),
    LaunchPrefix(usize),
    RefreshPrefixes,
    SelectPrefix(usize),
    UpdateList,
}

impl AppModel {
    fn scan_wine_prefixes(wine_dir: &PathBuf) -> Vec<WinePrefix> {
        let mut prefixes = Vec::new();
        
        if let Ok(entries) = fs::read_dir(wine_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if this directory looks like a Wine prefix
                    let drive_c = path.join("drive_c");
                    let system_reg = path.join("system.reg");
                    
                    if drive_c.exists() && system_reg.exists() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            prefixes.push(WinePrefix {
                                name: name.to_string(),
                                path: path.clone(),
                            });
                        }
                    }
                }
            }
        }
        
        prefixes.sort_by(|a, b| a.name.cmp(&b.name));
        prefixes
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "root"]
        gtk::ApplicationWindow {
            set_title: Some("Tequila - Wine Prefix Manager"),
            set_default_width: 800,
            set_default_height: 600,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,

            

                // Header bar with title and create button
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_margin_bottom: 15,

                    gtk::Label {
                        set_label: "Wine Prefixes",
                        add_css_class: "title-1"
                    },

                    gtk::Box {
                        set_hexpand: true,
                        set_halign: gtk::Align::End,
                        
                        gtk::Button {
                            set_label: "Refresh",
                            connect_clicked => AppMsg::RefreshPrefixes,
                        },
                        
                        gtk::Button {
                            set_label: "Create New Prefix",
                            connect_clicked => AppMsg::CreatePrefix,
                            add_css_class: "suggested-action",
                            set_margin_start: 5
                        }
                    }
                },

                // Main content area
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,

                    // Prefix list
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_hexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        #[name = "prefix_list"]
                        gtk::ListBox {
                            set_css_classes: &["boxed-list"],
                            set_margin_all: 5,
                            // This will trigger view updates when model changes
                            #[track = "model.prefixes.len() > 0"]
                            set_visible: true,
                        }
                    },

                    // Actions panel
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_width_request: 250,

                        gtk::Label {
                            set_label: "Prefix Actions",
                            add_css_class: "heading"
                        },

                        gtk::Button {
                            set_label: "Launch",
                            connect_clicked[sender, selected = model.selected_prefix] => move |_| {
                                if let Some(index) = selected {
                                    sender.input(AppMsg::LaunchPrefix(index));
                                }
                            },
                            set_sensitive: model.selected_prefix.is_some()
                        },

                        gtk::Button {
                            set_label: "Delete",
                            connect_clicked[sender, selected = model.selected_prefix] => move |_| {
                                if let Some(index) = selected {
                                    sender.input(AppMsg::DeletePrefix(index));
                                }
                            },
                            set_sensitive: model.selected_prefix.is_some(),
                            add_css_class: "destructive-action"
                        },

                        gtk::Separator {},
                        
                        gtk::Label {
                            set_label: if let Some(index) = model.selected_prefix {
                                &model.prefixes[index].name
                            } else {
                                "No prefix selected"
                            },
                            set_margin_top: 20,
                            set_wrap: true,
                            set_wrap_mode: gtk::pango::WrapMode::WordChar
                        },

                        gtk::Label {
                            set_label: &if let Some(index) = model.selected_prefix {
                                model.prefixes[index].path.display().to_string()
                            } else {
                                String::new()
                            },
                            add_css_class: "caption",
                            set_wrap: true,
                            set_wrap_mode: gtk::pango::WrapMode::WordChar
                        }
                    }
                },

                // Status bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,
                    set_margin_top: 10,

                    gtk::Label {
                        set_label: &format!("{} prefixes loaded from {}", 
                                          model.prefixes.len(), 
                                          model.wine_dir.display()),
                        add_css_class: "caption"
                    }
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let wine_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Wine");
            
        let prefixes = Self::scan_wine_prefixes(&wine_dir);

        let model = AppModel {
            prefixes,
            wine_dir,
            selected_prefix: None,
        };

        let widgets = view_output!();
        
        #[cfg(not(target_os = "macos"))]
        {
            let header_bar = gtk::HeaderBar::new();
            widgets.root.set_titlebar(Some(&header_bar));
        }
        
        // Initialize the prefix list
        Self::populate_prefix_list(&model, &widgets, &sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::CreatePrefix => {
                // TODO: Implement create prefix dialog
                println!("Create prefix requested");
            }
            AppMsg::DeletePrefix(index) => {
                if index < self.prefixes.len() {
                    let prefix_name = self.prefixes[index].name.clone();
                    self.prefixes.remove(index);
                    if self.selected_prefix == Some(index) {
                        self.selected_prefix = None;
                    } else if let Some(selected) = self.selected_prefix {
                        if selected > index {
                            self.selected_prefix = Some(selected - 1);
                        }
                    }
                    println!("Deleted prefix: {}", prefix_name);
                    sender.input(AppMsg::UpdateList);
                }
            }
            AppMsg::LaunchPrefix(index) => {
                if index < self.prefixes.len() {
                    let prefix_name = self.prefixes[index].name.clone();
                    let prefix_path = self.prefixes[index].path.clone();
                    println!("Launching prefix: {} at {}", prefix_name, prefix_path.display());
                    // TODO: Implement prefix launch
                }
            }
            AppMsg::RefreshPrefixes => {
                println!("Refreshing prefix list");
                self.prefixes = Self::scan_wine_prefixes(&self.wine_dir);
                self.selected_prefix = None;
                sender.input(AppMsg::UpdateList);
            }
            AppMsg::SelectPrefix(index) => {
                self.selected_prefix = Some(index);
                println!("Selected prefix: {}", self.prefixes[index].name);
            }
            AppMsg::UpdateList => {
                // This message is just to trigger view update
            }
        }
    }


}

impl AppModel {
    fn populate_prefix_list(
        model: &AppModel,
        widgets: &<AppModel as SimpleComponent>::Widgets,
        sender: &ComponentSender<AppModel>
    ) {
        // Clear existing items
        while let Some(row) = widgets.prefix_list.first_child() {
            widgets.prefix_list.remove(&row);
        }

        // Add prefixes to the list
        for (_, prefix) in model.prefixes.iter().enumerate() {
            let prefix_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(8)
                .margin_end(8)
                .build();

            let prefix_label = gtk::Label::builder()
                .label(&prefix.name)
                .hexpand(true)
                .halign(gtk::Align::Start)
                .build();

            let path_label = gtk::Label::builder()
                .label(&prefix.path.display().to_string())
                .hexpand(true)
                .halign(gtk::Align::Start)
                .build();
            
            path_label.add_css_class("caption");

            let content_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .build();

            content_box.append(&prefix_label);
            content_box.append(&path_label);
            prefix_box.append(&content_box);

            let row = gtk::ListBoxRow::builder()
                .child(&prefix_box)
                .build();

            widgets.prefix_list.append(&row);
        }

        if model.prefixes.is_empty() {
            let no_prefixes_label = gtk::Label::builder()
                .label("No Wine prefixes found\nMake sure you have Wine prefixes in ~/Wine directory")
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .margin_top(50)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .build();
            
            no_prefixes_label.add_css_class("dim-label");
            
            let row = gtk::ListBoxRow::builder()
                .child(&no_prefixes_label)
                .selectable(false)
                .build();
            
            widgets.prefix_list.append(&row);
        }

        // Connect row selection signal
        let sender_clone = sender.clone();
        widgets.prefix_list.connect_row_activated(move |_, row| {
            let index = row.index();
            if index >= 0 {
                sender_clone.input(AppMsg::SelectPrefix(index as usize));
            }
        });

        // Set initial selection if there's a selected prefix
        if let Some(selected_index) = model.selected_prefix {
            if let Some(row) = widgets.prefix_list.row_at_index(selected_index as i32) {
                widgets.prefix_list.select_row(Some(&row));
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("com.github.tequila");
    app.run::<AppModel>(());
}
