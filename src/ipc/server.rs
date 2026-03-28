use anyhow::{Context, Result};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::watch;
use tracing::{error, info};
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::protocol::{read_message, write_message, Request, Response};

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
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(server, shutdown_tx_clone).await {
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
            Response::Status {
                running: true,
                pid,
                session_count: 0, // No sessions yet — Phase 2
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
        _ => Response::Error {
            message: "Command not yet implemented".to_string(),
        },
    };

    write_message(&mut writer, &response).await?;
    info!("Sent response, disconnecting client");

    Ok(())
}
