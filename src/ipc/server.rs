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
    mut pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    shutdown_tx: watch::Sender<bool>,
    session_manager: Arc<Mutex<SessionManager>>,
) -> Result<()> {
    let (mut reader, mut writer) = tokio::io::split(&mut pipe);

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
                Ok(info) => Response::Ok {
                    message: format!("Created session: {}", info.id),
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
            match mgr.get_session(&session_id) {
                Some(session) => {
                    match session.panes.iter().find(|p| p.id() == pane_id) {
                        Some(pane) => {
                            let sb = pane.scrollback();
                            let total = sb.line_count();
                            let start = total.saturating_sub(50);
                            let count = total - start;
                            let lines = sb.get_lines(start, count);
                            let mut data = Vec::new();
                            for line in &lines {
                                data.extend_from_slice(line);
                                data.push(b'\n');
                            }
                            Response::ScrollModeData {
                                data,
                                offset: start,
                                total_lines: total,
                            }
                        }
                        None => Response::Error {
                            message: format!("Pane {} not found in session '{}'", pane_id, session_id),
                        },
                    }
                }
                None => Response::Error {
                    message: format!("Session '{}' not found", session_id),
                },
            }
        }
        Request::ScrollBack { session_id, pane_id, lines } => {
            let mgr = session_manager.lock().await;
            match mgr.get_session(&session_id) {
                Some(session) => {
                    match session.panes.iter().find(|p| p.id() == pane_id) {
                        Some(pane) => {
                            let sb = pane.scrollback();
                            let total = sb.line_count();
                            let offset = (lines.max(0) as usize).min(total.saturating_sub(1));
                            let count = 50.min(total.saturating_sub(offset));
                            let scroll_lines = sb.get_lines(offset, count);
                            let mut data = Vec::new();
                            for line in &scroll_lines {
                                data.extend_from_slice(line);
                                data.push(b'\n');
                            }
                            Response::ScrollModeData {
                                data,
                                offset,
                                total_lines: total,
                            }
                        }
                        None => Response::Error {
                            message: format!("Pane {} not found", pane_id),
                        },
                    }
                }
                None => Response::Error {
                    message: format!("Session '{}' not found", session_id),
                },
            }
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
    mut reader: R,
    mut writer: W,
    session_id: String,
    session_manager: Arc<Mutex<SessionManager>>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. Validate session and get pipe handles as raw isize (brief lock).
    //    HANDLE contains *mut c_void which is !Send, so we extract the raw
    //    pointer value as isize (which IS Send) and reconstruct HANDLE inside
    //    spawn_blocking closures.
    //
    //    IMPORTANT: No iterators over Pane (which contains HANDLE) may be alive
    //    across await points, because the future must be Send for tokio::spawn.
    let (pipe_in_raw, pipe_out_raw, pane_count) = {
        let mut mgr = session_manager.lock().await;
        let session = match mgr.get_session_mut(&session_id) {
            Some(s) => s,
            None => {
                return send_error_and_return(&mut writer, format!("Session '{}' not found", session_id)).await;
            }
        };

        let pane_count = session.panes.len() as u32;
        let active_id = session.active_pane;

        // Extract pipe handles without holding an iterator across await points.
        // The iterator over Pane (which contains HANDLE, a !Send type) must not
        // be alive across any await point.
        let handles: Option<(isize, isize)> = session
            .panes
            .iter()
            .find(|p| p.id() == active_id)
            .map(|p| {
                let pin = p.conpty().pipe_in_handle().0 as isize;
                let pout = p.conpty().pipe_out_handle().0 as isize;
                (pin, pout)
            });

        match handles {
            Some((pin, pout)) => {
                mgr.attach_client(&session_id)?;
                (pin, pout, pane_count)
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

    // 3. Enter bidirectional streaming loop
    //    - Output task: read from ConPTY output pipe, send to client
    //    - Input task: read requests from client, forward input to ConPTY
    // We use two separate async operations in a select loop.
    // ConPTY reads use spawn_blocking (anonymous pipes don't support overlapped I/O).
    loop {
        // Spawn a blocking read from ConPTY output
        let out_raw = pipe_out_raw;
        let output_task = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let handle = HANDLE(out_raw as *mut _);
            let mut tmp = vec![0u8; 4096];
            let mut bytes_read: u32 = 0;
            unsafe {
                ReadFile(handle, Some(&mut tmp), Some(&mut bytes_read), None)
                    .context("ReadFile from ConPTY output pipe failed")?;
            }
            tmp.truncate(bytes_read as usize);
            Ok(tmp)
        });

        // Read the next message from the client (non-blocking async pipe)
        let input_task = read_message::<_, Request>(&mut reader);

        tokio::select! {
            // ConPTY produced output — forward to client
            output_result = output_task => {
                match output_result {
                    Ok(Ok(data)) if !data.is_empty() => {
                        // Capture output in scrollback buffer (brief lock)
                        {
                            let mut mgr = session_manager.lock().await;
                            if let Some(pane) = mgr.get_active_pane_mut(&session_id) {
                                pane.scrollback_mut().push_bytes(&data);
                            }
                        }
                        let resp = Response::SessionOutput { data };
                        if let Err(e) = write_message(&mut writer, &resp).await {
                            warn!("Failed to send output to client: {}", e);
                            break; // Client disconnected
                        }
                    }
                    Ok(Ok(_)) => {
                        // Empty read — ConPTY pipe closed (shell exited)
                        info!("ConPTY output pipe closed for session {}", session_id);
                        break;
                    }
                    Ok(Err(e)) => {
                        // ReadFile error — pipe broken (shell exited or crashed)
                        info!("ConPTY read error for session {}: {}", session_id, e);
                        break;
                    }
                    Err(e) => {
                        error!("spawn_blocking panicked: {}", e);
                        break;
                    }
                }
            }

            // Client sent a message — process it
            input_result = input_task => {
                match input_result {
                    Ok(Request::SessionInput { data }) => {
                        // Write input to ConPTY
                        let in_raw = pipe_in_raw;
                        let write_result = tokio::task::spawn_blocking(move || -> Result<()> {
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
                                error!("spawn_blocking panicked on write: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(Request::DetachSession { .. }) => {
                        info!("Client requested detach from session {}", session_id);
                        break;
                    }
                    Ok(Request::NavigatePane { session_id: sid, direction }) => {
                        // Update daemon's active pane tracking (best-effort directional mapping)
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
                                info!("Navigated to pane {} in session {}", new_pane_id, sid);
                            }
                        }
                    }
                    Ok(Request::SplitPane { session_id: sid, direction: _ }) => {
                        // Create a new pane on the daemon side
                        let mut mgr = session_manager.lock().await;
                        match mgr.add_pane(&sid, 120, 30, None) {
                            Ok((pane_id, _pid)) => {
                                // Send PaneInfo back so client can invoke wt.exe split-pane
                                let resp = Response::PaneInfo {
                                    session_id: sid.clone(),
                                    pane_id,
                                    pid: _pid,
                                };
                                let _ = write_message(&mut writer, &resp).await;
                                info!("Split pane {} created in session {} (via prefix key)", pane_id, sid);
                            }
                            Err(e) => {
                                warn!("Failed to split pane in session {}: {}", sid, e);
                            }
                        }
                    }
                    Ok(Request::KillPane { session_id: sid, pane_id }) => {
                        let mut mgr = session_manager.lock().await;
                        match mgr.kill_pane(&sid, pane_id) {
                            Ok(()) => {
                                info!("Pane {} killed in session {} (via prefix key)", pane_id, sid);
                            }
                            Err(e) => {
                                warn!("Failed to kill pane {} in session {}: {}", pane_id, sid, e);
                            }
                        }
                    }
                    Ok(Request::ResizePane { session_id: sid, pane_id, cols, rows }) => {
                        let mut mgr = session_manager.lock().await;
                        if let Err(e) = mgr.resize_pane(&sid, pane_id, cols, rows) {
                            warn!("Failed to resize pane {} in session {}: {}", pane_id, sid, e);
                        }
                    }
                    Ok(Request::EnterScrollMode { session_id: sid, pane_id }) => {
                        let resp = {
                            let mgr = session_manager.lock().await;
                            build_scroll_response(&mgr, &sid, pane_id, None)
                        };
                        if let Some(r) = resp {
                            let _ = write_message(&mut writer, &r).await;
                        }
                    }
                    Ok(Request::ScrollBack { session_id: sid, pane_id, lines }) => {
                        let resp = {
                            let mgr = session_manager.lock().await;
                            build_scroll_response(&mgr, &sid, pane_id, Some(lines))
                        };
                        if let Some(r) = resp {
                            let _ = write_message(&mut writer, &r).await;
                        }
                    }
                    Ok(Request::ExitScrollMode { .. }) => {
                        // Client exited scroll mode — resume normal streaming
                        info!("Client exited scroll mode for session {}", session_id);
                    }
                    Ok(other) => {
                        warn!("Unexpected request during attach: {:?}", other);
                    }
                    Err(e) => {
                        // Client disconnected (pipe broken/closed)
                        info!("Client disconnected from session {}: {}", session_id, e);
                        break;
                    }
                }
            }
        }
    }

    // 4. Clean up: decrement attached clients, send Ok response for detach
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

/// Helper: send an error response and return Ok(()).
async fn send_error_and_return<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    message: String,
) -> Result<()> {
    let resp = Response::Error { message };
    let _ = write_message(writer, &resp).await;
    Ok(())
}
