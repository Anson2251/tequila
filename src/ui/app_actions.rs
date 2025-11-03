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
}

#[derive(Debug)]
pub enum AppActionsMsg {
    SetSelection(bool),
    SetScanning(bool),
    Launch,
    Add,
    Remove,
}

#[derive(Debug)]
pub enum AppActionsOutput {
    Launch,
    Add,
    Remove,
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
                set_label: "Add",
                #[track = "model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Add);
                },
                add_css_class: "suggested-action",
            },

            gtk::Button {
                set_label: "Remove",
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Remove);
                },
                add_css_class: "destructive-action",
            },

            gtk::Button {
                set_label: "Launch",
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::Launch);
                },
                add_css_class: "suggested-action",
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
            AppActionsMsg::Launch => {
                let _ = sender.output(AppActionsOutput::Launch);
            }
            AppActionsMsg::Add => {
                let _ = sender.output(AppActionsOutput::Add);
            }
            AppActionsMsg::Remove => {
                let _ = sender.output(AppActionsOutput::Remove);
            }
        }
    }
}