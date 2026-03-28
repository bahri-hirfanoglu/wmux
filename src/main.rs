mod cli;
mod daemon;
mod paths;

use clap::Parser;
use cli::{Cli, Commands};

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
            daemon::lifecycle::daemon_status()?;
        }
        Some(Commands::KillServer) => {
            daemon::lifecycle::kill_server()?;
        }
        Some(Commands::New)
        | Some(Commands::Ls)
        | Some(Commands::Attach)
        | Some(Commands::Detach)
        | Some(Commands::KillSession)
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
