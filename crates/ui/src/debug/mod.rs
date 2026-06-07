use gtk::prelude::*;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex};
use std::thread;

/// Level of an output line, used for color highlighting.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputLevel {
    Stdout,
    Stderr,
    Info,
    Warn,
    Error,
}

#[tracker::track]
pub struct DebugWindowModel {
    pub executable_name: String,
    pub process_exited: bool,
    #[tracker::do_not_track]
    pub child: Arc<Mutex<Option<Child>>>,
    #[tracker::do_not_track]
    pub stdin_handle: Arc<Mutex<Option<ChildStdin>>>,
    #[tracker::do_not_track]
    pub buffer: gtk::TextBuffer,
    #[tracker::do_not_track]
    pub scrolled_window: gtk::ScrolledWindow,
}

#[derive(Debug)]
pub enum DebugWindowMsg {
    AppendOutput(String, OutputLevel),
    ProcessExited(i32),
    SendStdin(String),
}

#[derive(Debug)]
pub enum DebugWindowOutput {
    CloseRequest,
}

#[relm4::component(pub, async)]
impl AsyncComponent for DebugWindowModel {
    type Init = (String, Child);
    type Input = DebugWindowMsg;
    type Output = DebugWindowOutput;
    type CommandOutput = ();
    type Widgets = DebugWindowWidgets;

    view! {
        gtk::Window {
            set_default_width: 850,
            set_default_height: 600,
            set_hide_on_close: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 0,

                #[name = "scrolled"]
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hexpand: true,

                    #[name = "output_view"]
                    gtk::TextView {
                        set_editable: false,
                        set_cursor_visible: true,
                        set_monospace: true,
                        set_wrap_mode: gtk::WrapMode::WordChar,

                        set_margin_start: 6,
                        set_margin_end: 6,
                    },
                },

                gtk::Separator {},

                #[name = "input_box"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,
                    set_margin_top: 6,
                    set_margin_bottom: 6,
                    set_margin_start: 6,
                    set_margin_end: 6,

                    #[name = "input_entry"]
                    gtk::Entry {
                        set_hexpand: true,
                        set_placeholder_text: Some("Send input to process stdin..."),
                        #[track = "model.changed(DebugWindowModel::process_exited())"]
                        set_sensitive: !model.process_exited,
                        connect_activate[sender] => move |entry| {
                            let text = entry.text().to_string();
                            if !text.is_empty() {
                                sender.input(DebugWindowMsg::SendStdin(text));
                                entry.set_text("");
                            }
                        },
                    },

                    #[name = "send_btn"]
                    gtk::Button {
                        set_label: "Send",
                        #[track = "model.changed(DebugWindowModel::process_exited())"]
                        set_sensitive: !model.process_exited,
                        connect_clicked[sender, input_entry] => move |_| {
                            let text = input_entry.text().to_string();
                            if !text.is_empty() {
                                sender.input(DebugWindowMsg::SendStdin(text));
                                input_entry.set_text("");
                            }
                        },
                    },
                },
            },

            connect_close_request[sender] => move |_| {
                let _ = sender.output(DebugWindowOutput::CloseRequest);
                gtk::glib::Propagation::Proceed
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (name, mut child) = init;

        // Take pipes before moving child into the model
        let stdin_handle = child.stdin.take();
        let stdout = child.stdout.take().expect("debug launch: stdout must be piped");
        let stderr = child.stderr.take().expect("debug launch: stderr must be piped");

        // Create text buffer with colour tags
        let buffer = gtk::TextBuffer::new(None);
        buffer.create_tag(Some("dim"), &[("foreground", &"#888888".to_string())]);
        buffer.create_tag(Some("stdout"), &[]);
        buffer.create_tag(Some("stderr"), &[("foreground", &"#cc6666".to_string())]);
        buffer.create_tag(Some("warn"), &[
            ("foreground", &"#e5c07b".to_string()),
            ("weight", &700),
        ]);
        buffer.create_tag(Some("error"), &[
            ("foreground", &"#ff4444".to_string()),
            ("weight", &700),
            ("background", &"#330000".to_string()),
        ]);

        // ── Header bar with save button ─────────────────────────────
        let header_bar = gtk::HeaderBar::new();
        #[cfg(target_os = "macos")]
        header_bar.set_property("use-native-controls", true);
        header_bar.set_title_widget(Some(&gtk::Label::new(
            Some(&format!("🐞 Debug: {}", name)),
        )));

        let save_btn = gtk::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Save output to file")
            .build();
        {
            let buf = buffer.clone();
            let win = root.downcast_ref::<gtk::Window>().unwrap().clone();
            save_btn.connect_clicked(move |_| {
                let dialog = gtk::FileDialog::builder()
                    .initial_name("tequila-debug-output.txt")
                    .build();
                let buf = buf.clone();
                dialog.save(Some(&win), None::<&gtk::gio::Cancellable>, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            let text = buf.text(
                                &buf.start_iter(),
                                &buf.end_iter(),
                                false,
                            );
                            let _ = std::fs::write(&path, text.as_str());
                        }
                    }
                });
            });
        }
        #[cfg(target_os = "macos")]
        header_bar.pack_end(&save_btn);
        #[cfg(not(target_os = "macos"))]
        header_bar.pack_start(&save_btn);
        root.set_titlebar(Some(&header_bar));

        let mut model = DebugWindowModel {
            executable_name: name,
            process_exited: false,
            child: Arc::new(Mutex::new(Some(child))),
            stdin_handle: Arc::new(Mutex::new(stdin_handle)),
            buffer: buffer.clone(),
            scrolled_window: gtk::ScrolledWindow::new(),
            tracker: 0,
        };

        let widgets = view_output!();

        // Wire up the newly created scrolled window from view! into the model
        model.scrolled_window = widgets.scrolled.clone();

        widgets.output_view.set_buffer(Some(&buffer));

        // Show the window
        root.present();

        // ── Background threads ─────────────────────────────────────────
        let sender_clone = sender.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        let level = detect_level(&text).unwrap_or(OutputLevel::Stdout);
                        let _ = sender_clone.input(DebugWindowMsg::AppendOutput(text, level));
                    }
                    Err(_) => break,
                }
            }
        });

        let sender_clone = sender.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        let level = detect_level(&text).unwrap_or(OutputLevel::Stdout);
                        let _ = sender_clone.input(DebugWindowMsg::AppendOutput(text, level));
                    }
                    Err(_) => break,
                }
            }
        });

        let sender_clone = sender.clone();
        let child_arc = model.child.clone();
        thread::spawn(move || {
            let mut guard = child_arc.lock().unwrap();
            if let Some(ref mut child) = *guard {
                let status = child.wait();
                let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                let _ = sender_clone.input(DebugWindowMsg::ProcessExited(code));
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.reset();
        match msg {
            DebugWindowMsg::AppendOutput(text, level) => {
                let tag_name = match level {
                    OutputLevel::Info | OutputLevel::Stdout => "stdout",
                    OutputLevel::Stderr => "stderr",
                    OutputLevel::Warn => "warn",
                    OutputLevel::Error => "error",
                };

                let mut end = self.buffer.end_iter();
                if let Some(tag) = self.buffer.tag_table().lookup(tag_name) {
                    self.buffer
                        .insert_with_tags(&mut end, &format!("{}\n", text), &[&tag]);
                } else {
                    self.buffer.insert(&mut end, &format!("{}\n", text));
                }

                // Auto-scroll to bottom
                let vadj = self.scrolled_window.vadjustment();
                vadj.set_value(vadj.upper());
            }
            DebugWindowMsg::ProcessExited(code) => {
                self.set_process_exited(true);
                let mut end = self.buffer.end_iter();
                let msg = if code == 0 {
                    format!("✓ Process exited with code {}\n", code)
                } else {
                    format!("✗ Process exited with code {}\n", code)
                };
                if let Some(tag) = self.buffer.tag_table().lookup("dim") {
                    self.buffer.insert_with_tags(&mut end, &msg, &[&tag]);
                } else {
                    self.buffer.insert(&mut end, &msg);
                }

                let vadj = self.scrolled_window.vadjustment();
                vadj.set_value(vadj.upper());
            }
            DebugWindowMsg::SendStdin(text) => {
                if let Some(stdin) = self.stdin_handle.lock().unwrap().as_mut() {
                    let _ = writeln!(stdin, "{}", text);
                    let _ = stdin.flush();
                }

                // Echo input to the output view
                let mut end = self.buffer.end_iter();
                if let Some(tag) = self.buffer.tag_table().lookup("dim") {
                    self.buffer
                        .insert_with_tags(&mut end, &format!("> {}\n", text), &[&tag]);
                } else {
                    self.buffer.insert(&mut end, &format!("> {}\n", text));
                }

                let vadj = self.scrolled_window.vadjustment();
                vadj.set_value(vadj.upper());
            }
        }
    }
}

/// Heuristic level detection: scans a line for common Wine/Rust log markers.
fn detect_level(line: &str) -> Option<OutputLevel> {
    let lower = line.to_lowercase();
    if lower.contains("[error]") || lower.contains("err:") {
        Some(OutputLevel::Error)
    } else if lower.contains("[warn]") || lower.contains("warn:") {
        Some(OutputLevel::Warn)
    } else if lower.contains("[info]") {
        Some(OutputLevel::Info)
    } else {
        None
    }
}
