use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wmux",
    version,
    about = "Terminal multiplexer for Windows — keep sessions alive across terminal closes",
    long_about = "Terminal multiplexer for Windows — keep sessions alive across terminal closes\n\n\
        wmux runs a background daemon that keeps your shell sessions alive. You can detach\n\
        from a session, close your terminal, and reattach later without losing any running\n\
        processes.\n\n\
        Start with: wmux daemon-start && wmux new",
    after_help = "Run 'wmux <command> --help' for more information on a specific command."
)]
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
    #[command(
        name = "daemon-start",
        long_about = "Start the wmux daemon process\n\n\
            Starts the background daemon that manages all sessions. The daemon runs as a\n\
            detached process and persists after this terminal is closed. Only one daemon\n\
            can run at a time."
    )]
    DaemonStart,

    /// Show daemon and session status
    Status,

    /// Stop the daemon and all sessions
    #[command(
        name = "kill-server",
        long_about = "Stop the daemon and all sessions\n\n\
            Stops the wmux daemon process. All active sessions will be terminated."
    )]
    KillServer,

    /// Create a new session
    #[command(long_about = "Create a new session\n\n\
        Creates a new terminal session managed by the daemon. The session runs in the\n\
        background and can be attached to with 'wmux attach'.")]
    New,

    /// List active sessions
    Ls,

    /// Attach to a session
    #[command(long_about = "Attach to a session\n\n\
        Connects your terminal to an existing session. If no session ID is given,\n\
        attaches to the most recently created session.\n\n\
        Detach with Ctrl+B, d.")]
    Attach {
        /// Session ID (from 'wmux ls'). Defaults to most recent.
        session_id: Option<String>,

        /// Target a specific pane (used internally by split to attach new WT pane to daemon pane)
        #[arg(long, hide = true)]
        pane: Option<u32>,
    },

    /// Detach from the current session
    #[command(long_about = "Detach from the current session\n\n\
        Note: Detach is performed via the Ctrl+B, d keybinding while attached to a\n\
        session. This command is not used directly.")]
    Detach,

    /// Kill a session and all its panes
    #[command(name = "kill-session")]
    KillSession {
        /// Session ID to kill (from 'wmux ls')
        id: String,
    },

    /// Kill a pane in the current session
    #[command(name = "kill-pane")]
    KillPane {
        /// Pane ID to kill. Defaults to the active pane.
        #[arg(long)]
        pane_id: Option<u32>,
    },

    /// Split the current pane
    #[command(long_about = "Split the current pane\n\n\
        Creates a new pane by splitting the current one. Requires Windows Terminal.\n\n\
        Also available via keybinding: Ctrl+B, \" (horizontal) or Ctrl+B, % (vertical).")]
    Split {
        /// Split horizontally (top/bottom)
        #[arg(short = 'H', long = "horizontal")]
        horizontal: bool,

        /// Split vertically (left/right)
        #[arg(short = 'v', long = "vertical")]
        vertical: bool,
    },
}
