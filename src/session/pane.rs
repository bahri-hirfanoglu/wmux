use std::time::SystemTime;

use anyhow::{Context, Result};

use super::conpty::ConPtySession;
use super::scrollback::ScrollbackBuffer;

/// A single pane within a session, owning its own ConPTY process.
pub struct Pane {
    /// Pane index within the session (0-based).
    id: u32,
    /// The ConPTY session backing this pane.
    conpty: ConPtySession,
    /// When this pane was created.
    created_at: SystemTime,
    /// Whether this pane is the currently focused pane in the session.
    active: bool,
    /// Scrollback buffer for terminal output history.
    scrollback: ScrollbackBuffer,
}

impl Pane {
    /// Create a new pane, spawning a shell via ConPTY.
    pub fn new(id: u32, cols: i16, rows: i16, shell: Option<&str>) -> Result<Self> {
        let conpty = ConPtySession::new(cols, rows, shell)
            .with_context(|| format!("Failed to create ConPTY for pane {}", id))?;

        Ok(Pane {
            id,
            conpty,
            created_at: SystemTime::now(),
            active: id == 0, // First pane is active by default
            scrollback: ScrollbackBuffer::new(10_000),
        })
    }

    /// Create a pane from an existing ConPTY session (used during crash recovery).
    pub fn from_conpty(id: u32, conpty: ConPtySession) -> Self {
        Pane {
            id,
            conpty,
            created_at: SystemTime::now(),
            active: id == 0,
            scrollback: ScrollbackBuffer::new(10_000),
        }
    }

    /// Return the pane index.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Return the child process ID.
    pub fn process_id(&self) -> u32 {
        self.conpty.process_id()
    }

    /// Check if the child process is still running.
    pub fn is_alive(&self) -> bool {
        self.conpty.is_alive()
    }

    /// Terminate the pane's shell process.
    pub fn kill(&mut self) -> Result<()> {
        self.conpty.kill()
    }

    /// Whether this pane is the active (focused) pane in its session.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Set whether this pane is the active pane.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Get a reference to the underlying ConPTY session.
    pub fn conpty(&self) -> &ConPtySession {
        &self.conpty
    }

    /// Get a mutable reference to the underlying ConPTY session.
    pub fn conpty_mut(&mut self) -> &mut ConPtySession {
        &mut self.conpty
    }

    /// When this pane was created.
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Get a reference to the pane's scrollback buffer.
    pub fn scrollback(&self) -> &ScrollbackBuffer {
        &self.scrollback
    }

    /// Get a mutable reference to the pane's scrollback buffer.
    pub fn scrollback_mut(&mut self) -> &mut ScrollbackBuffer {
        &mut self.scrollback
    }
}

// Drop is handled by ConPtySession's own Drop implementation,
// which will be called automatically when Pane is dropped.
