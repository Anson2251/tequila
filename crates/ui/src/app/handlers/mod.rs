use crate::AppMsg;
use log::error;
use service::AppService;
use std::path::PathBuf;

/// Open prefix in file manager
pub fn handle_open_in_file_manager(prefixes: &[base::WinePrefix], index: usize) {
    if let Some(prefix) = prefixes.get(index) {
        service::terminal::open_in_file_manager(&prefix.path);
    }
}

/// Open terminal for prefix
pub fn handle_open_in_terminal(service: &AppService, prefixes: &[base::WinePrefix], index: usize) {
    if let Some(prefix) = prefixes.get(index) {
        let svc = service.clone();
        let pp = prefix.path.clone();
        std::thread::spawn(move || {
            if let Err(e) = service::terminal::open_terminal_for_prefix(&svc, &pp) {
                error!("[term] failed to open terminal: {}", e);
            }
        });
    }
}

/// Scan for applications and update config in-place
pub fn handle_scan_for_applications(
    service: &AppService,
    prefixes: &mut [base::WinePrefix],
    index: usize,
) {
    if index < prefixes.len() {
        let prefix_path = prefixes[index].path.clone();
        let result =
            service::sync::scan_prefix_apps(service, &prefix_path, prefixes[index].config.clone());
        prefixes[index].config = result.config;

        if let Some(err) = result.error {
            error!("[app] scan failed: {}", err);
        }
    }
}

/// Sync all prefixes (background thread)
pub fn handle_sync_prefixes(
    service: AppService,
    sender: relm4::ComponentSender<crate::app::AppModel>,
    progress_sender: relm4::ComponentSender<crate::app::AppModel>,
) {
    std::thread::spawn(move || {
        let result = service::sync::sync_all_prefixes(&service);
        let total = result.prefixes.len();
        for i in 0..total {
            let _ = progress_sender.input(AppMsg::SyncProgress(i + 1, total));
        }
        let _ = sender.input(AppMsg::SyncComplete(result.prefixes));
    });
}

/// Refresh prefix list (background thread)
pub fn handle_refresh_prefixes(
    service: AppService,
    sender: relm4::ComponentSender<crate::app::AppModel>,
) {
    std::thread::spawn(move || {
        let fresh = service.scan_prefixes();
        let _ = sender.input(AppMsg::ReloadPrefixes(fresh));
    });
}

/// Handle config update with optional graphics backend switching
pub fn handle_config_updated(
    service: &AppService,
    index: usize,
    config: base::PrefixConfig,
    prefixes: &mut Vec<base::WinePrefix>,
    selected_prefix: Option<usize>,
) -> Option<ConfigUpdateAction> {
    let actual_index = if index == 0 { selected_prefix? } else { index };

    if actual_index >= prefixes.len() {
        return None;
    }

    let prefix_path = prefixes[actual_index].path.clone();
    let old_graphics = prefixes[actual_index].config.graphics.clone();
    let new_graphics = config.graphics.clone();
    let graphics_changed = match (&old_graphics, &new_graphics) {
        (None, None) => false,
        (Some(a), Some(b)) => a.backend != b.backend || a.version != b.version,
        _ => true,
    };

    if graphics_changed {
        let rollback_config = prefixes[actual_index].config.clone();
        prefixes[actual_index].config = config;

        Some(ConfigUpdateAction::SwitchGraphics {
            service: service.clone(),
            prefix_path,
            old_graphics,
            new_graphics,
            rollback_config,
        })
    } else {
        // Normal config save
        if let Err(e) = service.update_config(&prefix_path, &config) {
            error!("[app] failed to update config: {}", e);
        } else {
            prefixes[actual_index].config = config;
        }
        None
    }
}

pub enum ConfigUpdateAction {
    SwitchGraphics {
        service: AppService,
        prefix_path: PathBuf,
        old_graphics: Option<base::GraphicsConfig>,
        new_graphics: Option<base::GraphicsConfig>,
        rollback_config: base::PrefixConfig,
    },
}
