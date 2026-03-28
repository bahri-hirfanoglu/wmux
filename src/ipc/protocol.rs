use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Direction for splitting a pane.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Direction for navigating between panes.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Status,
    KillServer,
    NewSession { name: Option<String> },
    ListSessions,
    AttachSession { session_id: String },
    DetachSession { session_id: String },
    KillSession { id: String },
    SplitPane { session_id: String, direction: SplitDirection },
    KillPane { session_id: String, pane_id: u32 },
    NavigatePane { session_id: String, direction: NavDirection },
    ResizePane { session_id: String, pane_id: u32, cols: i16, rows: i16 },
    ScrollBack { session_id: String, pane_id: u32, lines: i32 },
    EnterScrollMode { session_id: String, pane_id: u32 },
    ExitScrollMode { session_id: String, pane_id: u32 },
    /// Raw input data from an attached client to forward to ConPTY stdin.
    SessionInput { data: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Response {
    Pong,
    Status {
        running: bool,
        pid: u32,
        session_count: u32,
    },
    Ok {
        message: String,
    },
    Error {
        message: String,
    },
    SessionList {
        sessions: Vec<SessionInfo>,
    },
    PaneInfo {
        session_id: String,
        pane_id: u32,
        pid: u32,
    },
    SessionOutput {
        data: Vec<u8>,
    },
    AttachStarted {
        session_id: String,
        pane_count: u32,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub pane_count: u32,
}

/// Write a length-prefixed JSON message to an async writer.
///
/// Format: [4-byte LE length][JSON bytes]
pub async fn write_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &impl Serialize,
) -> Result<()> {
    let json = serde_json::to_vec(msg).context("Failed to serialize message")?;
    let len = json.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .await
        .context("Failed to write message length")?;
    writer
        .write_all(&json)
        .await
        .context("Failed to write message body")?;
    writer.flush().await.context("Failed to flush writer")?;
    Ok(())
}

/// Read a length-prefixed JSON message from an async reader.
///
/// Format: [4-byte LE length][JSON bytes]
pub async fn read_message<R: AsyncReadExt + Unpin, T: DeserializeOwned>(
    reader: &mut R,
) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .await
        .context("Failed to read message length")?;
    let len = u32::from_le_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .await
        .context("Failed to read message body")?;

    serde_json::from_slice(&buf).context("Failed to deserialize message")
}
