use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Status,
    KillServer,
    // Placeholders for Phase 2
    NewSession { name: Option<String> },
    ListSessions,
    AttachSession { id: String },
    DetachSession { id: String },
    KillSession { id: String },
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
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
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
