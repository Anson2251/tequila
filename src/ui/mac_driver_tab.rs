use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct MacDriverModel {
    editing: bool,
    mac_allow_vertical_sync: Option<bool>,
    mac_capture_displays: Option<bool>,
    mac_precise_scrolling: Option<bool>,
    mac_retina_mode: Option<bool>,
}

#[derive(Debug)]
pub enum MacDriverMsg {
    SetEditing(bool),
    SetMacDriverSettings {
        allow_vertical_sync: Option<bool>,
        capture_displays: Option<bool>,
        precise_scrolling: Option<bool>,
        retina_mode: Option<bool>,
    },
    UpdateMacAllowVerticalSync(bool),
    UpdateMacCaptureDisplays(bool),
    UpdateMacPreciseScrolling(bool),
    UpdateMacRetinaMode(bool),
}

#[relm4::component(pub)]
impl SimpleComponent for MacDriverModel {
    type Init = (
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
    );
    type Input = MacDriverMsg;
    type Output = MacDriverMsg;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 15,
            set_margin_all: 15,

            gtk::Label {
                set_label: "Mac Driver Settings",
                add_css_class: "title-4",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::CheckButton {
                    set_label: Some("Allow Vertical Sync"),
                    #[track = "model.changed(MacDriverModel::mac_allow_vertical_sync())"]
                    set_active: model.mac_allow_vertical_sync.unwrap_or(false),
                    #[track = "model.changed(MacDriverModel::editing())"]
                    set_sensitive: model.editing,
                    connect_toggled[sender] => move |check| {
                        sender.input(MacDriverMsg::UpdateMacAllowVerticalSync(check.is_active()));
                    },
                },

                gtk::CheckButton {
                    #[watch]
                    set_label: Some(&format!("Capture Displays for Fullscreen {}", model.editing)),
                    #[track = "model.changed(MacDriverModel::mac_capture_displays())"]
                    set_active: model.mac_capture_displays.unwrap_or(false),
                    #[track = "model.changed(MacDriverModel::editing())"]
                    set_sensitive: model.editing,
                    connect_toggled[sender] => move |check| {
                        sender.input(MacDriverMsg::UpdateMacCaptureDisplays(check.is_active()));
                    },
                },

                gtk::CheckButton {
                    set_label: Some("Use Precise Scrolling"),
                    #[track = "model.changed(MacDriverModel::mac_precise_scrolling())"]
                    set_active: model.mac_precise_scrolling.unwrap_or(false),
                    #[track = "model.changed(MacDriverModel::editing())"]
                    set_sensitive: model.editing,
                    connect_toggled[sender] => move |check| {
                        sender.input(MacDriverMsg::UpdateMacPreciseScrolling(check.is_active()));
                    },
                },

                gtk::CheckButton {
                    set_label: Some("Enable Retina Mode"),
                    #[track = "model.changed(MacDriverModel::mac_retina_mode())"]
                    set_active: model.mac_retina_mode.unwrap_or(false),
                    #[track = "model.changed(MacDriverModel::editing())"]
                    set_sensitive: model.editing,
                    connect_toggled[sender] => move |check| {
                        sender.input(MacDriverMsg::UpdateMacRetinaMode(check.is_active()));
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
        let (allow_vertical_sync, capture_displays, precise_scrolling, retina_mode) = init;

        let model = MacDriverModel {
            editing: false,
            mac_allow_vertical_sync: allow_vertical_sync,
            mac_capture_displays: capture_displays,
            mac_precise_scrolling: precise_scrolling,
            mac_retina_mode: retina_mode,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            MacDriverMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            MacDriverMsg::SetMacDriverSettings { allow_vertical_sync, capture_displays, precise_scrolling, retina_mode } => {
                self.set_mac_allow_vertical_sync(allow_vertical_sync);
                self.set_mac_capture_displays(capture_displays);
                self.set_mac_precise_scrolling(precise_scrolling);
                self.set_mac_retina_mode(retina_mode);
            }
            MacDriverMsg::UpdateMacAllowVerticalSync(enabled) => {
                self.set_mac_allow_vertical_sync(Some(enabled));
                let _ = sender.output(MacDriverMsg::UpdateMacAllowVerticalSync(enabled));
            }
            MacDriverMsg::UpdateMacCaptureDisplays(enabled) => {
                self.set_mac_capture_displays(Some(enabled));
                let _ = sender.output(MacDriverMsg::UpdateMacCaptureDisplays(enabled));
            }
            MacDriverMsg::UpdateMacPreciseScrolling(enabled) => {
                self.set_mac_precise_scrolling(Some(enabled));
                let _ = sender.output(MacDriverMsg::UpdateMacPreciseScrolling(enabled));
            }
            MacDriverMsg::UpdateMacRetinaMode(enabled) => {
                self.set_mac_retina_mode(Some(enabled));
                let _ = sender.output(MacDriverMsg::UpdateMacRetinaMode(enabled));
            }
        }
    }
}