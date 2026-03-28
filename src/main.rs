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
        Some(Commands::Attach)
        | Some(Commands::Detach)
        | Some(Commands::KillPane)
        | Some(Commands::Split) => {
            eprintln!("Not yet implemented");
            std::process::exit(1);
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
