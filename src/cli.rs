use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wmux", version, about = "Terminal multiplexer for Windows")]
pub struct Cli {
    /// Internal flag: run as daemon process (not for user use)
    #[arg(long, hide = true)]
    pub daemon_mode: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the wmux daemon process
    #[command(name = "daemon-start")]
    DaemonStart,

    /// Show daemon and session status
    Status,

    /// Stop the wmux daemon
    #[command(name = "kill-server")]
    KillServer,

    /// Create a new session
    New,

    /// List active sessions
    Ls,

    /// Attach to a session
    Attach,

    /// Detach from the current session
    Detach,

    /// Kill a session
    #[command(name = "kill-session")]
    KillSession,

    /// Kill a pane
    #[command(name = "kill-pane")]
    KillPane,

    /// Split current pane
    Split,
}
