use crate::app::AppMsg;
use adw::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use gtk4::gio;
use relm4::{ComponentSender, adw, gtk};
use std::sync::OnceLock;

/// Configure the application menu bar with platform-appropriate menus.
///
/// On macOS, creates a native NSMenu bar with AppKit APIs.
/// On Linux, uses GTK's gio::Menu with set_menubar.
pub fn setup_menu_bar(app: gtk::Application, sender: ComponentSender<crate::app::AppModel>) {
    // Register shared actions so keyboard shortcuts work regardless of menu system
    register_menu_actions(&app, &sender);

    #[cfg(target_os = "macos")]
    setup_macos_native_menu(&app, sender);

    #[cfg(not(target_os = "macos"))]
    {
        use gtk::gio::Menu;

        let menubar = Menu::new();

        let file_menu = Menu::new();
        file_menu.append(Some("_New Prefix"), Some("app.new-prefix"));
        file_menu.append(Some("_Preferences"), Some("app.preferences"));
        file_menu.append(Some("_Quit"), Some("app.quit"));
        menubar.append_submenu(Some("_File"), &file_menu);

        let view_menu = Menu::new();
        view_menu.append(Some("Toggle _Sidebar"), Some("app.toggle-sidebar"));
        menubar.append_submenu(Some("_View"), &view_menu);

        app.set_menubar(Some(&menubar));
    }
}

/// Register GIO actions so keyboard shortcuts like Cmd+N, Cmd+, etc. work.
/// These actions are used by both the gio::Menu (Linux) and native NSMenu (macOS).
fn register_menu_actions(app: &gtk::Application, sender: &ComponentSender<crate::app::AppModel>) {
    use gtk::gio::SimpleAction;

    let new_prefix_action = SimpleAction::new("new-prefix", None);
    let s = sender.clone();
    new_prefix_action.connect_activate(move |_, _| {
        s.input(AppMsg::ShowCreatePrefixDialog);
    });
    app.add_action(&new_prefix_action);
    app.set_accels_for_action("app.new-prefix", &["<primary>n"]);

    let preferences_action = SimpleAction::new("preferences", None);
    let s = sender.clone();
    preferences_action.connect_activate(move |_, _| {
        s.input(AppMsg::ShowSettings);
    });
    app.add_action(&preferences_action);
    app.set_accels_for_action("app.preferences", &["<primary>comma"]);

    let toggle_sidebar_action = SimpleAction::new("toggle-sidebar", None);
    let s = sender.clone();
    toggle_sidebar_action.connect_activate(move |_, _| {
        s.input(AppMsg::ToggleSidebar);
    });
    app.add_action(&toggle_sidebar_action);
    app.set_accels_for_action("app.toggle-sidebar", &["<primary>backslash"]);

    let app_quit = app.clone();
    let quit_action = SimpleAction::new("quit", None);
    quit_action.connect_activate(move |_, _| {
        app_quit.quit();
    });
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<primary>q"]);
}

// ── macOS native menu (NSMenu / NSMenuItem) ──────────────────────────────

#[cfg(target_os = "macos")]
static MENU_CALLBACK: OnceLock<Box<dyn Fn(AppMsg) + Send + Sync>> = OnceLock::new();

// Must be kept alive for the lifetime of the app — menu items hold a reference to this target.
#[cfg(target_os = "macos")]
static MENU_TARGET: OnceLock<objc2::rc::Retained<TequilaMenuHandler>> = OnceLock::new();

#[cfg(target_os = "macos")]
// Objective-C class that acts as the target for native NSMenuItem actions.
// Uses a global callback to dispatch AppMsg values to the component.
objc2::define_class!(
    #[unsafe(super(objc2::runtime::NSObject))]
    #[name = "TequilaMenuHandler"]
    struct TequilaMenuHandler;

    impl TequilaMenuHandler {
        #[unsafe(method(handleMenuAction:))]
        fn handle_menu_action(&self, sender: &objc2::runtime::NSObject) {
            use objc2::msg_send;
            let tag: isize = unsafe { msg_send![sender, tag] };
            match tag {
                1 => {
                    let about = adw::AboutDialog::new();
                    about.set_application_name("Tequila");
                    about.set_application_icon("com.github.anson2251.tequila");
                    about.set_version("0.1.0");
                    about.set_comments("Wine Prefix Manager");
                    about.set_developer_name("Anson2251");
                    let parent = gio::Application::default()
                        .and_then(|a| a.downcast::<gtk::Application>().ok())
                        .and_then(|app| app.active_window());
                    about.present(parent.as_ref());
                }
                2 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ShowCreatePrefixDialog);
                    }
                }
                3 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ShowSettings);
                    }
                }
                4 => {
                    if let Some(cb) = MENU_CALLBACK.get() {
                        cb(AppMsg::ToggleSidebar);
                    }
                }
                5 => {
                    if let Some(gio_app) = gio::Application::default() {
                        if let Ok(gtk_app) = gio_app.downcast::<gtk::Application>() {
                            gtk_app.quit();
                        }
                    }
                }
                _ => {}
            }
        }
    }
);

#[cfg(target_os = "macos")]
impl TequilaMenuHandler {
    objc2::extern_methods!(
        #[unsafe(method(new))]
        fn new() -> objc2::rc::Retained<Self>;
    );
}

/// Create a native macOS menu bar using NSMenu / NSMenuItem.
#[cfg(target_os = "macos")]
fn setup_macos_native_menu(_app: &gtk::Application, sender: ComponentSender<crate::app::AppModel>) {
    use objc2::runtime::NSObject;
    use objc2::{MainThreadMarker, sel};
    use objc2_app_kit::{NSApp, NSEventModifierFlags, NSMenu, NSMenuItem};
    use objc2_foundation::NSString;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    // Set up a channel: native menu callbacks send AppMsg through this,
    // and a glib timeout on the main thread polls it and forwards to the component.
    let s = sender.clone();
    let (tx, rx) = std::sync::mpsc::channel::<AppMsg>();
    MENU_CALLBACK
        .set(Box::new(move |msg| {
            let _ = tx.send(msg);
        }))
        .ok();

    // Poll the channel every 50ms on the GTK main loop
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(msg) = rx.try_recv() {
            s.input(msg);
        }
        glib::ControlFlow::Continue
    });

    // Store the target permanently — menu items hold references to it
    MENU_TARGET.set(TequilaMenuHandler::new()).ok();
    let target = MENU_TARGET.get().expect("Menu target should be set");

    unsafe {
        let main_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        main_menu.setTitle(&NSString::from_str("MainMenu"));

        // ── App Menu ──
        let app_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let app_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        app_menu.setTitle(&NSString::from_str("AppMenu"));
        app_menu_item.setSubmenu(Some(&app_menu));
        main_menu.addItem(&app_menu_item);

        // About
        let about_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        about_item.setTitle(&NSString::from_str("About Tequila"));
        about_item.setAction(Some(sel!(handleMenuAction:)));
        about_item.setTarget(Some(&*target as &NSObject));
        about_item.setTag(1);
        app_menu.addItem(&about_item);

        // Separator
        let sep1 = NSMenuItem::separatorItem(mtm);
        app_menu.addItem(&sep1);

        // Preferences
        let prefs_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        prefs_item.setTitle(&NSString::from_str("Preferences\u{2026}"));
        prefs_item.setAction(Some(sel!(handleMenuAction:)));
        prefs_item.setTarget(Some(&*target as &NSObject));
        prefs_item.setTag(3);
        prefs_item.setKeyEquivalent(&NSString::from_str(","));
        prefs_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        app_menu.addItem(&prefs_item);

        // Separator
        let sep2 = NSMenuItem::separatorItem(mtm);
        app_menu.addItem(&sep2);

        // Quit
        let quit_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        quit_item.setTitle(&NSString::from_str("Quit Tequila"));
        quit_item.setAction(Some(sel!(handleMenuAction:)));
        quit_item.setTarget(Some(&*target as &NSObject));
        quit_item.setTag(5);
        quit_item.setKeyEquivalent(&NSString::from_str("q"));
        quit_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        app_menu.addItem(&quit_item);

        // ── File Menu ──
        let file_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let file_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        file_menu.setTitle(&NSString::from_str("File"));
        file_menu_item.setSubmenu(Some(&file_menu));
        main_menu.addItem(&file_menu_item);

        let new_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        new_item.setTitle(&NSString::from_str("New Prefix"));
        new_item.setAction(Some(sel!(handleMenuAction:)));
        new_item.setTarget(Some(&*target as &NSObject));
        new_item.setTag(2);
        new_item.setKeyEquivalent(&NSString::from_str("n"));
        new_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        file_menu.addItem(&new_item);

        // ── View Menu ──
        let view_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let view_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        view_menu.setTitle(&NSString::from_str("View"));
        view_menu_item.setSubmenu(Some(&view_menu));
        main_menu.addItem(&view_menu_item);

        let sidebar_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        sidebar_item.setTitle(&NSString::from_str("Toggle Sidebar"));
        sidebar_item.setAction(Some(sel!(handleMenuAction:)));
        sidebar_item.setTarget(Some(&*target as &NSObject));
        sidebar_item.setTag(4);
        sidebar_item.setKeyEquivalent(&NSString::from_str("\\"));
        sidebar_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        );
        view_menu.addItem(&sidebar_item);

        // ── Edit Menu (responder-chain with nil target) ──
        let edit_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let edit_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        edit_menu.setTitle(&NSString::from_str("Edit"));
        edit_menu_item.setSubmenu(Some(&edit_menu));
        main_menu.addItem(&edit_menu_item);

        // Undo
        let undo_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        undo_item.setTitle(&NSString::from_str("Undo"));
        undo_item.setAction(Some(sel!(undo:)));
        undo_item.setTarget(None);
        undo_item.setKeyEquivalent(&NSString::from_str("z"));
        undo_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&undo_item);

        // Redo
        let redo_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        redo_item.setTitle(&NSString::from_str("Redo"));
        redo_item.setAction(Some(sel!(redo:)));
        redo_item.setTarget(None);
        redo_item.setKeyEquivalent(&NSString::from_str("z"));
        redo_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        );
        edit_menu.addItem(&redo_item);

        // Separator
        let edit_sep1 = NSMenuItem::separatorItem(mtm);
        edit_menu.addItem(&edit_sep1);

        // Cut
        let cut_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        cut_item.setTitle(&NSString::from_str("Cut"));
        cut_item.setAction(Some(sel!(cut:)));
        cut_item.setTarget(None);
        cut_item.setKeyEquivalent(&NSString::from_str("x"));
        cut_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&cut_item);

        // Copy
        let copy_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        copy_item.setTitle(&NSString::from_str("Copy"));
        copy_item.setAction(Some(sel!(copy:)));
        copy_item.setTarget(None);
        copy_item.setKeyEquivalent(&NSString::from_str("c"));
        copy_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&copy_item);

        // Paste
        let paste_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        paste_item.setTitle(&NSString::from_str("Paste"));
        paste_item.setAction(Some(sel!(paste:)));
        paste_item.setTarget(None);
        paste_item.setKeyEquivalent(&NSString::from_str("v"));
        paste_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&paste_item);

        // Separator
        let edit_sep2 = NSMenuItem::separatorItem(mtm);
        edit_menu.addItem(&edit_sep2);

        // Select All
        let select_all_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        select_all_item.setTitle(&NSString::from_str("Select All"));
        select_all_item.setAction(Some(sel!(selectAll:)));
        select_all_item.setTarget(None);
        select_all_item.setKeyEquivalent(&NSString::from_str("a"));
        select_all_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        edit_menu.addItem(&select_all_item);

        // ── Window Menu ──
        let window_menu_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        let window_menu = NSMenu::init(mtm.alloc::<NSMenu>());
        window_menu.setTitle(&NSString::from_str("Window"));
        window_menu_item.setSubmenu(Some(&window_menu));
        main_menu.addItem(&window_menu_item);

        // Minimize
        let minimize_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        minimize_item.setTitle(&NSString::from_str("Minimize"));
        minimize_item.setAction(Some(sel!(performMiniaturize:)));
        minimize_item.setTarget(None);
        minimize_item.setKeyEquivalent(&NSString::from_str("m"));
        minimize_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        window_menu.addItem(&minimize_item);

        // Zoom
        let zoom_item = NSMenuItem::init(mtm.alloc::<NSMenuItem>());
        zoom_item.setTitle(&NSString::from_str("Zoom"));
        zoom_item.setAction(Some(sel!(performZoom:)));
        zoom_item.setTarget(None);
        window_menu.addItem(&zoom_item);

        // Set as the NSApplication's main menu
        let nsapp = NSApp(mtm);
        nsapp.setMainMenu(Some(&main_menu));
    }
}
