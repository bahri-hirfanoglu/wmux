mod cli;

use clap::Parser;
use cli::{Cli, Commands};

use wmux::daemon;
use wmux::paths;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Internal daemon mode — entered when the binary re-spawns itself
    if cli.daemon_mode {
        daemon::lifecycle::run_daemon().await?;
        return Ok(());
    }

    match cli.command {
        Some(Commands::DaemonStart) => {
            daemon::lifecycle::start_daemon().await?;
        }
        Some(Commands::Status) => {
            daemon::lifecycle::daemon_status().await?;
        }
        Some(Commands::KillServer) => {
            daemon::lifecycle::kill_server().await?;
        }
        Some(Commands::New) => {
            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::NewSession { name: None };
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::Ok { message }) => {
                    println!("{}", message);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("Unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to create session: {}. Is the daemon running?", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Ls) => {
            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::ListSessions;
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::SessionList { sessions }) => {
                    if sessions.is_empty() {
                        println!("No active sessions");
                    } else {
                        println!("{:<6} {:<20} {:<8} {}", "ID", "NAME", "PANES", "CREATED");
                        for s in &sessions {
                            println!(
                                "{:<6} {:<20} {:<8} {}",
                                s.id,
                                s.name.as_deref().unwrap_or("-"),
                                s.pane_count,
                                s.created_at
                            );
                        }
                        println!("\n{} session(s)", sessions.len());
                    }
                }
                Ok(other) => {
                    eprintln!("Unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to list sessions: {}. Is the daemon running?", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::KillSession { id }) => {
            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::KillSession { id };
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::Ok { message }) => {
                    println!("{}", message);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("Unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to kill session: {}. Is the daemon running?", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Attach { session_id, pane }) => {
            wmux::wt::require_windows_terminal()?;
            let pipe_name = paths::control_pipe();

            // If no session_id given, find the most recent session
            let sid = if let Some(id) = session_id {
                id
            } else {
                // Query the daemon for sessions and pick the most recent
                let request = wmux::ipc::protocol::Request::ListSessions;
                match wmux::ipc::client::send_request(&pipe_name, &request).await {
                    Ok(wmux::ipc::protocol::Response::SessionList { sessions }) => {
                        if sessions.is_empty() {
                            eprintln!("No active sessions to attach to");
                            std::process::exit(1);
                        }
                        // Pick the last one (most recently created)
                        sessions.last().unwrap().id.clone()
                    }
                    Ok(other) => {
                        eprintln!("Unexpected response: {:?}", other);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Failed to list sessions: {}. Is the daemon running?", e);
                        std::process::exit(1);
                    }
                }
            };

            // Set WMUX_SESSION_ID so child processes (like `wmux split`) know which session
            // Safety: we're in a single-threaded context here before entering the attach loop
            unsafe { std::env::set_var("WMUX_SESSION_ID", &sid); }
            if let Some(pane_id) = pane {
                unsafe { std::env::set_var("WMUX_PANE_ID", pane_id.to_string()); }
            }

            match wmux::ipc::client::attach_session(&pipe_name, &sid).await {
                Ok(()) => {
                    println!("Detached from session {}", sid);
                }
                Err(e) => {
                    eprintln!("Attach failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Detach) => {
            eprintln!("Detach is handled via Ctrl+B, d while attached to a session");
            std::process::exit(1);
        }
        Some(Commands::Split { horizontal, vertical }) => {
            wmux::wt::require_windows_terminal()?;
            if !horizontal && !vertical {
                eprintln!("Error: specify --horizontal (-h) or --vertical (-v)");
                std::process::exit(1);
            }

            // Get session ID from env var set during attach
            let session_id = match std::env::var("WMUX_SESSION_ID") {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Error: not attached to a session — run wmux split from within an attached session");
                    std::process::exit(1);
                }
            };

            let direction = if horizontal {
                wmux::ipc::protocol::SplitDirection::Horizontal
            } else {
                wmux::ipc::protocol::SplitDirection::Vertical
            };
            let direction_str = if horizontal { "horizontal" } else { "vertical" };

            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::SplitPane {
                session_id: session_id.clone(),
                direction,
            };
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::PaneInfo { session_id, pane_id, pid: _ }) => {
                    // Build the attach command for the new WT pane
                    let exe_path = std::env::current_exe()
                        .unwrap_or_else(|_| std::path::PathBuf::from("wmux.exe"));
                    let attach_cmd = format!(
                        "\"{}\" attach {} --pane {}",
                        exe_path.display(),
                        session_id,
                        pane_id
                    );

                    // Create the WT split pane running the attach command
                    if let Err(e) = wmux::wt::wt_split_pane(direction_str, &attach_cmd) {
                        eprintln!("Error creating WT split pane: {}", e);
                        std::process::exit(1);
                    }

                    println!("Split pane {} created", pane_id);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("Unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to split pane: {}. Is the daemon running?", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::KillPane { pane_id }) => {
            wmux::wt::require_windows_terminal()?;

            // Get session ID from env var set during attach
            let session_id = match std::env::var("WMUX_SESSION_ID") {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Error: not attached to a session — run wmux kill-pane from within an attached session");
                    std::process::exit(1);
                }
            };

            // If no pane_id specified, try to get the current pane from env var
            let target_pane = pane_id.unwrap_or_else(|| {
                std::env::var("WMUX_PANE_ID")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0)
            });

            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::KillPane {
                session_id,
                pane_id: target_pane,
            };
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::Ok { message }) => {
                    println!("{}", message);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("Unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to kill pane: {}. Is the daemon running?", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            // No subcommand — print help
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
