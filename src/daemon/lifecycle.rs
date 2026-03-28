use anyhow::Result;

/// Start the daemon as a background process.
pub async fn start_daemon() -> Result<()> {
    eprintln!("Not yet implemented: daemon start");
    std::process::exit(1);
}

/// Run the daemon main loop (called internally via --daemon-mode).
pub async fn run_daemon() -> Result<()> {
    eprintln!("Not yet implemented: daemon run");
    std::process::exit(1);
}

/// Print daemon status.
pub fn daemon_status() -> Result<()> {
    eprintln!("Not yet implemented: daemon status");
    std::process::exit(1);
}

/// Stop the daemon.
pub fn kill_server() -> Result<()> {
    eprintln!("Not yet implemented: kill server");
    std::process::exit(1);
}
