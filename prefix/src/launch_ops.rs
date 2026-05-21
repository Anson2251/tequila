use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::{Result, PrefixError};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::Manager;
use crate::wine_processes::apply_runtime_env;

impl Manager {
    pub fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()> {
        if !executable.executable_path.exists() {
            return Err(PrefixError::NotFound("Executable file does not exist".to_string()));
        }
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_with_args(&[&executable.executable_path.to_string_lossy()], &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to launch executable: {}", e)))?;
        Ok(())
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_for_exe("winecfg", &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;
        Ok(())
    }

    pub fn run_regedit(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_with_args(&["regedit"], &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run regedit: {}", e)))?;
        Ok(())
    }

    #[allow(dead_code)]
    fn build_wine_command(&self, config: &PrefixConfig, prefix_path: &Path) -> Command {
        let mut cmd = Command::new("wine");
        if let Some(runtime) = self.runtime_for_prefix(config) {
            apply_runtime_env(&mut cmd, runtime, prefix_path);
        } else {
            cmd.env("WINEPREFIX", prefix_path);
        }
        cmd
    }

    pub(crate) fn build_wine_command_for_exe(&self, exe: &str, config: &PrefixConfig, prefix_path: &Path) -> Command {
        if let Some(runtime) = self.runtime_for_prefix(config) {
            let mut cmd = Command::new(exe);
            apply_runtime_env(&mut cmd, runtime, prefix_path);
            cmd
        } else {
            let mut cmd = Command::new(exe);
            cmd.env("WINEPREFIX", prefix_path);
            cmd
        }
    }

    pub(crate) fn build_wine_command_with_args(&self, args: &[&str], config: &PrefixConfig, prefix_path: &Path) -> Command {
        let mut cmd = if let Some(runtime) = self.runtime_for_prefix(config) {
            let mut cmd = Command::new("wine");
            apply_runtime_env(&mut cmd, runtime, prefix_path);
            cmd
        } else {
            let mut cmd = Command::new("wine");
            cmd.env("WINEPREFIX", prefix_path);
            cmd
        };
        for arg in args {
            cmd.arg(arg);
        }
        cmd
    }
}
