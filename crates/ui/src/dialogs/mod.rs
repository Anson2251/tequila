use std::path::PathBuf;
// Used for the non-macos folder dialog
#[cfg(not(target_os = "macos"))]
use gtk4::prelude::*;

/// Opens a folder picker dialog with an optional initial directory.
///
/// On macOS this uses `NSOpenPanel` (native), on other platforms it falls back
/// to `gtk4::FileDialog`.
pub fn pick_folder<F>(parent: &gtk4::Window, initial_path: Option<&str>, callback: F)
where
    F: Fn(String) + 'static,
{
    log::debug!("[dialogs] {:?}", &initial_path);
    #[cfg(target_os = "macos")]
    macos_pick_folder(parent, initial_path, callback);

    #[cfg(not(target_os = "macos"))]
    gtk_pick_folder(parent, initial_path, callback);
}

/// Opens a file picker dialog for the given file extensions.
///
/// On macOS this uses `NSOpenPanel` (native), on other platforms it falls back
/// to `gtk4::FileDialog`.  `extensions` is a list of allowed suffixes
/// (e.g. `["dmg"]`, `["png", "jpg"]`).  Pass an empty slice to allow all files.
pub fn pick_file<F>(parent: &gtk4::Window, title: &str, extensions: &[&str], callback: F)
where
    F: Fn(Option<String>) + 'static,
{
    #[cfg(target_os = "macos")]
    macos_pick_file(parent, title, extensions, callback);

    #[cfg(not(target_os = "macos"))]
    gtk_pick_file(parent, title, extensions, callback);
}

/// Opens a **save** file dialog for the given file extensions.
///
/// `suggested_name` is the default filename shown in the dialog.
/// On macOS this uses `NSSavePanel` (native), on other platforms it falls back
/// to `gtk4::FileDialog::save()`.
pub fn save_file<F>(
    parent: &gtk4::Window,
    title: &str,
    suggested_name: &str,
    extensions: &[&str],
    callback: F,
) where
    F: Fn(Option<String>) + 'static,
{
    #[cfg(target_os = "macos")]
    macos_save_file(parent, title, suggested_name, extensions, callback);

    #[cfg(not(target_os = "macos"))]
    gtk_save_file(parent, title, suggested_name, extensions, callback);
}

// ── macOS native implementation ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_pick_folder<F>(_parent: &gtk4::Window, initial_path: Option<&str>, callback: F)
where
    F: Fn(String) + 'static,
{
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSURL;
    use std::cell::RefCell;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&objc2_foundation::NSString::from_str(
        "Choose Working Directory",
    )));

    // Set initial directory if provided
    if let Some(path) = initial_path {
        if let Ok(canonical) = PathBuf::from(path).canonicalize() {
            if canonical.is_dir() {
                let url = NSURL::fileURLWithPath(&objc2_foundation::NSString::from_str(
                    &canonical.display().to_string(),
                ));
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

// ── macOS save panel ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_save_file<F>(
    _parent: &gtk4::Window,
    title: &str,
    suggested_name: &str,
    _extensions: &[&str],
    callback: F,
) where
    F: Fn(Option<String>) + 'static,
{
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSSavePanel};
    use std::cell::RefCell;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSSavePanel::savePanel(mtm);
    panel.setTitle(Some(&objc2_foundation::NSString::from_str(title)));
    panel.setNameFieldStringValue(&objc2_foundation::NSString::from_str(suggested_name));
    panel.setCanCreateDirectories(true);

    // Set allowed file types if extensions are provided
    if !_extensions.is_empty() {
        use objc2_foundation::NSArray;
        let allowed: Vec<&objc2_foundation::NSString> = _extensions
            .iter()
            .map(|e| objc2_foundation::NSString::from_str(e))
            .collect();
        let arr = NSArray::from_vec(&allowed);
        panel.setAllowedFileTypes(Some(&arr));
    }

    let cb = RefCell::new(Some(callback));
    let panel_for_block = panel.clone();
    let block = RcBlock::new(move |result: NSModalResponse| {
        if result == NSModalResponseOK {
            if let Some(url) = panel_for_block.URL() {
                if let Some(path_str) = url.path() {
                    let path: String = path_str.to_string();
                    if let Some(cb) = cb.take() {
                        cb(Some(path));
                        return;
                    }
                }
            }
        }
        if let Some(cb) = cb.take() {
            cb(None);
        }
    });

    panel.beginWithCompletionHandler(&block);
}

// ── macOS file picker ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_pick_file<F>(_parent: &gtk4::Window, title: &str, _extensions: &[&str], callback: F)
where
    F: Fn(Option<String>) + 'static,
{
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel};
    use std::cell::RefCell;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&objc2_foundation::NSString::from_str(title)));

    let cb = RefCell::new(Some(callback));
    let panel_for_block = panel.clone();
    let block = RcBlock::new(move |result: NSModalResponse| {
        if result == NSModalResponseOK {
            let urls = panel_for_block.URLs();
            if let Some(url) = urls.firstObject() {
                if let Some(path_str) = url.path() {
                    let path: String = path_str.to_string();
                    if let Some(cb) = cb.take() {
                        cb(Some(path));
                        return;
                    }
                }
            }
        }
        // Cancelled or no path
        if let Some(cb) = cb.take() {
            cb(None);
        }
    });

    panel.beginWithCompletionHandler(&block);
}

// ── GTK fallback implementation ──────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
fn gtk_pick_folder<F>(parent: &gtk4::Window, initial_path: Option<&str>, callback: F)
where
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

#[cfg(not(target_os = "macos"))]
fn gtk_pick_file<F>(parent: &gtk4::Window, title: &str, extensions: &[&str], callback: F)
where
    F: Fn(Option<String>) + 'static,
{
    let dialog = gtk4::FileDialog::builder().title(title).build();

    let filter = if !extensions.is_empty() {
        let filter = gtk4::FileFilter::new();
        for ext in extensions {
            filter.add_suffix(ext);
        }
        Some(filter)
    } else {
        None
    };
    if let Some(f) = filter {
        dialog.set_default_filter(Some(&f));
    }

    dialog.open(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| match result {
            Ok(file) => callback(file.path().map(|p| p.display().to_string())),
            Err(_) => callback(None),
        },
    );
}

#[cfg(not(target_os = "macos"))]
fn gtk_save_file<F>(
    parent: &gtk4::Window,
    title: &str,
    suggested_name: &str,
    extensions: &[&str],
    callback: F,
) where
    F: Fn(Option<String>) + 'static,
{
    let dialog = gtk4::FileDialog::builder()
        .title(title)
        .initial_name(suggested_name)
        .build();

    let filter = if !extensions.is_empty() {
        let filter = gtk4::FileFilter::new();
        for ext in extensions {
            filter.add_suffix(ext);
        }
        Some(filter)
    } else {
        None
    };
    if let Some(f) = filter {
        dialog.set_default_filter(Some(&f));
    }

    dialog.save(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| match result {
            Ok(file) => callback(file.path().map(|p| p.display().to_string())),
            Err(_) => callback(None),
        },
    );
}
