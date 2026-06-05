use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};

const DEFAULT_WINE_DIR: &str = "Wine";

/// Tequila — Wine Prefix Manager
#[derive(Parser)]
#[command(name = "tequila", version, about)]
struct Cli {
    /// Force GUI mode (default when no subcommand is given)
    #[arg(long, global = true)]
    gui: bool,

    #[command(subcommand)]
    command: Option<Subcmd>,
}

#[derive(Subcommand)]
enum Subcmd {
    /// Launch an executable in a Wine prefix (headless, no GUI)
    Run {
        /// Prefix name (looked up under ~/Wine/<name>) or direct path
        prefix: String,

        /// Registered executable name, or path to a .exe file
        exe: String,

        /// Optional arguments forwarded to the executable
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .target(env_logger::Target::Stdout)
        .init();

    // Check for --gui in raw args before clap parsing, so it wins even when
    // placed after subcommand arguments that would otherwise be consumed by
    // `trailing_var_arg` (e.g. `tequila run myprefix myexe --gui`).
    let has_gui_flag = std::env::args().any(|a| a == "--gui");
    let cli = Cli::parse();

    // Dispatch: --gui or no subcommand → GTK UI; otherwise → CLI mode
    if has_gui_flag || cli.command.is_none() {
        start_gui()
    } else {
        match cli.command.unwrap() {
            Subcmd::Run { prefix, exe, args } => match run(&prefix, &exe, &args) {
                Ok(code) => ExitCode::from(code),
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            },
        }
    }
}

// ── GUI entry point (original behaviour) ───────────────────────────────

fn start_gui() -> ExitCode {
    log::info!("[tequila] starting GUI");
    let app = relm4::RelmApp::new("com.github.anson2251.tequila");
    ui::initialize_custom_resources();
    app.run::<ui::AppModel>(());
    ExitCode::SUCCESS
}

// ── CLI mode ───────────────────────────────────────────────────────────

fn run(prefix_arg: &str, exe_arg: &str, extra_args: &[String]) -> Result<u8, String> {
    let prefix_path = resolve_prefix(prefix_arg)?;

    let config = base::config::PrefixConfig::load_from_file(&prefix_path)
        .map_err(|e| format!("failed to load prefix config: {e}"))?
        .ok_or_else(|| {
            format!(
                "no tequila-config.json found in '{}' — is this a Tequila-managed prefix?",
                prefix_path.display()
            )
        })?;

    let exe_path = resolve_exe_path(&config, exe_arg, &prefix_path)?;

    let runtime_manager: runtime::RuntimeManager = if let Some(settings) = store::Settings::load() {
        let mut rm: runtime::RuntimeManager = settings.into();
        rm.ensure_system_runtime();
        rm
    } else {
        let mut rm = runtime::RuntimeManager::new();
        rm.ensure_system_runtime();
        rm
    };

    let runtime = runtime_manager
        .resolve(config.wine_version.as_deref())
        .ok_or_else(|| "no Wine runtime configured or available".to_string())?;

    check_wine_available(runtime)?;

    let (env_vars, cwd) = config
        .get_executable_by_name(exe_arg)
        .map(|exe| (exe.env_vars.clone(), exe.cwd.clone()))
        .unwrap_or_default();

    let mut cmd = Command::new("wine");
    prefix::apply_runtime_env(&mut cmd, runtime, &prefix_path);

    for (key, value) in &env_vars {
        cmd.env(key, value);
    }

    cmd.arg(&exe_path);
    for arg in extra_args {
        cmd.arg(arg);
    }

    if let Some(cwd) = &cwd {
        cmd.current_dir(cwd);
    } else {
        cmd.current_dir(&prefix_path);
    }

    log::info!(
        "launching '{}' in prefix '{}'",
        exe_path,
        prefix_path.display()
    );

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn wine: {e}"))?;

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait for wine: {e}"))?;

    let code = status.code().unwrap_or(1) as u8;
    log::info!("process exited with code {}", code);
    Ok(code)
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn resolve_prefix(arg: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(arg);
    if candidate.is_dir() {
        if is_valid_prefix(&candidate) {
            return Ok(candidate);
        }
        return Err(format!(
            "'{}' exists but is not a valid Wine prefix (missing drive_c/, system.reg, or user.reg)",
            candidate.display()
        ));
    }

    let wine_dir = default_wine_dir();
    let by_name = wine_dir.join(arg);
    if by_name.is_dir() && is_valid_prefix(&by_name) {
        return Ok(by_name);
    }

    Err(format!(
        "prefix '{}' not found — not a valid path and no matching directory under {}",
        arg,
        wine_dir.display()
    ))
}

fn default_wine_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(DEFAULT_WINE_DIR)
}

fn is_valid_prefix(path: &Path) -> bool {
    path.join("drive_c").exists()
        && path.join("system.reg").exists()
        && path.join("user.reg").exists()
}

fn resolve_exe_path(
    config: &base::config::PrefixConfig,
    arg: &str,
    prefix_path: &Path,
) -> Result<String, String> {
    if let Some(exe) = config.get_executable_by_name(arg) {
        return exe
            .executable_path
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "invalid executable path in config".to_string());
    }

    let path = PathBuf::from(arg);
    if path.is_absolute() {
        if path.exists() {
            return Ok(path.to_string_lossy().to_string());
        }
        return Err(format!("file not found: {}", path.display()));
    }

    let joined = prefix_path.join(&path);
    if joined.exists() {
        return Ok(joined.to_string_lossy().to_string());
    }

    Err(format!(
        "executable '{arg}' not found — not a registered executable and not a valid path"
    ))
}

fn check_wine_available(runtime: &runtime::Runtime) -> Result<(), String> {
    if runtime.source == runtime::RuntimeSource::System {
        if find_in_path("wine").is_some() {
            return Ok(());
        }
        if Path::new("/usr/bin/wine").exists() || Path::new("/usr/local/bin/wine").exists() {
            return Ok(());
        }
        return Err("system Wine runtime is configured but 'wine' was not found in PATH".into());
    }

    let wine_bin = runtime.bundle_dir.join("bin").join("wine");
    if wine_bin.exists() {
        return Ok(());
    }

    Err(format!(
        "Wine runtime '{}' is configured but not found at '{}'",
        runtime.name,
        runtime.bundle_dir.display()
    ))
}

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
