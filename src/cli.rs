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
    Attach {
        /// Session ID to attach to (attaches to most recent if omitted)
        session_id: Option<String>,
    },

    /// Detach from the current session
    Detach,

    /// Kill a session
    #[command(name = "kill-session")]
    KillSession {
        /// Session ID to kill
        id: String,
    },

    /// Kill a pane
    #[command(name = "kill-pane")]
    KillPane {
        /// Pane ID to kill (kills active pane if omitted)
        #[arg(long)]
        pane_id: Option<u32>,
    },

    /// Split current pane
    Split {
        /// Split horizontally
        #[arg(short = 'h', long = "horizontal")]
        horizontal: bool,

        /// Split vertically
        #[arg(short = 'v', long = "vertical")]
        vertical: bool,
    },
}
