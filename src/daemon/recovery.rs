//! State persistence and crash recovery for the wmux daemon.
//!
//! The daemon persists session metadata to `state.json` on every structural change
//! (session create/kill). On startup after a crash, sessions are recovered by checking
//! if child processes are still alive and respawning shells as needed.
//!
//! **ConPTY re-adoption limitation:** ConPTY pseudo-console handles are process-local
//! and cannot be inherited across daemon restarts. After a crash, the daemon cannot
//! re-attach to existing ConPTY sessions. The pragmatic approach is to always respawn
//! shells and log what happened. Scrollback is lost on crash (per CONTEXT.md decision).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Persisted state schema for crash recovery.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub sessions: Vec<PersistedSession>,
    pub next_id: u32,
    pub saved_at: String,
}

impl Default for PersistedState {
    fn default() -> Self {
        PersistedState {
            version: 1,
            sessions: Vec::new(),
            next_id: 1,
            saved_at: String::new(),
        }
    }
}

/// Metadata for a single persisted pane within a session.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersistedPane {
    pub id: u32,
    pub pid: u32,
    pub shell: String,
    pub cols: i16,
    pub rows: i16,
}

/// Metadata for a single persisted session.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: String,
    pub name: Option<String>,
    /// Legacy single-pane pid (kept for backward compat with v1 state files).
    #[serde(default)]
    pub pid: u32,
    pub created_at: String,
    /// Legacy single-pane fields (kept for backward compat).
    #[serde(default = "default_shell")]
    pub shell: String,
    #[serde(default = "default_cols")]
    pub cols: i16,
    #[serde(default = "default_rows")]
    pub rows: i16,
    /// All panes in this session. If empty, falls back to legacy single-pane fields.
    #[serde(default)]
    pub panes: Vec<PersistedPane>,
}

fn default_shell() -> String {
    "powershell.exe".to_string()
}

fn default_cols() -> i16 {
    120
}

fn default_rows() -> i16 {
    30
}

/// Report of recovery results after daemon restart.
#[derive(Debug)]
pub struct RecoveryReport {
    pub recovered: u32,
    pub respawned: u32,
    pub failed: u32,
}

/// Save state to the default wmux data directory.
pub fn save_state(state: &PersistedState) -> Result<()> {
    let dir = crate::paths::wmux_data_dir()?;
    save_state_to(state, &dir)
}

/// Save state to a specific directory (for testing).
pub fn save_state_to(state: &PersistedState, dir: &PathBuf) -> Result<()> {
    let state_path = dir.join("state.json");
    let tmp_path = dir.join("state.json.tmp");

    let json = serde_json::to_string_pretty(state)
        .context("Failed to serialize state to JSON")?;

    // Write to temp file first, then atomically rename
    fs::write(&tmp_path, &json)
        .with_context(|| format!("Failed to write temp state file: {}", tmp_path.display()))?;

    fs::rename(&tmp_path, &state_path)
        .with_context(|| format!("Failed to rename state file: {} -> {}", tmp_path.display(), state_path.display()))?;

    debug!("State saved: {} sessions", state.sessions.len());
    Ok(())
}

/// Load state from the default wmux data directory.
pub fn load_state() -> Result<PersistedState> {
    let dir = crate::paths::wmux_data_dir()?;
    load_state_from(&dir)
}

/// Load state from a specific directory (for testing).
pub fn load_state_from(dir: &PathBuf) -> Result<PersistedState> {
    let state_path = dir.join("state.json");

    if !state_path.exists() {
        debug!("No state file found, starting fresh");
        return Ok(PersistedState::default());
    }

    let contents = fs::read_to_string(&state_path)
        .with_context(|| format!("Failed to read state file: {}", state_path.display()))?;

    match serde_json::from_str::<PersistedState>(&contents) {
        Ok(state) => {
            debug!("State loaded: {} sessions", state.sessions.len());
            Ok(state)
        }
        Err(e) => {
            warn!("Corrupted state file, starting fresh: {}", e);
            // Back up the corrupted file
            let backup_path = dir.join("state.json.bak");
            let _ = fs::rename(&state_path, &backup_path);
            Ok(PersistedState::default())
        }
    }
}

/// Recover sessions from persisted state after a daemon restart.
///
/// For each persisted session:
/// - If the original process is still alive, we note it as orphaned (cannot re-attach
///   to its ConPTY) and spawn a replacement shell.
/// - If the process is dead, spawn a new shell with the same terminal dimensions.
/// - In both cases, the session ID and name are preserved.
pub fn recover_sessions(
    persisted: &PersistedState,
    manager: &mut crate::session::SessionManager,
) -> Result<RecoveryReport> {
    use tracing::info;

    let mut recovered: u32 = 0;
    let mut respawned: u32 = 0;
    let mut failed: u32 = 0;

    for ps in &persisted.sessions {
        // Determine the panes to recover: use new multi-pane list if available,
        // otherwise fall back to legacy single-pane fields.
        let panes_to_recover: Vec<PersistedPane> = if !ps.panes.is_empty() {
            ps.panes.clone()
        } else {
            vec![PersistedPane {
                id: 0,
                pid: ps.pid,
                shell: ps.shell.clone(),
                cols: ps.cols,
                rows: ps.rows,
            }]
        };

        // Recover the first pane to create the session via restore_session
        let first = &panes_to_recover[0];
        let alive = is_process_alive(first.pid);
        info!(
            "Session {}: recovering {} pane(s), first pane pid {} ({})",
            ps.id,
            panes_to_recover.len(),
            first.pid,
            if alive { "alive" } else { "dead" }
        );

        match crate::session::conpty::ConPtySession::new(first.cols, first.rows, Some(&first.shell)) {
            Ok(conpty) => {
                manager.restore_session(ps.id.clone(), ps.name.clone(), conpty);
                if alive { recovered += 1; } else { respawned += 1; }
            }
            Err(e) => {
                warn!("Failed to recover session {}: {}", ps.id, e);
                failed += 1;
                continue;
            }
        }

        // Recover additional panes (index 1+)
        for pp in &panes_to_recover[1..] {
            let pane_alive = is_process_alive(pp.pid);
            info!(
                "Session {} pane {}: pid {} ({}), spawning replacement",
                ps.id, pp.id, pp.pid,
                if pane_alive { "alive" } else { "dead" }
            );

            match manager.add_pane(&ps.id, pp.cols, pp.rows, Some(&pp.shell)) {
                Ok((new_pane_id, new_pid)) => {
                    info!(
                        "Session {} pane {} recovered as pane {} (pid {})",
                        ps.id, pp.id, new_pane_id, new_pid
                    );
                    if pane_alive { recovered += 1; } else { respawned += 1; }
                }
                Err(e) => {
                    warn!(
                        "Failed to recover pane {} in session {}: {}",
                        pp.id, ps.id, e
                    );
                    failed += 1;
                }
            }
        }
    }

    // Restore next_id counter
    manager.set_next_id(persisted.next_id);

    Ok(RecoveryReport {
        recovered,
        respawned,
        failed,
    })
}

/// Check if a process with the given PID is still alive.
fn is_process_alive(pid: u32) -> bool {
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

        let result = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        match result {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(_) => false,
        }
    }
}
