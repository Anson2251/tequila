use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct VirtualDesktopModel {
    editing: bool,
    virtual_desktop_enabled: bool,
    virtual_desktop_width: u32,
    virtual_desktop_height: u32,
}

#[derive(Debug)]
pub enum VirtualDesktopMsg {
    SetEditing(bool),
    SetVirtualDesktopSettings {
        enabled: bool,
        width: u32,
        height: u32,
    },
    UpdateVirtualDesktop(bool),
    UpdateVirtualDesktopWidth(String),
    UpdateVirtualDesktopHeight(String),
}

#[relm4::component(pub)]
impl SimpleComponent for VirtualDesktopModel {
    type Init = (bool, u32, u32);
    type Input = VirtualDesktopMsg;
    type Output = VirtualDesktopMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Virtual Desktop",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::CheckButton {
                    set_label: Some("Enable Virtual Desktop"),
                    #[track = "model.changed(VirtualDesktopModel::virtual_desktop_enabled())"]
                    set_active: model.virtual_desktop_enabled,
                    #[track = "model.changed(VirtualDesktopModel::editing())"]
                    set_sensitive: model.editing,
                    connect_toggled[sender] => move |check| {
                        sender.input(VirtualDesktopMsg::UpdateVirtualDesktop(check.is_active()));
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_margin_top: 10,
                    set_sensitive: model.virtual_desktop_enabled,

                    gtk::Label {
                        set_label: "Width:",
                    },

                    gtk::Entry {
                        set_width_chars: 6,
                        #[track = "model.changed(VirtualDesktopModel::virtual_desktop_width())"]
                        set_text: &model.virtual_desktop_width.to_string(),
                        #[track = "model.changed(VirtualDesktopModel::editing())"]
                        set_editable: model.editing,
                        #[track = "model.changed(VirtualDesktopModel::editing())"]
                        set_sensitive: model.editing && model.virtual_desktop_enabled,
                        connect_changed[sender] => move |entry| {
                            sender.input(VirtualDesktopMsg::UpdateVirtualDesktopWidth(entry.text().to_string()));
                        },
                    },

                    gtk::Label {
                        set_label: "Height:",
                    },

                    gtk::Entry {
                        set_width_chars: 6,
                        #[track = "model.changed(VirtualDesktopModel::virtual_desktop_height())"]
                        set_text: &model.virtual_desktop_height.to_string(),
                        #[track = "model.changed(VirtualDesktopModel::editing())"]
                        set_editable: model.editing,
                        #[track = "model.changed(VirtualDesktopModel::editing())"]
                        set_sensitive: model.editing && model.virtual_desktop_enabled,
                        connect_changed[sender] => move |entry| {
                            sender.input(VirtualDesktopMsg::UpdateVirtualDesktopHeight(entry.text().to_string()));
                        },
                    },
                },
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (enabled, width, height) = init;
        
        let model = VirtualDesktopModel {
            editing: false,
            virtual_desktop_enabled: enabled,
            virtual_desktop_width: width,
            virtual_desktop_height: height,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            VirtualDesktopMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            VirtualDesktopMsg::SetVirtualDesktopSettings { enabled, width, height } => {
                self.set_virtual_desktop_enabled(enabled);
                self.set_virtual_desktop_width(width);
                self.set_virtual_desktop_height(height);
            }
            VirtualDesktopMsg::UpdateVirtualDesktop(enabled) => {
                self.set_virtual_desktop_enabled(enabled);
                let _ = sender.output(VirtualDesktopMsg::UpdateVirtualDesktop(enabled));
            }
            VirtualDesktopMsg::UpdateVirtualDesktopWidth(width_str) => {
                if let Ok(width) = width_str.parse::<u32>() {
                    self.set_virtual_desktop_width(width);
                    let _ = sender.output(VirtualDesktopMsg::UpdateVirtualDesktopWidth(width_str));
                }
            }
            VirtualDesktopMsg::UpdateVirtualDesktopHeight(height_str) => {
                if let Ok(height) = height_str.parse::<u32>() {
                    self.set_virtual_desktop_height(height);
                    let _ = sender.output(VirtualDesktopMsg::UpdateVirtualDesktopHeight(height_str));
                }
            }
        }
    }
}