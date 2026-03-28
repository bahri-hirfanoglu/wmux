use std::path::PathBuf;

use anyhow::{Context, Result};

/// Returns the wmux data directory: %LOCALAPPDATA%\wmux
/// Creates the directory if it does not exist.
pub fn wmux_data_dir() -> Result<PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .context("LOCALAPPDATA environment variable not set")?;
    let dir = PathBuf::from(local_app_data).join("wmux");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create wmux data directory: {}", dir.display()))?;
    }
    Ok(dir)
}

/// Returns the path to the PID file: %LOCALAPPDATA%\wmux\wmux.pid
pub fn pid_file() -> Result<PathBuf> {
    Ok(wmux_data_dir()?.join("wmux.pid"))
}

/// Returns the path to the log file: %LOCALAPPDATA%\wmux\wmux.log
pub fn log_file() -> Result<PathBuf> {
    Ok(wmux_data_dir()?.join("wmux.log"))
}

/// Returns the path to the state file: %LOCALAPPDATA%\wmux\state.json
pub fn state_file() -> Result<PathBuf> {
    Ok(wmux_data_dir()?.join("state.json"))
}

/// Returns the named pipe path for the control pipe: \\.\pipe\wmux-ctl
pub fn control_pipe() -> String {
    r"\\.\pipe\wmux-ctl".to_string()
}
