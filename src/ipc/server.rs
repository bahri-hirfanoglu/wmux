use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::{watch, Mutex};
use tracing::{error, info, warn};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::protocol::{read_message, write_message, Request, Response};
use crate::session::SessionManager;

/// Number of scrollback lines to send per scroll mode page.
const SCROLL_PAGE_SIZE: usize = 50;

pub struct ControlServer;

impl ControlServer {
    /// Start the Named Pipe control server.
    ///
    /// Listens on \\.\pipe\wmux-ctl for client connections.
    /// Each connection is handled in a spawned task.
    /// Exits when the shutdown signal is received.
    pub async fn start(
        pipe_name: &str,
        mut shutdown_rx: watch::Receiver<bool>,
        shutdown_tx: watch::Sender<bool>,
        session_manager: Arc<Mutex<SessionManager>>,
    ) -> Result<()> {
        info!("Control server starting on {}", pipe_name);

        loop {
            // Create a new pipe instance for the next client
            let server = ServerOptions::new()
                .first_pipe_instance(false)
                .create(pipe_name)
                .context("Failed to create Named Pipe server")?;

            // Wait for either a client connection or shutdown signal
            tokio::select! {
                result = server.connect() => {
                    match result {
                        Ok(()) => {
                            info!("Client connected to control pipe");
                            let shutdown_tx_clone = shutdown_tx.clone();
                            let sm = session_manager.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(server, shutdown_tx_clone, sm).await {
                                    error!("Error handling client connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept client connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Control server received shutdown signal");
                        break;
                    }
                }
            }
        }

        // Clean up all sessions before exiting
        {
            let mut mgr = session_manager.lock().await;
            let count = mgr.session_count();
            if count > 0 {
                info!("Cleaning up {} sessions before shutdown", count);
                mgr.kill_all();
            }
        }

        info!("Control server stopped");
        Ok(())
    }
}

/// Handle a single client connection.
///
/// For most requests: reads a Request, processes it, sends a Response, then disconnects.
/// For AttachSession: enters a long-lived bidirectional streaming loop.
async fn handle_connection(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    shutdown_tx: watch::Sender<bool>,
    session_manager: Arc<Mutex<SessionManager>>,
) -> Result<()> {
    let (mut reader, mut writer) = tokio::io::split(pipe);

    let request: Request = read_message(&mut reader)
        .await
        .context("Failed to read request from client")?;

    info!("Received request: {:?}", request);

    // Check if this is an attach request — it needs special long-lived handling
    if let Request::AttachSession { session_id } = request {
        return handle_attach(reader, writer, session_id, session_manager).await;
    }

    let response = match request {
        Request::Ping => Response::Pong,
        Request::Status => {
            let pid = unsafe { GetCurrentProcessId() };
            let mgr = session_manager.lock().await;
            Response::Status {
                running: true,
                pid,
                session_count: mgr.session_count(),
            }
        }
        Request::KillServer => {
            let resp = Response::Ok {
                message: "Server shutting down".to_string(),
            };
            write_message(&mut writer, &resp).await?;
            info!("KillServer requested — signaling shutdown");
            let _ = shutdown_tx.send(true);
            return Ok(());
        }
        Request::NewSession { name } => {
            let mut mgr = session_manager.lock().await;
            match mgr.create_session(name) {
                Ok(info) => {
                    let sid = info.id.clone();
                    // Start background drain thread for this session's ConPTY output.
                    // This ensures the output pipe is always read, preventing shell blocking.
                    start_conpty_drain(&mgr, &sid, session_manager.clone());
                    Response::Ok {
                        message: format!("Created session: {}", sid),
                    }
                },
                Err(e) => Response::Error {
                    message: format!("Failed to create session: {}", e),
                },
            }
        }
        Request::ListSessions => {
            let mgr = session_manager.lock().await;
            Response::SessionList {
                sessions: mgr.list_sessions(),
            }
        }
        Request::KillSession { id } => {
            let mut mgr = session_manager.lock().await;
            match mgr.kill_session(&id) {
                Ok(()) => Response::Ok {
                    message: format!("Session {} killed", id),
                },
                Err(e) => Response::Error {
                    message: format!("{}", e),
                },
            }
        }
        Request::SplitPane { session_id, direction: _direction } => {
            let mut mgr = session_manager.lock().await;
            match mgr.add_pane(&session_id, 120, 30, None) {
                Ok(pane_info) => Response::PaneInfo {
                    session_id,
                    pane_id: pane_info.0,
                    pid: pane_info.1,
                },
                Err(e) => Response::Error {
                    message: format!("Failed to split pane: {}", e),
                },
            }
        }
        Request::KillPane { session_id, pane_id } => {
            let mut mgr = session_manager.lock().await;
            match mgr.kill_pane(&session_id, pane_id) {
                Ok(()) => Response::Ok {
                    message: format!("Pane {} killed in session {}", pane_id, session_id),
                },
                Err(e) => Response::Error {
                    message: format!("{}", e),
                },
            }
        }
        Request::ResizePane { session_id, pane_id, cols, rows } => {
            let mut mgr = session_manager.lock().await;
            match mgr.resize_pane(&session_id, pane_id, cols, rows) {
                Ok(()) => Response::Ok {
                    message: format!("Pane {} resized to {}x{}", pane_id, cols, rows),
                },
                Err(e) => Response::Error {
                    message: format!("{}", e),
                },
            }
        }
        Request::EnterScrollMode { session_id, pane_id } => {
            let mgr = session_manager.lock().await;
            build_scroll_response(&mgr, &session_id, pane_id, None)
                .unwrap_or_else(|| Response::Error {
                    message: format!("Pane {} not found in session '{}'", pane_id, session_id),
                })
        }
        Request::ScrollBack { session_id, pane_id, lines } => {
            let mgr = session_manager.lock().await;
            build_scroll_response(&mgr, &session_id, pane_id, Some(lines))
                .unwrap_or_else(|| Response::Error {
                    message: format!("Pane {} not found in session '{}'", pane_id, session_id),
                })
        }
        Request::ExitScrollMode { session_id: _, pane_id: _ } => {
            Response::Ok {
                message: "Exited scroll mode".to_string(),
            }
        }
        // AttachSession is handled above, but the compiler needs this arm
        Request::AttachSession { .. } => unreachable!(),
        _ => Response::Error {
            message: "Command not yet implemented".to_string(),
        },
    };

    write_message(&mut writer, &response).await?;
    info!("Sent response, disconnecting client");

    Ok(())
}

/// Handle a long-lived attach session with bidirectional I/O streaming.
///
/// IMPORTANT: The SessionManager mutex is NOT held across await points.
/// We lock briefly to extract raw HANDLE values (which are Copy), then
/// use the handles directly in the streaming loop.
async fn handle_attach<R, W>(
    reader: R,
    mut writer: W,
    session_id: String,
    session_manager: Arc<Mutex<SessionManager>>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. Validate session and get pipe handles as raw isize (brief lock).
    //    HANDLE contains *mut c_void which is !Send, so we extract the raw
    //    pointer value as isize (which IS Send) and reconstruct HANDLE inside
    //    spawn_blocking closures.
    //
    //    IMPORTANT: No iterators over Pane (which contains HANDLE) may be alive
    //    across await points, because the future must be Send for tokio::spawn.
    let (pipe_in_raw, pane_count) = {
        let mut mgr = session_manager.lock().await;
        let session = match mgr.get_session_mut(&session_id) {
            Some(s) => s,
            None => {
                return send_error_and_return(&mut writer, format!("Session '{}' not found", session_id)).await;
            }
        };

        let pane_count = session.panes.len() as u32;
        let active_id = session.active_pane;

        let pipe_in = session
            .panes
            .iter()
            .find(|p| p.id() == active_id)
            .map(|p| p.conpty().pipe_in_handle().0 as isize);

        match pipe_in {
            Some(pin) => {
                info!("Attach: session={}, pane={}", session_id, active_id);
                mgr.attach_client(&session_id)?;
                (pin, pane_count)
            }
            None => {
                return send_error_and_return(
                    &mut writer,
                    format!("No active pane in session '{}'", session_id),
                ).await;
            }
        }
    }; // mutex released here

    // 2. Send AttachStarted confirmation
    let resp = Response::AttachStarted {
        session_id: session_id.clone(),
        pane_count,
    };
    write_message(&mut writer, &resp).await?;
    info!("Attach started for session {}", session_id);

    // 3. Subscribe to the session's broadcast channel for ConPTY output.
    let mut output_rx = {
        let mgr = session_manager.lock().await;
        match mgr.get_session(&session_id) {
            Some(session) => match &session.output_tx {
                Some(tx) => tx.subscribe(),
                None => {
                    return send_error_and_return(&mut writer, format!("Session '{}' has no output channel", session_id)).await;
                }
            },
            None => {
                return send_error_and_return(&mut writer, format!("Session '{}' not found", session_id)).await;
            }
        }
    };

    // 4. Send buffered scrollback content so client sees the current screen state.
    {
        let mgr = session_manager.lock().await;
        if let Some(pane) = mgr.get_active_pane(&session_id) {
            let sb = pane.scrollback();
            let total = sb.line_count();
            if total > 0 {
                let start = total.saturating_sub(50);
                let lines = sb.get_lines(start, total - start);
                let mut replay_data = Vec::new();
                for line in &lines {
                    replay_data.extend_from_slice(line);
                    replay_data.push(b'\n');
                }
                if !replay_data.is_empty() {
                    let resp = Response::SessionOutput { data: replay_data };
                    let _ = write_message(&mut writer, &resp).await;
                }
            }
        }
    }

    // 5. Spawn a dedicated client reader task.
    //    This is CRITICAL: read_message is NOT cancel-safe in tokio::select!.
    //    If the output branch wins, a partial read_message would be dropped,
    //    corrupting the Named Pipe protocol stream.
    //    By using a dedicated task + channel, reads are never cancelled.
    let (client_tx, mut client_rx) = tokio::sync::mpsc::channel::<Request>(32);
    let client_reader = tokio::spawn(async move {
        let mut reader = reader;
        loop {
            match read_message::<_, Request>(&mut reader).await {
                Ok(req) => {
                    if client_tx.send(req).await.is_err() {
                        break; // receiver dropped — attach ending
                    }
                }
                Err(_) => break, // client disconnected
            }
        }
    });

    // 6. Enter bidirectional streaming loop.
    //    Both branches now read from channels — no cancel-safety issues.
    loop {
        tokio::select! {
            // ConPTY output from drain thread via broadcast channel
            output_result = output_rx.recv() => {
                match output_result {
                    Ok(data) => {
                        let resp = Response::SessionOutput { data };
                        if let Err(e) = write_message(&mut writer, &resp).await {
                            warn!("Failed to send output to client: {}", e);
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client lagged behind by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("ConPTY output channel closed for session {}", session_id);
                        break;
                    }
                }
            }

            // Client sent a message via dedicated reader task
            client_result = client_rx.recv() => {
                match client_result {
                    Some(Request::SessionInput { data }) => {
                        // Write input to ConPTY
                        let in_raw = pipe_in_raw;
                        let write_result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                            let handle = HANDLE(in_raw as *mut _);
                            let mut written: u32 = 0;
                            unsafe {
                                WriteFile(handle, Some(&data), Some(&mut written), None)
                                    .context("WriteFile to ConPTY input pipe failed")?;
                            }
                            Ok(())
                        }).await;

                        match write_result {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => {
                                warn!("Failed to write input to ConPTY: {}", e);
                                break;
                            }
                            Err(e) => {
                                error!("ConPTY write task panicked: {}", e);
                                break;
                            }
                        }
                    }
                    Some(Request::DetachSession { .. }) => {
                        info!("Client requested detach from session {}", session_id);
                        break;
                    }
                    Some(Request::NavigatePane { session_id: sid, direction }) => {
                        let mut mgr = session_manager.lock().await;
                        if let Some(session) = mgr.get_session_mut(&sid) {
                            let current_idx = session.panes.iter().position(|p| p.id() == session.active_pane).unwrap_or(0);
                            let pane_count = session.panes.len();
                            if pane_count > 1 {
                                use crate::ipc::protocol::NavDirection;
                                let new_idx = match direction {
                                    NavDirection::Left | NavDirection::Up => {
                                        if current_idx == 0 { pane_count - 1 } else { current_idx - 1 }
                                    }
                                    NavDirection::Right | NavDirection::Down => {
                                        (current_idx + 1) % pane_count
                                    }
                                };
                                let new_pane_id = session.panes[new_idx].id();
                                session.active_pane = new_pane_id;
                                for pane in &mut session.panes {
                                    pane.set_active(pane.id() == new_pane_id);
                                }
                            }
                        }
                    }
                    Some(Request::SplitPane { session_id: sid, direction: _ }) => {
                        let mut mgr = session_manager.lock().await;
                        match mgr.add_pane(&sid, 120, 30, None) {
                            Ok((pane_id, _pid)) => {
                                let resp = Response::PaneInfo {
                                    session_id: sid.clone(),
                                    pane_id,
                                    pid: _pid,
                                };
                                let _ = write_message(&mut writer, &resp).await;
                            }
                            Err(e) => {
                                warn!("Failed to split pane: {}", e);
                            }
                        }
                    }
                    Some(Request::KillPane { session_id: sid, pane_id }) => {
                        let mut mgr = session_manager.lock().await;
                        let _ = mgr.kill_pane(&sid, pane_id);
                    }
                    Some(Request::ResizePane { session_id: sid, pane_id, cols, rows }) => {
                        let mut mgr = session_manager.lock().await;
                        let _ = mgr.resize_pane(&sid, pane_id, cols, rows);
                    }
                    Some(Request::EnterScrollMode { session_id: sid, pane_id }) => {
                        let resp = {
                            let mgr = session_manager.lock().await;
                            build_scroll_response(&mgr, &sid, pane_id, None)
                        };
                        if let Some(r) = resp {
                            let _ = write_message(&mut writer, &r).await;
                        }
                    }
                    Some(Request::ScrollBack { session_id: sid, pane_id, lines }) => {
                        let resp = {
                            let mgr = session_manager.lock().await;
                            build_scroll_response(&mgr, &sid, pane_id, Some(lines))
                        };
                        if let Some(r) = resp {
                            let _ = write_message(&mut writer, &r).await;
                        }
                    }
                    Some(Request::ExitScrollMode { .. }) => {}
                    Some(_) => {}
                    None => {
                        // Client reader task ended — client disconnected
                        info!("Client disconnected from session {}", session_id);
                        break;
                    }
                }
            }
        }
    }

    // 7. Clean up
    drop(output_rx);
    drop(client_rx);
    client_reader.abort();

    // Decrement attached clients, send Ok response for detach
    {
        let mut mgr = session_manager.lock().await;
        mgr.detach_client(&session_id);
    }

    // Try to send a final Ok response (may fail if client already disconnected)
    let _ = write_message(&mut writer, &Response::Ok {
        message: format!("Detached from session {}", session_id),
    }).await;

    info!("Attach handler finished for session {}", session_id);
    Ok(())
}

/// Start a background thread that continuously reads ConPTY output for a session.
///
/// This prevents the output pipe from filling up (which would block the shell).
/// Output is sent to the session's broadcast channel (for attached clients) and
/// stored in the scrollback buffer.
pub fn start_conpty_drain(
    mgr: &SessionManager,
    session_id: &str,
    session_manager: Arc<Mutex<SessionManager>>,
) {
    let session = match mgr.get_session(session_id) {
        Some(s) => s,
        None => return,
    };

    // Get the active pane's output pipe handle
    let active_id = session.active_pane;
    let pipe_out_raw = match session.panes.iter().find(|p| p.id() == active_id) {
        Some(pane) => pane.conpty().pipe_out_handle().0 as isize,
        None => return,
    };

    // Get the broadcast sender
    let output_tx = match &session.output_tx {
        Some(tx) => tx.clone(),
        None => return,
    };

    let sid = session_id.to_string();

    // Spawn a permanent reader thread for this session
    std::thread::spawn(move || {
        let handle = HANDLE(pipe_out_raw as *mut _);
        info!("ConPTY drain thread started for session {}", sid);
        loop {
            let mut buf = vec![0u8; 4096];
            let mut bytes_read: u32 = 0;
            let result = unsafe {
                ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None)
            };
            match result {
                Ok(()) if bytes_read > 0 => {
                    buf.truncate(bytes_read as usize);

                    // Store in scrollback (best-effort — lock briefly)
                    if let Ok(mut mgr) = session_manager.try_lock() {
                        if let Some(pane) = mgr.get_active_pane_mut(&sid) {
                            pane.scrollback_mut().push_bytes(&buf);
                        }
                    }

                    // Broadcast to any attached clients (ignore if no receivers)
                    let _ = output_tx.send(buf);
                }
                _ => {
                    info!("ConPTY drain thread ending for session {} (pipe closed)", sid);
                    break;
                }
            }
        }
    });
}

/// Build a scroll mode response from the scrollback buffer.
///
/// If `lines` is None, returns the last page (enter scroll mode).
/// If `lines` is Some(offset), returns data starting at that offset.
///
/// Returns None if the session/pane is not found.
/// IMPORTANT: This function must NOT be async — it borrows Pane (contains HANDLE)
/// which is !Send. All data is extracted synchronously while the lock is held.
fn build_scroll_response(
    mgr: &SessionManager,
    session_id: &str,
    pane_id: u32,
    lines: Option<i32>,
) -> Option<Response> {
    let session = mgr.get_session(session_id)?;
    let pane = session.panes.iter().find(|p| p.id() == pane_id)?;
    let sb = pane.scrollback();
    let total = sb.line_count();

    let offset = match lines {
        None => total.saturating_sub(SCROLL_PAGE_SIZE),
        Some(l) => (l.max(0) as usize).min(total.saturating_sub(1)),
    };
    let count = SCROLL_PAGE_SIZE.min(total.saturating_sub(offset));
    let scroll_lines = sb.get_lines(offset, count);

    let mut data = Vec::new();
    for line in &scroll_lines {
        data.extend_from_slice(line);
        data.push(b'\n');
    }

    Some(Response::ScrollModeData {
        data,
        offset,
        total_lines: total,
    })
}

/// Helper: send an error response and return Ok(()).
async fn send_error_and_return<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    message: String,
) -> Result<()> {
    let resp = Response::Error { message };
    let _ = write_message(writer, &resp).await;
    Ok(())
}
