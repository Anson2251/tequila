use crate::Manager;
use crate::wine_processes::apply_runtime_env;
use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::{PrefixError, Result};
use log::{error, info};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

impl Manager {
    /// Check that the given binary (`"wine"`, `"winecfg"`, …) is available
    /// for the current runtime configuration.  Returns a clear error message
    /// when it isn't.
    pub(crate) fn check_wine_available(
        &self,
        binary_name: &str,
        config: &PrefixConfig,
    ) -> Result<()> {
        if let Some(runtime) = self.runtime_for_prefix(config) {
            // For system-installed Wine, check PATH and standard locations
            // instead of bundle_dir (which is empty for system runtimes).
            if runtime.source == runtime::RuntimeSource::System {
                if find_in_path(binary_name).is_some() {
                    return Ok(());
                }
                // Some distros don't add /usr/bin to PATH by default
                if binary_name == "wine"
                    && (Path::new("/usr/bin/wine").exists()
                        || Path::new("/usr/local/bin/wine").exists())
                {
                    return Ok(());
                }
                return Err(PrefixError::NotFound(format!(
                    "Wine runtime 'System Wine' is configured but '{}' was not \
                     found in PATH.\n\
                     Install Wine through your package manager, or add a managed \
                     runtime in Settings → Wine Runtime.",
                    binary_name,
                )));
            }

            let bundle_bin = runtime.bundle_dir.join("bin").join(binary_name);
            if bundle_bin.exists() {
                return Ok(());
            }
            // Runtime is configured but the bundle is missing
            let dir = runtime.bundle_dir.display();
            return Err(PrefixError::NotFound(format!(
                "Wine runtime '{}' is configured but not found at {}.\n\
                 The runtime directory may have been deleted or moved.\n\
                 Please go to Settings → Wine Runtime and reinstall \
                 or select a different runtime.",
                runtime.name, dir,
            )));
        }

        // No runtime configured — look for the binary in PATH
        if find_in_path(binary_name).is_some() {
            return Ok(());
        }

        // For "wine" specifically, also check if there's a system wine at the
        // standard location (some distros don't add /usr/bin/wine to PATH by default).
        if binary_name == "wine"
            && (Path::new("/usr/bin/wine").exists() || Path::new("/usr/local/bin/wine").exists())
        {
            return Ok(());
        }

        Err(PrefixError::NotFound(format!(
            "'{}' was not found on your system and no Wine runtime is configured.\n\
             Install Wine through your package manager, or add a managed runtime \
             in Settings → Wine Runtime.",
            binary_name,
        )))
    }

    pub fn launch_executable(
        &self,
        prefix_path: &PathBuf,
        executable: &RegisteredExecutable,
    ) -> Result<Child> {
        if !executable.executable_path.exists() {
            error!(
                "[launch] Executable not found: {}",
                executable.executable_path.display()
            );
            return Err(PrefixError::NotFound(
                "Executable file does not exist".to_string(),
            ));
        }
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;

        // Check wine is available before building the command
        self.check_wine_available("wine", &config)?;

        let mut cmd = self.build_wine_command_with_args(
            &[&executable.executable_path.to_string_lossy()],
            &config,
            prefix_path,
        );

        info!(
            "[launch] launching '{}' in prefix '{}'",
            executable.name, name
        );

        // Log the full command line
        let cmd_line: Vec<String> =
            std::iter::once(cmd.get_program().to_string_lossy().to_string())
                .chain(cmd.get_args().map(|a| a.to_string_lossy().to_string()))
                .collect();
        info!("[launch]   {}", cmd_line.join(" "));

        // Apply per-executable environment variables
        for (key, value) in &executable.env_vars {
            cmd.env(key, value);
            info!("[launch]   {}={}", key, value);
        }

        // Apply per-executable working directory (fall back to prefix_path)
        if let Some(cwd) = &executable.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(prefix_path);
        }

        match cmd.spawn() {
            Ok(child) => {
                info!(
                    "[launch] '{}' started (PID: {})",
                    executable.name,
                    child.id()
                );
                Ok(child)
            }
            Err(e) => {
                error!("[launch] failed to launch '{}': {}", executable.name, e);
                Err(PrefixError::Process(format!(
                    "Failed to launch executable: {}",
                    e
                )))
            }
        }
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<Child> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;

        // Check winecfg is available before spawning
        self.check_wine_available("winecfg", &config)?;

        info!("[launch] opening winecfg for prefix '{}'", name);
        let child = self
            .build_wine_command_for_exe("winecfg", &config, prefix_path)
            .current_dir(prefix_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;
        Ok(child)
    }

    pub fn run_regedit(&self, prefix_path: &PathBuf) -> Result<Child> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;

        // Check wine is available before spawning
        self.check_wine_available("wine", &config)?;

        info!("[launch] opening regedit for prefix '{}'", name);
        let child = self
            .build_wine_command_with_args(&["regedit"], &config, prefix_path)
            .current_dir(prefix_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run regedit: {}", e)))?;
        Ok(child)
    }

    /// Core helper: build a `Command` with runtime env applied (WINEPREFIX, PATH, WINEDLLPATH, etc.).
    fn build_wine_command(&self, config: &PrefixConfig, prefix_path: &Path) -> Command {
        let mut cmd = Command::new("wine");
        if let Some(runtime) = self.runtime_for_prefix(config) {
            apply_runtime_env(&mut cmd, runtime, prefix_path);
        } else {
            cmd.env("WINEPREFIX", prefix_path);
        }
        cmd
    }

    /// Build a wine command for a named binary (e.g. "winecfg", "regedit").
    pub fn build_wine_command_for_exe(
        &self,
        exe: &str,
        config: &PrefixConfig,
        prefix_path: &Path,
    ) -> Command {
        let mut cmd = Command::new(exe);
        if let Some(runtime) = self.runtime_for_prefix(config) {
            apply_runtime_env(&mut cmd, runtime, prefix_path);
        } else {
            cmd.env("WINEPREFIX", prefix_path);
        }
        cmd
    }

    /// Build a wine command with additional arguments.
    pub fn build_wine_command_with_args(
        &self,
        args: &[&str],
        config: &PrefixConfig,
        prefix_path: &Path,
    ) -> Command {
        let mut cmd = self.build_wine_command(config, prefix_path);
        for arg in args {
            cmd.arg(arg);
        }
        cmd
    }
}

/// Search PATH for a named executable using `which`.
fn find_in_path(name: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()?;
    if output.status.success() {
        Some(PathBuf::from(String::from_utf8(output.stdout).ok()?.trim()))
    } else {
        None
    }
}
