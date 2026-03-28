mod cli;

use clap::Parser;
use cli::{Cli, Commands};

use wmux::daemon;
use wmux::paths;

/// Print an error message to stderr and exit with the given code.
fn exit_error(message: &str, hint: Option<&str>, code: i32) -> ! {
    eprintln!("error: {}", message);
    if let Some(h) = hint {
        eprintln!("hint: {}", h);
    }
    std::process::exit(code);
}

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
            if let Err(e) = daemon::lifecycle::start_daemon().await {
                exit_error(
                    &format!("failed to start daemon: {}", e),
                    Some("Check if another daemon is already running with 'wmux status'"),
                    1,
                );
            }
        }
        Some(Commands::Status) => {
            if let Err(e) = daemon::lifecycle::daemon_status().await {
                exit_error(
                    &format!("failed to get status: {}", e),
                    Some("Run 'wmux daemon-start' to start the daemon"),
                    1,
                );
            }
        }
        Some(Commands::KillServer) => {
            if let Err(e) = daemon::lifecycle::kill_server().await {
                exit_error(
                    &format!("failed to stop daemon: {}", e),
                    Some("Run 'wmux daemon-start' to start the daemon"),
                    1,
                );
            }
        }
        Some(Commands::New) => {
            // Auto-start daemon if not running (tmux behavior)
            if let Err(e) = wmux::ipc::client::ensure_daemon_running().await {
                exit_error(
                    &format!("failed to start daemon: {}", e),
                    Some("Try starting manually with 'wmux daemon-start'"),
                    1,
                );
            }
            let pipe_name = paths::control_pipe();
            let request = wmux::ipc::protocol::Request::NewSession { name: None };
            match wmux::ipc::client::send_request(&pipe_name, &request).await {
                Ok(wmux::ipc::protocol::Response::Ok { message }) => {
                    println!("{}", message);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    exit_error(&message, Some("Run 'wmux ls' to see active sessions"), 1);
                }
                Ok(other) => {
                    exit_error(
                        &format!("unexpected response: {:?}", other),
                        None,
                        1,
                    );
                }
                Err(e) => {
                    exit_error(
                        &format!("failed to create session: {}", e),
                        Some("Run 'wmux daemon-start' to start the daemon"),
                        1,
                    );
                }
            }
        }
        Some(Commands::Ls) => {
            // Auto-start daemon if not running
            if let Err(e) = wmux::ipc::client::ensure_daemon_running().await {
                exit_error(
                    &format!("failed to start daemon: {}", e),
                    Some("Try starting manually with 'wmux daemon-start'"),
                    1,
                );
            }
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
                    exit_error(
                        &format!("unexpected response: {:?}", other),
                        None,
                        1,
                    );
                }
                Err(e) => {
                    exit_error(
                        &format!("failed to list sessions: {}", e),
                        Some("Run 'wmux daemon-start' to start the daemon"),
                        1,
                    );
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
                    exit_error(&message, Some("Run 'wmux ls' to see active sessions"), 1);
                }
                Ok(other) => {
                    exit_error(
                        &format!("unexpected response: {:?}", other),
                        None,
                        1,
                    );
                }
                Err(e) => {
                    exit_error(
                        &format!("failed to kill session: {}", e),
                        Some("Run 'wmux daemon-start' to start the daemon"),
                        1,
                    );
                }
            }
        }
        Some(Commands::Attach { session_id, pane }) => {
            // Auto-start daemon if not running (tmux behavior)
            if let Err(e) = wmux::ipc::client::ensure_daemon_running().await {
                exit_error(
                    &format!("failed to start daemon: {}", e),
                    Some("Try starting manually with 'wmux daemon-start'"),
                    1,
                );
            }
            if let Err(e) = wmux::wt::require_windows_terminal() {
                exit_error(
                    &format!("Windows Terminal is required: {}", e),
                    Some("Install Windows Terminal from the Microsoft Store, or run wmux from within Windows Terminal"),
                    1,
                );
            }
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
                            exit_error(
                                "no active sessions to attach to",
                                Some("Run 'wmux new' to create a session"),
                                1,
                            );
                        }
                        // Pick the last one (most recently created)
                        sessions.last().unwrap().id.clone()
                    }
                    Ok(other) => {
                        exit_error(
                            &format!("unexpected response: {:?}", other),
                            None,
                            1,
                        );
                    }
                    Err(e) => {
                        exit_error(
                            &format!("failed to list sessions: {}", e),
                            Some("Run 'wmux daemon-start' to start the daemon"),
                            1,
                        );
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
                    exit_error(
                        &format!("attach failed: {}", e),
                        Some("Run 'wmux ls' to see active sessions"),
                        1,
                    );
                }
            }
        }
        Some(Commands::Detach) => {
            exit_error(
                "detach is performed via the Ctrl+B, d keybinding while attached to a session",
                Some("Attach to a session first with 'wmux attach', then press Ctrl+B, d to detach"),
                2,
            );
        }
        Some(Commands::Split { horizontal, vertical }) => {
            if let Err(e) = wmux::wt::require_windows_terminal() {
                exit_error(
                    &format!("Windows Terminal is required: {}", e),
                    Some("Install Windows Terminal from the Microsoft Store, or run wmux from within Windows Terminal"),
                    1,
                );
            }
            if !horizontal && !vertical {
                exit_error(
                    "no split direction specified",
                    Some("Use -H for horizontal or -v for vertical"),
                    2,
                );
            }

            // Get session ID from env var set during attach
            let session_id = match std::env::var("WMUX_SESSION_ID") {
                Ok(id) => id,
                Err(_) => {
                    exit_error(
                        "not attached to a session",
                        Some("This command must be run from within an attached session"),
                        1,
                    );
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
                        exit_error(
                            &format!("failed to create WT split pane: {}", e),
                            Some("Ensure Windows Terminal is running and supports split-pane"),
                            1,
                        );
                    }

                    println!("Split pane {} created", pane_id);
                }
                Ok(wmux::ipc::protocol::Response::Error { message }) => {
                    exit_error(&message, Some("Run 'wmux ls' to see active sessions"), 1);
                }
                Ok(other) => {
                    exit_error(
                        &format!("unexpected response: {:?}", other),
                        None,
                        1,
                    );
                }
                Err(e) => {
                    exit_error(
                        &format!("failed to split pane: {}", e),
                        Some("Run 'wmux daemon-start' to start the daemon"),
                        1,
                    );
                }
            }
        }
        Some(Commands::KillPane { pane_id }) => {
            if let Err(e) = wmux::wt::require_windows_terminal() {
                exit_error(
                    &format!("Windows Terminal is required: {}", e),
                    Some("Install Windows Terminal from the Microsoft Store, or run wmux from within Windows Terminal"),
                    1,
                );
            }

            // Get session ID from env var set during attach
            let session_id = match std::env::var("WMUX_SESSION_ID") {
                Ok(id) => id,
                Err(_) => {
                    exit_error(
                        "not attached to a session",
                        Some("This command must be run from within an attached session"),
                        1,
                    );
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
                    exit_error(&message, Some("Run 'wmux ls' to see active sessions"), 1);
                }
                Ok(other) => {
                    exit_error(
                        &format!("unexpected response: {:?}", other),
                        None,
                        1,
                    );
                }
                Err(e) => {
                    exit_error(
                        &format!("failed to kill pane: {}", e),
                        Some("Run 'wmux daemon-start' to start the daemon"),
                        1,
                    );
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
