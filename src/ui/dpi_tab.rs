use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct DpiModel {
    editing: bool,
    log_pixels: Option<u32>,
}

#[derive(Debug)]
pub enum DpiMsg {
    SetEditing(bool),
    SetDpiSettings {
        log_pixels: Option<u32>,
    },
    UpdateLogPixels(String),
}

#[relm4::component(pub)]
impl SimpleComponent for DpiModel {
    type Init = Option<u32>;
    type Input = DpiMsg;
    type Output = DpiMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Display Settings",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Frame {
                set_label: Some("DPI Settings"),
                set_margin_top: 10,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    set_margin_all: 10,

                    gtk::Label {
                        set_label: "LogPixels (DPI):",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 10,

                        gtk::SpinButton {
                            #[track = "model.changed(DpiModel::log_pixels())"]
                            set_value: model.log_pixels.unwrap_or(96) as f64,
                            set_adjustment: &gtk::Adjustment::builder()
                                .lower(96.0)
                                .upper(480.0)
                                .step_increment(1.0)
                                .page_increment(24.0)
                                .value(model.log_pixels.unwrap_or(96) as f64)
                                .build(),
                            set_width_chars: 5,
                            #[track = "model.changed(DpiModel::editing())"]
                            set_sensitive: model.editing,
                            connect_changed[sender] => move |spin| {
                                sender.input(DpiMsg::UpdateLogPixels(spin.text().to_string()));
                            },
                        },

                        gtk::Label {
                            set_label: "(96 = 100%, 120 = 125%, 144 = 150%)",
                            add_css_class: "caption",
                            set_halign: gtk::Align::Start,
                        },
                    },

                    gtk::Label {
                        set_label: "Controls the system DPI scaling. Higher values make text and UI elements larger.",
                        set_wrap: true,
                        add_css_class: "caption",
                        set_margin_top: 5,
                    },

                    gtk::Label {
                        set_label: "Common values:",
                        set_halign: gtk::Align::Start,
                        set_margin_top: 10,
                    },

                    gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        gtk::Label {
                            set_label: "96 DPI:",
                            set_halign: gtk::Align::End,
                            set_width_request: 80,
                        },
                        gtk::Label {
                            set_label: "100% (Default)",
                            add_css_class: "caption",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        gtk::Label {
                            set_label: "120 DPI:",
                            set_halign: gtk::Align::End,
                            set_width_request: 80,
                        },
                        gtk::Label {
                            set_label: "125% scaling",
                            add_css_class: "caption",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        gtk::Label {
                            set_label: "144 DPI:",
                            set_halign: gtk::Align::End,
                            set_width_request: 80,
                        },
                        gtk::Label {
                            set_label: "150% scaling",
                            add_css_class: "caption",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 15,

                        gtk::Label {
                            set_label: "192 DPI:",
                            set_halign: gtk::Align::End,
                            set_width_request: 80,
                        },
                        gtk::Label {
                            set_label: "200% scaling",
                            add_css_class: "caption",
                        },
                    },
                },
                }
            },

            gtk::Frame {
                set_label: Some("Important Notes"),
                set_margin_top: 10,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    set_margin_all: 10,

                    gtk::Label {
                        set_label: "• Changes require application restart to take effect",
                        set_wrap: true,
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Label {
                        set_label: "• Some applications may not respect DPI settings",
                        set_wrap: true,
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Label {
                        set_label: "• Very high DPI values may cause UI issues in older applications",
                        set_wrap: true,
                        set_halign: gtk::Align::Start,
                    },
                }
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let log_pixels = init;

        let model = DpiModel {
            editing: false,
            log_pixels,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            DpiMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            DpiMsg::SetDpiSettings { log_pixels } => {
                self.set_log_pixels(log_pixels);
            }
            DpiMsg::UpdateLogPixels(value_str) => {
                if let Ok(value) = value_str.parse::<u32>() {
                    if value >= 96 && value <= 480 {
                        self.set_log_pixels(Some(value));
                        let _ = sender.output(DpiMsg::UpdateLogPixels(value.to_string()));
                    }
                }
            }
        }
    }
}