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
/// - "horizontal" creates a horizontal split (new pane below)
/// - "vertical" creates a vertical split (new pane to the right)
///
/// `command_line` is the full command to run in the new pane
/// (e.g., `"C:\path\to\wmux.exe attach 1 --pane 2"`).
pub fn wt_split_pane(direction: &str, command_line: &str) -> Result<()> {
    let output = std::process::Command::new("wt.exe")
        .args([
            "-w",
            "0",
            "split-pane",
            &format!("--{}", direction),
            "cmd",
            "/c",
            command_line,
        ])
        .output()
        .context("Failed to execute wt.exe — is Windows Terminal installed and on PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "wt.exe split-pane failed (exit code {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
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
