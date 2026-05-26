use crate::Manager;
use crate::wine_processes::apply_runtime_env;
use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::{PrefixError, Result};
use log::{error, info};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

impl Manager {
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
        let mut cmd = self.build_wine_command_with_args(
            &[&executable.executable_path.to_string_lossy()],
            &config,
            prefix_path,
        );

        info!(
            "[launch] Launching '{}' in prefix '{}'",
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
                error!("[launch] Failed to launch '{}': {}", executable.name, e);
                Err(PrefixError::Process(format!(
                    "Failed to launch executable: {}",
                    e
                )))
            }
        }
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        info!("[launch] Opening winecfg for prefix '{}'", name);
        self.build_wine_command_for_exe("winecfg", &config, prefix_path)
            .current_dir(prefix_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;
        Ok(())
    }

    pub fn run_regedit(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        info!("[launch] Opening regedit for prefix '{}'", name);
        self.build_wine_command_with_args(&["regedit"], &config, prefix_path)
            .current_dir(prefix_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run regedit: {}", e)))?;
        Ok(())
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
