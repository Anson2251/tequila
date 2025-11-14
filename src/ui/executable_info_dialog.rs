use relm4::{
    gtk, RelmWidgetExt,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    view
};
use gtk::prelude::*;
use crate::prefix::config::RegisteredExecutable;

#[derive(Debug)]
#[tracker::track]
pub struct ExecutableInfoDialogModel {
    executable: Option<RegisteredExecutable>,
    visible: bool,
}

#[derive(Debug)]
pub enum ExecutableInfoDialogMsg {
    ShowInfo(RegisteredExecutable),
    Hide,
}

#[relm4::component(pub, async)]
impl AsyncComponent for ExecutableInfoDialogModel {
    type Init = ();
    type Input = ExecutableInfoDialogMsg;
    type Output = ();
    type CommandOutput = ();
    type Widgets = ExecutableInfoDialogWidgets;

    view! {
        gtk::Window {
            set_title: Some("Executable Information"),
            set_default_width: 500,
            set_default_height: 600,
            set_modal: true,
            set_resizable: true,
            #[watch]
            set_visible: model.visible,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_margin_all: 20,

                // Header with icon and name
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 15,
                    set_margin_bottom: 15,

                    gtk::Image {
                        set_pixel_size: 64,
                        #[watch]
                        set_from_file: model.executable.as_ref().and_then(|e| e.icon_path.as_ref()),
                        set_icon_name: Some("application-x-executable"),
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,

                        gtk::Label {
                            #[watch]
                            set_label: model.executable.as_ref().map(|e| e.name.as_str()).unwrap_or(""),
                            add_css_class: "title-2",
                            set_halign: gtk::Align::Start,
                            set_wrap: true,
                            set_wrap_mode: gtk::pango::WrapMode::WordChar,
                        },

                        gtk::Label {
                            #[watch]
                            set_label: model.executable.as_ref()
                                .and_then(|e| e.description.as_deref())
                                .unwrap_or("No description available"),
                            add_css_class: "body",
                            set_halign: gtk::Align::Start,
                            set_wrap: true,
                            set_wrap_mode: gtk::pango::WrapMode::WordChar,
                        },
                    }
                },

                gtk::Separator {},

                // Information sections in a scrolled window
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                    set_min_content_height: 400,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 20,
                        set_margin_all: 10,

                        // File Information Section
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,

                            gtk::Label {
                                set_label: "File Information",
                                add_css_class: "heading",
                                set_halign: gtk::Align::Start,
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                set_margin_start: 20,

                                // Executable Path
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 15,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "Path:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                        set_width_request: 120,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.executable.as_ref()
                                            .map(|e| e.executable_path.display().to_string())
                                            .unwrap_or_else(|| "N/A".to_string()),
                                        add_css_class: "monospace",
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                        set_ellipsize: gtk::pango::EllipsizeMode::Middle,
                                        set_max_width_chars: 40,
                                    },
                                },

                                // File Version
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 15,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "File Version:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                        set_width_request: 120,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.executable.as_ref()
                                            .and_then(|e| e.file_version.as_deref())
                                            .unwrap_or("N/A"),
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                    },
                                },

                                // Product Version
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 15,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "Product Version:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                        set_width_request: 120,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.executable.as_ref()
                                            .and_then(|e| e.product_version.as_deref())
                                            .unwrap_or("N/A"),
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                    },
                                },

                                // Company Name
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 15,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "Company:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                        set_width_request: 120,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.executable.as_ref()
                                            .and_then(|e| e.company_name.as_deref())
                                            .unwrap_or("N/A"),
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                    },
                                },

                                // Product Name
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 15,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "Product:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                        set_width_request: 120,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.executable.as_ref()
                                            .and_then(|e| e.product_name.as_deref())
                                            .unwrap_or("N/A"),
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                    },
                                },

                                // File Description
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 5,
                                    set_margin_bottom: 5,

                                    gtk::Label {
                                        set_label: "File Description:",
                                        add_css_class: "caption",
                                        set_halign: gtk::Align::Start,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.executable.as_ref()
                                            .and_then(|e| e.file_description.as_deref())
                                            .unwrap_or("N/A"),
                                        set_halign: gtk::Align::Start,
                                        set_selectable: true,
                                        set_wrap: true,
                                        set_wrap_mode: gtk::pango::WrapMode::WordChar,
                                        set_max_width_chars: 40,
                                        set_margin_start: 20,
                                    },
                                },
                            },
                        },

                        // Imported Modules Section
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            #[watch]
                            set_visible: model.executable.as_ref()
                                .map(|e| !e.imported_modules.is_empty())
                                .unwrap_or(false),

                            gtk::Label {
                                set_label: "Imported Modules",
                                add_css_class: "heading",
                                set_halign: gtk::Align::Start,
                            },

                            gtk::ScrolledWindow {
                                set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Automatic),
                                set_min_content_height: 150,
                                set_max_content_height: 200,

                                gtk::TextView {
                                    set_editable: false,
                                    set_cursor_visible: false,
                                    set_margin_start: 20,
                                    set_buffer: Some(&gtk::TextBuffer::builder()
                                        .text(model.executable.as_ref()
                                            .map(|e| e.imported_modules.join("\n"))
                                            .unwrap_or_else(|| "".to_string()))
                                        .build()),
                                },
                            },
                        },
                    },
                },

                gtk::Separator {},

                // Close button
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_halign: gtk::Align::End,
                    set_margin_top: 10,

                    gtk::Button {
                        set_label: "Close",
                        add_css_class: "suggested-action",
                        connect_clicked[sender] => move |_| {
                            sender.input(ExecutableInfoDialogMsg::Hide);
                        },
                    },
                },
            },

            connect_close_request[sender] => move |_| {
                sender.input(ExecutableInfoDialogMsg::Hide);
                gtk::glib::Propagation::Stop
            },
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = ExecutableInfoDialogModel {
            executable: None,
            visible: false,
            tracker: 0,
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _widgets: &gtk::Window,
    ) {
        self.reset();
        match msg {
            ExecutableInfoDialogMsg::ShowInfo(executable) => {
                self.set_executable(Some(executable));
                self.set_visible(true);
            }
            ExecutableInfoDialogMsg::Hide => {
                self.set_visible(false);
            }
        }
    }
}