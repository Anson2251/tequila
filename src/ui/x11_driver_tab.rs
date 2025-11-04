use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, gtk, view};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct X11DriverModel {
    editing: bool,
    decorated: Option<bool>,
    client_side_graphics: Option<bool>,
    client_side_with_render: Option<bool>,
    client_side_antialias_with_render: Option<bool>,
    client_side_antialias_with_core: Option<bool>,
    grab_fullscreen: Option<bool>,
    grab_pointer: Option<bool>,
    managed: Option<bool>,
    use_xrandr: Option<bool>,
    use_xvid_mode: Option<bool>,
}

#[derive(Debug)]
pub enum X11DriverMsg {
    SetEditing(bool),
    SetX11DriverSettings {
        decorated: Option<bool>,
        client_side_graphics: Option<bool>,
        client_side_with_render: Option<bool>,
        client_side_antialias_with_render: Option<bool>,
        client_side_antialias_with_core: Option<bool>,
        grab_fullscreen: Option<bool>,
        grab_pointer: Option<bool>,
        managed: Option<bool>,
        use_xrandr: Option<bool>,
        use_xvid_mode: Option<bool>,
    },
    UpdateDecorated(bool),
    UpdateClientSideGraphics(bool),
    UpdateClientSideWithRender(bool),
    UpdateClientSideAntialiasWithRender(bool),
    UpdateClientSideAntialiasWithCore(bool),
    UpdateGrabFullscreen(bool),
    UpdateGrabPointer(bool),
    UpdateManaged(bool),
    UpdateUseXRandR(bool),
    UpdateUseXVidMode(bool),
}

#[relm4::component(pub)]
impl SimpleComponent for X11DriverModel {
    type Init = (
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
    );
    type Input = X11DriverMsg;
    type Output = X11DriverMsg;

    view! {
        #[root]
        gtk::ScrolledWindow {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 15,
                set_margin_all: 15,

                gtk::Label {
                    set_label: "X11 Driver Settings",
                    add_css_class: "title-4",
                    set_halign: gtk::Align::Start,
                },

                gtk::Frame {
                    set_label: Some("Window Management"),
                    set_margin_top: 10,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_margin_all: 10,

                        gtk::CheckButton {
                            set_label: Some("Decorated Windows"),
                            #[track = "model.changed(X11DriverModel::decorated())"]
                            set_active: model.decorated.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateDecorated(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Managed by Window Manager"),
                            #[track = "model.changed(X11DriverModel::managed())"]
                            set_active: model.managed.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateManaged(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Grab Pointer"),
                            #[track = "model.changed(X11DriverModel::grab_pointer())"]
                            set_active: model.grab_pointer.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateGrabPointer(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Grab Fullscreen"),
                            #[track = "model.changed(X11DriverModel::grab_fullscreen())"]
                            set_active: model.grab_fullscreen.unwrap_or(false),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateGrabFullscreen(check.is_active()));
                            },
                        },
                    }
                },

                gtk::Frame {
                    set_label: Some("Rendering"),
                    set_margin_top: 10,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_margin_all: 10,

                        gtk::CheckButton {
                            set_label: Some("Client Side Graphics"),
                            #[track = "model.changed(X11DriverModel::client_side_graphics())"]
                            set_active: model.client_side_graphics.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateClientSideGraphics(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Client Side With Render"),
                            #[track = "model.changed(X11DriverModel::client_side_with_render())"]
                            set_active: model.client_side_with_render.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateClientSideWithRender(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Client Side Anti-Alias With Render"),
                            #[track = "model.changed(X11DriverModel::client_side_antialias_with_render())"]
                            set_active: model.client_side_antialias_with_render.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateClientSideAntialiasWithRender(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Client Side Anti-Alias With Core"),
                            #[track = "model.changed(X11DriverModel::client_side_antialias_with_core())"]
                            set_active: model.client_side_antialias_with_core.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateClientSideAntialiasWithCore(check.is_active()));
                            },
                        },
                    }
                },

                gtk::Frame {
                    set_label: Some("Display Management"),
                    set_margin_top: 10,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_margin_all: 10,

                        gtk::CheckButton {
                            set_label: Some("Use XRandR"),
                            #[track = "model.changed(X11DriverModel::use_xrandr())"]
                            set_active: model.use_xrandr.unwrap_or(true),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateUseXRandR(check.is_active()));
                            },
                        },

                        gtk::CheckButton {
                            set_label: Some("Use XVidMode"),
                            #[track = "model.changed(X11DriverModel::use_xvid_mode())"]
                            set_active: model.use_xvid_mode.unwrap_or(false),
                            #[track = "model.changed(X11DriverModel::editing())"]
                            set_sensitive: model.editing,
                            connect_toggled[sender] => move |check| {
                                sender.input(X11DriverMsg::UpdateUseXVidMode(check.is_active()));
                            },
                        },
                    }
                },
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (decorated, client_side_graphics, client_side_with_render, client_side_antialias_with_render,
             client_side_antialias_with_core, grab_fullscreen, grab_pointer, managed, use_xrandr, use_xvid_mode, _extra_param) = init;

        let model = X11DriverModel {
            editing: false,
            decorated,
            client_side_graphics,
            client_side_with_render,
            client_side_antialias_with_render,
            client_side_antialias_with_core,
            grab_fullscreen,
            grab_pointer,
            managed,
            use_xrandr,
            use_xvid_mode,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        self.reset();
        match msg {
            X11DriverMsg::SetEditing(editing) => {
                self.set_editing(editing);
            }
            X11DriverMsg::SetX11DriverSettings { decorated, client_side_graphics, client_side_with_render, client_side_antialias_with_render, client_side_antialias_with_core, grab_fullscreen, grab_pointer, managed, use_xrandr, use_xvid_mode } => {
                self.set_decorated(decorated);
                self.set_client_side_graphics(client_side_graphics);
                self.set_client_side_with_render(client_side_with_render);
                self.set_client_side_antialias_with_render(client_side_antialias_with_render);
                self.set_client_side_antialias_with_core(client_side_antialias_with_core);
                self.set_grab_fullscreen(grab_fullscreen);
                self.set_grab_pointer(grab_pointer);
                self.set_managed(managed);
                self.set_use_xrandr(use_xrandr);
                self.set_use_xvid_mode(use_xvid_mode);
            }
            X11DriverMsg::UpdateDecorated(enabled) => {
                self.set_decorated(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateDecorated(enabled));
            }
            X11DriverMsg::UpdateClientSideGraphics(enabled) => {
                self.set_client_side_graphics(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateClientSideGraphics(enabled));
            }
            X11DriverMsg::UpdateClientSideWithRender(enabled) => {
                self.set_client_side_with_render(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateClientSideWithRender(enabled));
            }
            X11DriverMsg::UpdateClientSideAntialiasWithRender(enabled) => {
                self.set_client_side_antialias_with_render(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateClientSideAntialiasWithRender(enabled));
            }
            X11DriverMsg::UpdateClientSideAntialiasWithCore(enabled) => {
                self.set_client_side_antialias_with_core(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateClientSideAntialiasWithCore(enabled));
            }
            X11DriverMsg::UpdateGrabFullscreen(enabled) => {
                self.set_grab_fullscreen(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateGrabFullscreen(enabled));
            }
            X11DriverMsg::UpdateGrabPointer(enabled) => {
                self.set_grab_pointer(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateGrabPointer(enabled));
            }
            X11DriverMsg::UpdateManaged(enabled) => {
                self.set_managed(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateManaged(enabled));
            }
            X11DriverMsg::UpdateUseXRandR(enabled) => {
                self.set_use_xrandr(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateUseXRandR(enabled));
            }
            X11DriverMsg::UpdateUseXVidMode(enabled) => {
                self.set_use_xvid_mode(Some(enabled));
                let _ = sender.output(X11DriverMsg::UpdateUseXVidMode(enabled));
            }
        }
    }
}