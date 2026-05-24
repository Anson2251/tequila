use std::path::PathBuf;
// Used for the non-macos folder dialog
#[cfg(not(target_os = "macos"))]
use gtk4::prelude::*;

/// Opens a folder picker dialog with an optional initial directory.
///
/// On macOS this uses `NSOpenPanel` (native), on other platforms it falls back
/// to `gtk4::FileDialog`.
pub fn pick_folder<F>(
    parent: &gtk4::Window,
    initial_path: Option<&str>,
    callback: F,
) where
    F: Fn(String) + 'static,
{
    println!("{:?}", &initial_path);
    #[cfg(target_os = "macos")]
    macos_pick_folder(parent, initial_path, callback);

    #[cfg(not(target_os = "macos"))]
    gtk_pick_folder(parent, initial_path, callback);
}

// ── macOS native implementation ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_pick_folder<F>(
    _parent: &gtk4::Window,
    initial_path: Option<&str>,
    callback: F,
) where
    F: Fn(String) + 'static,
{
    use std::cell::RefCell;
    use objc2::MainThreadMarker;
    use objc2_foundation::NSURL;
    use objc2_app_kit::{NSOpenPanel, NSModalResponse, NSModalResponseOK};
    use block2::RcBlock;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&objc2_foundation::NSString::from_str("Choose Working Directory")));

    // Set initial directory if provided
    if let Some(path) = initial_path {
        if let Ok(canonical) = PathBuf::from(path).canonicalize() {
            if canonical.is_dir() {
                let url = NSURL::fileURLWithPath(&objc2_foundation::NSString::from_str(&canonical.display().to_string()));
                panel.setDirectoryURL(Some(&url));
            }
        }
    }

    let callback = RefCell::new(Some(callback));
    let panel_for_block = panel.clone();
    let block = RcBlock::new(move |result: NSModalResponse| {
        if result == NSModalResponseOK {
            let urls = panel_for_block.URLs();
            if let Some(url) = urls.firstObject() {
                if let Some(path_str) = url.path() {
                    let path: String = path_str.to_string();
                    if let Some(cb) = callback.take() {
                        cb(path);
                    }
                }
            }
        }
    });

    panel.beginWithCompletionHandler(&block);
}

// ── GTK fallback implementation ──────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
fn gtk_pick_folder<F>(
    parent: &gtk4::Window,
    initial_path: Option<&str>,
    callback: F,
) where
    F: Fn(String) + 'static,
{
    let dialog = gtk4::FileDialog::builder()
        .title("Choose Working Directory")
        .build();

    if let Some(path) = initial_path {
        if let Ok(canonical) = PathBuf::from(path).canonicalize() {
            if canonical.is_dir() {
                dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&canonical)));
            }
        }
    }

    dialog.select_folder(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    callback(path.display().to_string());
                }
            }
        },
    );
}
