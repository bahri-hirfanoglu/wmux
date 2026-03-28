use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use tracing::info;

use super::conpty::ConPtySession;
use crate::ipc::protocol::SessionInfo;

/// Metadata and ConPTY handle for a single session.
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub created_at: SystemTime,
    pub conpty: ConPtySession,
}

/// Manages all active terminal sessions.
pub struct SessionManager {
    sessions: HashMap<String, Session>,
    next_id: u32,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new session, spawning a shell via ConPTY.
    pub fn create_session(&mut self, name: Option<String>) -> Result<SessionInfo> {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let conpty = ConPtySession::new(120, 30, None)
            .with_context(|| format!("Failed to create ConPTY session {}", id))?;

        let created_at = SystemTime::now();

        info!(
            "Session created: id={}, name={:?}, pid={}",
            id,
            name,
            conpty.process_id()
        );

        let session_info = SessionInfo {
            id: id.clone(),
            name: name.clone(),
            created_at: format_time(created_at),
        };

        self.sessions.insert(
            id.clone(),
            Session {
                id,
                name,
                created_at,
                conpty,
            },
        );

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
            })
            .collect()
    }

    /// Kill a session by ID, terminating its shell process.
    pub fn kill_session(&mut self, id: &str) -> Result<()> {
        match self.sessions.remove(id) {
            Some(mut session) => {
                session.conpty.kill()?;
                info!("Session killed: id={}", id);
                Ok(())
            }
            None => {
                bail!("Session '{}' not found", id);
            }
        }
    }

    /// Return the number of active sessions.
    pub fn session_count(&self) -> u32 {
        self.sessions.len() as u32
    }

    /// Get a reference to a session by ID.
    #[allow(dead_code)]
    pub fn get_session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Restore a session with a specific ID (used during crash recovery).
    pub fn restore_session(&mut self, id: String, name: Option<String>, conpty: ConPtySession) {
        let created_at = SystemTime::now();
        info!("Session restored: id={}, name={:?}, pid={}", id, name, conpty.process_id());
        self.sessions.insert(
            id.clone(),
            Session {
                id,
                name,
                created_at,
                conpty,
            },
        );
    }

    /// Set the next session ID counter (used during crash recovery).
    pub fn set_next_id(&mut self, next_id: u32) {
        self.next_id = next_id;
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
