//! Windows Terminal detection and command helpers.
//!
//! wmux requires Windows Terminal (wt.exe) for pane management.
//! This module provides detection and wt.exe CLI command wrappers.

use anyhow::{bail, Context, Result};

/// Check if the current process is running inside Windows Terminal.
///
/// Windows Terminal sets the `WT_SESSION` environment variable for all
/// processes running inside it.
pub fn is_windows_terminal() -> bool {
    std::env::var("WT_SESSION").is_ok()
}

/// Require that the process is running inside Windows Terminal.
///
/// Returns `Ok(())` if inside WT, otherwise returns an error with
/// a clear message directing the user to use Windows Terminal.
pub fn require_windows_terminal() -> Result<()> {
    if !is_windows_terminal() {
        bail!(
            "wmux requires Windows Terminal. Please run wmux inside Windows Terminal (wt.exe)."
        );
    }
    Ok(())
}

/// Split the current pane in Windows Terminal using `wt.exe`.
///
/// `direction` should be "horizontal" or "vertical".
/// `command` is the command to run in the new pane.
pub fn wt_split_pane(direction: &str, command: &str) -> Result<()> {
    let status = std::process::Command::new("wt.exe")
        .args(["split-pane", &format!("--{}", direction), command])
        .status()
        .context("Failed to execute wt.exe. Is Windows Terminal installed?")?;

    if !status.success() {
        bail!(
            "wt.exe split-pane failed with exit code: {}",
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

/// Focus a specific pane in Windows Terminal by index.
///
/// `pane_index` is the 0-based pane index in the current tab.
pub fn wt_focus_pane(pane_index: u32) -> Result<()> {
    let status = std::process::Command::new("wt.exe")
        .args(["focus-pane", "--target", &pane_index.to_string()])
        .status()
        .context("Failed to execute wt.exe. Is Windows Terminal installed?")?;

    if !status.success() {
        bail!(
            "wt.exe focus-pane failed with exit code: {}",
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}
