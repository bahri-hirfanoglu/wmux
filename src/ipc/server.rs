use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::{watch, Mutex};
use tracing::{error, info};
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::protocol::{read_message, write_message, Request, Response};
use crate::session::SessionManager;

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
/// Reads a Request, processes it, sends a Response, then disconnects.
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
        _ => Response::Error {
            message: "Command not yet implemented".to_string(),
        },
    };

    write_message(&mut writer, &response).await?;
    info!("Sent response, disconnecting client");

    Ok(())
}
