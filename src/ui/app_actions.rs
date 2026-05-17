use relm4::{
    gtk,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
};
use gtk::prelude::*;
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AppActionsModel {
    has_selection: bool,
    is_scanning: bool,
    selected_running: bool,
}

#[derive(Debug)]
pub enum AppActionsMsg {
    SetSelection(bool),
    SetScanning(bool),
    SetSelectedRunning(bool),
    Launch,
    Add,
    Remove,
    ShowInfo,
}

#[derive(Debug)]
pub enum AppActionsOutput {
    Launch,
    Kill,
    Add,
    Remove,
    ShowInfo,
}

#[relm4::component(pub, async)]
impl AsyncComponent for AppActionsModel {
    type Init = (bool, bool); // (has_selection, is_scanning)
    type Input = AppActionsMsg;
    type Output = AppActionsOutput;
    type CommandOutput = ();
    type Widgets = AppActionsWidgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 10,
            set_halign: gtk::Align::End,
            set_margin_top: 10,

            #[name = "add_button"]
            gtk::Button {
                set_icon_name: "list-add-symbolic",
                set_tooltip_text: Some("Add Application"),
                #[track = "model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Add);
                },
                add_css_class: "suggested-action",
            },

            gtk::Button {
                set_icon_name: "user-trash-symbolic",
                set_tooltip_text: Some("Remove Application"),
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Remove);
                },
                add_css_class: "destructive-action",
            },

            gtk::Button {
                set_icon_name: "dialog-information-symbolic",
                set_tooltip_text: Some("Application Info"),
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::ShowInfo);
                },
            },

            #[name = "launch_btn"]
            gtk::Button {
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning()) || model.changed(AppActionsModel::selected_running())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                #[track = "model.changed(AppActionsModel::selected_running())"]
                set_tooltip_text: Some(if model.selected_running { "Kill" } else { "Launch" }),
                #[track = "model.changed(AppActionsModel::selected_running())"]
                set_icon_name: if model.selected_running { "media-playback-stop-symbolic" } else { "media-playback-start-symbolic" },
                #[track = "model.changed(AppActionsModel::selected_running())"]
                set_css_classes: if model.selected_running {
                    &["destructive-action"]
                } else {
                    &["suggested-action"]
                },
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Launch);
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (has_selection, is_scanning) = init;

        let model = AppActionsModel {
            has_selection,
            is_scanning,
            selected_running: false,
            tracker: 0
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.reset();
        match msg {
            AppActionsMsg::SetSelection(has_selection) => {
                self.set_has_selection(has_selection);
            }
            AppActionsMsg::SetScanning(is_scanning) => {
                self.set_is_scanning(is_scanning);
            }
            AppActionsMsg::SetSelectedRunning(running) => {
                self.set_selected_running(running);
            }
            AppActionsMsg::Launch => {
                if self.selected_running {
                    let _ = sender.output(AppActionsOutput::Kill);
                } else {
                    let _ = sender.output(AppActionsOutput::Launch);
                }
            }
            AppActionsMsg::Add => {
                let _ = sender.output(AppActionsOutput::Add);
            }
            AppActionsMsg::Remove => {
                let _ = sender.output(AppActionsOutput::Remove);
            }
            AppActionsMsg::ShowInfo => {
                let _ = sender.output(AppActionsOutput::ShowInfo);
            }
        }
    }
}
