use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use tracing::info;

use super::conpty::ConPtySession;
use super::pane::Pane;
use crate::daemon::recovery::{self, PersistedPane, PersistedSession, PersistedState};
use crate::ipc::protocol::SessionInfo;

/// Metadata and panes for a single session.
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub created_at: SystemTime,
    pub panes: Vec<Pane>,
    pub active_pane: u32,
    /// Number of clients currently attached to this session.
    pub attached_clients: u32,
    /// Broadcast channel for ConPTY output — clients subscribe when attaching.
    /// The drain thread continuously reads ConPTY and sends here.
    pub output_tx: Option<tokio::sync::broadcast::Sender<Vec<u8>>>,
}

/// Manages all active terminal sessions.
pub struct SessionManager {
    sessions: HashMap<String, Session>,
    next_id: u32,
    /// Default shell from config, used when no explicit shell is specified.
    default_shell: Option<String>,
}

impl SessionManager {
    pub fn new(default_shell: Option<String>) -> Self {
        SessionManager {
            sessions: HashMap::new(),
            next_id: 1,
            default_shell,
        }
    }

    /// Create a new session, spawning a default pane with a shell via ConPTY.
    pub fn create_session(&mut self, name: Option<String>) -> Result<SessionInfo> {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let pane = Pane::new(0, 120, 30, self.default_shell.as_deref())
            .with_context(|| format!("Failed to create default pane for session {}", id))?;

        let created_at = SystemTime::now();

        info!(
            "Session created: id={}, name={:?}, pid={}",
            id,
            name,
            pane.process_id()
        );

        let session_info = SessionInfo {
            id: id.clone(),
            name: name.clone(),
            created_at: format_time(created_at),
            pane_count: 1,
        };

        // Create broadcast channel for ConPTY output (256 message buffer)
        let (output_tx, _) = tokio::sync::broadcast::channel(256);

        self.sessions.insert(
            id.clone(),
            Session {
                id,
                name,
                created_at,
                panes: vec![pane],
                active_pane: 0,
                attached_clients: 0,
                output_tx: Some(output_tx),
            },
        );

        // Persist state after structural change (best-effort)
        self.persist_state();

        Ok(session_info)
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .values()
            .map(|s| SessionInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                created_at: format_time(s.created_at),
                pane_count: s.panes.len() as u32,
            })
            .collect()
    }

    /// Kill a session by ID, terminating all its panes.
    pub fn kill_session(&mut self, id: &str) -> Result<()> {
        match self.sessions.remove(id) {
            Some(mut session) => {
                for pane in &mut session.panes {
                    if let Err(e) = pane.kill() {
                        tracing::error!(
                            "Failed to kill pane {} in session {}: {}",
                            pane.id(),
                            id,
                            e
                        );
                    }
                }
                info!("Session killed: id={}", id);

                // Persist state after structural change (best-effort)
                self.persist_state();

                Ok(())
            }
            None => {
                bail!("Session '{}' not found", id);
            }
        }
    }

    /// Add a new pane to an existing session. Returns (pane_id, pid).
    pub fn add_pane(
        &mut self,
        session_id: &str,
        cols: i16,
        rows: i16,
        shell: Option<&str>,
    ) -> Result<(u32, u32)> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;

        let next_pane_id = session.panes.iter().map(|p| p.id()).max().unwrap_or(0) + 1;
        let effective_shell = shell.or(self.default_shell.as_deref());
        let pane = Pane::new(next_pane_id, cols, rows, effective_shell).with_context(|| {
            format!(
                "Failed to create pane {} in session {}",
                next_pane_id, session_id
            )
        })?;

        let pid = pane.process_id();
        info!(
            "Pane added: session={}, pane_id={}, pid={}",
            session_id, next_pane_id, pid
        );

        session.panes.push(pane);

        self.persist_state();

        Ok((next_pane_id, pid))
    }

    /// Kill a specific pane. If it's the last pane, kills the entire session.
    pub fn kill_pane(&mut self, session_id: &str, pane_id: u32) -> Result<()> {
        // Check if session exists and if this is the last pane
        let is_last_pane = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;
            let _pane_idx = session
                .panes
                .iter()
                .position(|p| p.id() == pane_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Pane {} not found in session '{}'", pane_id, session_id)
                })?;
            session.panes.len() == 1
        };

        // If this is the last pane, kill the entire session
        if is_last_pane {
            return self.kill_session(session_id);
        }

        let session = self.sessions.get_mut(session_id).unwrap();
        let pane_idx = session
            .panes
            .iter()
            .position(|p| p.id() == pane_id)
            .unwrap();
        let mut pane = session.panes.remove(pane_idx);
        pane.kill()?;

        // If the active pane was killed, switch to the first available pane
        if session.active_pane == pane_id {
            session.active_pane = session.panes[0].id();
            session.panes[0].set_active(true);
        }

        info!("Pane killed: session={}, pane_id={}", session_id, pane_id);
        self.persist_state();

        Ok(())
    }

    /// Get a reference to the active pane in a session.
    pub fn get_active_pane(&self, session_id: &str) -> Option<&Pane> {
        self.sessions
            .get(session_id)
            .and_then(|s| s.panes.iter().find(|p| p.id() == s.active_pane))
    }

    /// Get a mutable reference to the active pane in a session.
    pub fn get_active_pane_mut(&mut self, session_id: &str) -> Option<&mut Pane> {
        self.sessions.get_mut(session_id).and_then(|s| {
            let active = s.active_pane;
            s.panes.iter_mut().find(|p| p.id() == active)
        })
    }

    /// Set which pane is focused in a session.
    pub fn set_active_pane(&mut self, session_id: &str, pane_id: u32) -> Result<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;

        let pane_exists = session.panes.iter().any(|p| p.id() == pane_id);
        if !pane_exists {
            bail!("Pane {} not found in session '{}'", pane_id, session_id);
        }

        // Deactivate old, activate new
        for pane in &mut session.panes {
            pane.set_active(pane.id() == pane_id);
        }
        session.active_pane = pane_id;

        Ok(())
    }

    /// Resize a specific pane's ConPTY pseudo console.
    pub fn resize_pane(
        &mut self,
        session_id: &str,
        pane_id: u32,
        cols: i16,
        rows: i16,
    ) -> Result<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;

        let pane = session
            .panes
            .iter_mut()
            .find(|p| p.id() == pane_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Pane {} not found in session '{}'", pane_id, session_id)
            })?;

        pane.conpty_mut().resize(cols, rows)?;
        info!(
            "Pane resized: session={}, pane_id={}, cols={}, rows={}",
            session_id, pane_id, cols, rows
        );

        Ok(())
    }

    /// Return the number of active sessions.
    pub fn session_count(&self) -> u32 {
        self.sessions.len() as u32
    }

    /// Get all session IDs.
    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Get a reference to a session by ID.
    #[allow(dead_code)]
    pub fn get_session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Get a mutable reference to a session by ID.
    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    /// Get a mutable reference to the active pane's ConPTY for a given session.
    pub fn get_active_conpty_mut(&mut self, session_id: &str) -> Option<&mut ConPtySession> {
        self.sessions.get_mut(session_id).and_then(|s| {
            let active = s.active_pane;
            s.panes
                .iter_mut()
                .find(|p| p.id() == active)
                .map(|p| p.conpty_mut())
        })
    }

    /// Increment the attached client count for a session.
    pub fn attach_client(&mut self, session_id: &str) -> Result<u32> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;
        session.attached_clients += 1;
        info!(
            "Client attached to session {}: {} clients",
            session_id, session.attached_clients
        );
        Ok(session.attached_clients)
    }

    /// Decrement the attached client count for a session.
    pub fn detach_client(&mut self, session_id: &str) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.attached_clients = session.attached_clients.saturating_sub(1);
            info!(
                "Client detached from session {}: {} clients",
                session_id, session.attached_clients
            );
        }
    }

    /// Restore a session with a specific ID (used during crash recovery).
    /// Creates a session with a single default pane from the given ConPTY.
    pub fn restore_session(&mut self, id: String, name: Option<String>, conpty: ConPtySession) {
        let created_at = SystemTime::now();
        info!(
            "Session restored: id={}, name={:?}, pid={}",
            id,
            name,
            conpty.process_id()
        );

        // Wrap the restored ConPTY in a Pane
        let pane = Pane::from_conpty(0, conpty);
        let (output_tx, _) = tokio::sync::broadcast::channel(256);

        self.sessions.insert(
            id.clone(),
            Session {
                id,
                name,
                created_at,
                panes: vec![pane],
                active_pane: 0,
                attached_clients: 0,
                output_tx: Some(output_tx),
            },
        );
    }

    /// Set the next session ID counter (used during crash recovery).
    pub fn set_next_id(&mut self, next_id: u32) {
        self.next_id = next_id;
    }

    /// Convert current state to a persistable format.
    pub fn to_persisted_state(&self) -> PersistedState {
        let sessions: Vec<PersistedSession> = self
            .sessions
            .values()
            .map(|s| {
                let persisted_panes: Vec<PersistedPane> = s
                    .panes
                    .iter()
                    .map(|p| PersistedPane {
                        id: p.id(),
                        pid: p.process_id(),
                        shell: p.conpty().shell().to_string(),
                        cols: p.conpty().cols(),
                        rows: p.conpty().rows(),
                    })
                    .collect();

                // First pane used for legacy fields (backward compat)
                let first = &s.panes[0];
                PersistedSession {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    pid: first.process_id(),
                    created_at: format_time(s.created_at),
                    shell: first.conpty().shell().to_string(),
                    cols: first.conpty().cols(),
                    rows: first.conpty().rows(),
                    panes: persisted_panes,
                }
            })
            .collect();

        PersistedState {
            version: 1,
            sessions,
            next_id: self.next_id,
            saved_at: format_time(SystemTime::now()),
        }
    }

    /// Persist state to disk (best-effort -- logs errors but does not fail).
    fn persist_state(&self) {
        let state = self.to_persisted_state();
        if let Err(e) = recovery::save_state(&state) {
            tracing::error!("Failed to persist state: {}", e);
        }
    }

    /// Kill all sessions. Used during daemon shutdown.
    pub fn kill_all(&mut self) {
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        for id in ids {
            if let Err(e) = self.kill_session(&id) {
                tracing::error!("Failed to kill session {}: {}", id, e);
            }
        }
    }
}

/// Format a SystemTime as a human-readable string.
fn format_time(time: SystemTime) -> String {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let hours = (secs / 3600) % 24;
            let minutes = (secs / 60) % 60;
            let seconds = secs % 60;
            // Simple UTC timestamp
            format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
        }
        Err(_) => "unknown".to_string(),
    }
}
