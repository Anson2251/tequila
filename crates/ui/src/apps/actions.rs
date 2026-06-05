use gtk::prelude::*;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use tracker;

#[derive(Debug)]
#[tracker::track]
pub struct AppActionsModel {
    has_selection: bool,
    is_scanning: bool,
    selected_running: bool,
    prefix_set: bool,
    uninstaller_running: bool,
    exe_running: bool,
    has_desktop: bool,
}

#[derive(Debug)]
pub enum AppActionsMsg {
    SetSelection(bool),
    SetScanning(bool),
    SetSelectedRunning(bool),
    SetPrefixSet(bool),
    SetUninstallerRunning(bool),
    SetExeRunning(bool),
    SetDesktopExists(bool),
    Launch,
    Add,
    Remove,
    ShowInfo,
    RunUninstaller,
    RunExe,
    CreateDesktop,
}

#[derive(Debug)]
pub enum AppActionsOutput {
    Launch,
    Kill,
    Add,
    Remove,
    ShowInfo,
    RunUninstaller,
    RunExe,
    CreateDesktop,
}

#[relm4::component(pub, async)]
impl AsyncComponent for AppActionsModel {
    type Init = (bool, bool, bool); // (has_selection, is_scanning, prefix_set)
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

            // Separator
            gtk::Separator {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_start: 5,
                set_margin_end: 5,
            },

            gtk::Button {
                set_tooltip_text: Some("Wine Uninstaller"),
                #[track = "model.changed(AppActionsModel::prefix_set()) || model.changed(AppActionsModel::is_scanning()) || model.changed(AppActionsModel::uninstaller_running())"]
                set_sensitive: model.prefix_set && !model.is_scanning && !model.uninstaller_running,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::RunUninstaller);
                },

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,
                    set_halign: gtk::Align::Center,

                    gtk::Image {
                        set_icon_name: Some("preferences-other-symbolic"),
                        #[track = "model.changed(AppActionsModel::uninstaller_running())"]
                        set_visible: !model.uninstaller_running,
                    },

                    #[name = "uninstaller_spinner"]
                    gtk::Spinner {
                        set_width_request: 16,
                        set_height_request: 16,
                        #[track = "model.changed(AppActionsModel::uninstaller_running())"]
                        set_visible: model.uninstaller_running,
                        #[track = "model.changed(AppActionsModel::uninstaller_running())"]
                        set_spinning: model.uninstaller_running,
                    },
                },
            },

            gtk::Button {
                set_tooltip_text: Some("Run Executable…"),
                #[track = "model.changed(AppActionsModel::prefix_set()) || model.changed(AppActionsModel::is_scanning()) || model.changed(AppActionsModel::exe_running())"]
                set_sensitive: model.prefix_set && !model.is_scanning && !model.exe_running,
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::RunExe);
                },

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,
                    set_halign: gtk::Align::Center,

                    gtk::Image {
                        set_icon_name: Some("document-open-symbolic"),
                        #[track = "model.changed(AppActionsModel::exe_running())"]
                        set_visible: !model.exe_running,
                    },

                    #[name = "exe_spinner"]
                    gtk::Spinner {
                        set_width_request: 16,
                        set_height_request: 16,
                        #[track = "model.changed(AppActionsModel::exe_running())"]
                        set_visible: model.exe_running,
                        #[track = "model.changed(AppActionsModel::exe_running())"]
                        set_spinning: model.exe_running,
                    },
                },
            },

            // Desktop launcher toggle
            gtk::Button {
                #[track = "model.changed(AppActionsModel::has_selection()) || model.changed(AppActionsModel::is_scanning()) || model.changed(AppActionsModel::has_desktop())"]
                set_sensitive: model.has_selection && !model.is_scanning,
                #[track = "model.changed(AppActionsModel::has_desktop())"]
                set_tooltip_text: Some(if model.has_desktop { "Remove Desktop Launcher" } else { "Create Desktop Launcher" }),
                connect_clicked[sender] => move |_| {
                    sender.input(AppActionsMsg::CreateDesktop);
                },

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,
                    set_halign: gtk::Align::Center,

                    gtk::Image {
                        set_icon_name: Some("computer-symbolic"),
                    },
                    gtk::Label {
                        #[track = "model.changed(AppActionsModel::has_desktop())"]
                        set_label: if model.has_desktop { "Delete" } else { "Desktop" },
                    },
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
        let (has_selection, is_scanning, prefix_set) = init;

        let model = AppActionsModel {
            has_selection,
            is_scanning,
            selected_running: false,
            prefix_set,
            uninstaller_running: false,
            exe_running: false,
            has_desktop: false,
            tracker: 0,
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
            AppActionsMsg::SetPrefixSet(prefix_set) => {
                self.set_prefix_set(prefix_set);
            }
            AppActionsMsg::SetUninstallerRunning(running) => {
                self.set_uninstaller_running(running);
            }
            AppActionsMsg::SetExeRunning(running) => {
                self.set_exe_running(running);
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
            AppActionsMsg::RunUninstaller => {
                let _ = sender.output(AppActionsOutput::RunUninstaller);
            }
            AppActionsMsg::RunExe => {
                let _ = sender.output(AppActionsOutput::RunExe);
            }
            AppActionsMsg::SetDesktopExists(exists) => {
                self.set_has_desktop(exists);
            }
            AppActionsMsg::CreateDesktop => {
                let _ = sender.output(AppActionsOutput::CreateDesktop);
            }
        }
    }
}
