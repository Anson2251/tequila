use crate::prefix::error::{Result, PrefixError};
use std::path::PathBuf;
use std::process::Command;
use std::ffi::OsStr;

/// Trait for Wine process operations
pub trait WineProcesses {
    /// Get the Wine version for this prefix
    fn get_wine_version(&self) -> Result<String>;
    
    /// Start winecfg for this prefix
    fn start_winecfg(&self) -> Result<()>;
    
    /// Start regedit for this prefix
    fn start_regedit(&self) -> Result<()>;
    
    /// Start control panel for this prefix
    fn start_control_panel(&self) -> Result<()>;
    
    /// Run an executable within this prefix
    fn run_executable(&self, executable_path: &PathBuf) -> Result<()>;
    
    /// Run a Windows command within this prefix
    fn run_windows_command(&self, command: &str) -> Result<()>;
}

/// Implementation of WineProcesses for WinePrefix
impl WineProcesses for super::traits::WinePrefix {
    fn get_wine_version(&self) -> Result<String> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        let output = Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8(output.stdout)
                .map_err(|e| PrefixError::Process(format!("Failed to parse wine version: {}", e)))?;
            Ok(version.trim().to_string())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(PrefixError::Process(format!("Failed to get wine version: {}", error)))
        }
    }

    fn start_winecfg(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("winecfg")
            .env("WINEPREFIX", &wine_prefix)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start winecfg: {}", e)))?;

        // Don't wait for completion - winecfg is a GUI application
        Ok(())
    }

    fn start_regedit(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("regedit")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start regedit: {}", e)))?;

        // Don't wait for completion - regedit is a GUI application
        Ok(())
    }

    fn start_control_panel(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("control")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start control panel: {}", e)))?;

        // Don't wait for completion - control panel is a GUI application
        Ok(())
    }

    fn run_executable(&self, executable_path: &PathBuf) -> Result<()> {
        if !executable_path.exists() {
            return Err(PrefixError::NotFound(format!("Executable not found: {}", executable_path.display())));
        }

        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg(executable_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run executable: {}", e)))?;

        // Don't wait for completion - executable might be a GUI application
        Ok(())
    }

    fn run_windows_command(&self, command: &str) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("cmd")
            .arg("/c")
            .arg(command)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run Windows command: {}", e)))?;

        // Don't wait for completion - command might be interactive
        Ok(())
    }
}

/// Implementation of WineProcesses for Manager
impl WineProcesses for super::manager::Manager {
    fn get_wine_version(&self) -> Result<String> {
        let output = Command::new("wine")
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8(output.stdout)
                .map_err(|e| PrefixError::Process(format!("Failed to parse wine version: {}", e)))?;
            Ok(version.trim().to_string())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(PrefixError::Process(format!("Failed to get wine version: {}", error)))
        }
    }

    fn start_winecfg(&self) -> Result<()> {
        let output = Command::new("winecfg")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start winecfg: {}", e)))?;

        // Don't wait for completion - winecfg is a GUI application
        Ok(())
    }

    fn start_regedit(&self) -> Result<()> {
        let output = Command::new("wine")
            .arg("regedit")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start regedit: {}", e)))?;

        // Don't wait for completion - regedit is a GUI application
        Ok(())
    }

    fn start_control_panel(&self) -> Result<()> {
        let output = Command::new("wine")
            .arg("control")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start control panel: {}", e)))?;

        // Don't wait for completion - control panel is a GUI application
        Ok(())
    }

    fn run_executable(&self, executable_path: &PathBuf) -> Result<()> {
        if !executable_path.exists() {
            return Err(PrefixError::NotFound(format!("Executable not found: {}", executable_path.display())));
        }

        let output = Command::new("wine")
            .arg(executable_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run executable: {}", e)))?;

        // Don't wait for completion - executable might be a GUI application
        Ok(())
    }

    fn run_windows_command(&self, command: &str) -> Result<()> {
        let output = Command::new("wine")
            .arg("cmd")
            .arg("/c")
            .arg(command)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run Windows command: {}", e)))?;

        // Don't wait for completion - command might be interactive
        Ok(())
    }
}