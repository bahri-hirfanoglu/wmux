use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use tracing::info;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

use crate::paths;

/// Check if a process with the given PID is alive.
fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let result = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        match result {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(_) => false,
        }
    }
}

/// Read the PID from the PID file. Returns None if file doesn't exist or is invalid.
fn read_pid_file() -> Result<Option<u32>> {
    let pid_path = paths::pid_file()?;
    if !pid_path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&pid_path)
        .with_context(|| format!("Failed to read PID file: {}", pid_path.display()))?;
    let pid: u32 = contents
        .trim()
        .parse()
        .with_context(|| format!("Invalid PID in file: {}", contents.trim()))?;
    Ok(Some(pid))
}

/// Write the current process PID to the PID file.
fn write_pid_file(pid: u32) -> Result<()> {
    let pid_path = paths::pid_file()?;
    let mut file = fs::File::create(&pid_path)
        .with_context(|| format!("Failed to create PID file: {}", pid_path.display()))?;
    write!(file, "{}", pid)?;
    Ok(())
}

/// Remove the PID file if it exists.
fn remove_pid_file() -> Result<()> {
    let pid_path = paths::pid_file()?;
    if pid_path.exists() {
        fs::remove_file(&pid_path)
            .with_context(|| format!("Failed to remove PID file: {}", pid_path.display()))?;
    }
    Ok(())
}

/// Start the daemon as a detached background process.
///
/// If the daemon is already running, prints a message and exits.
/// Otherwise, spawns a new detached process with --daemon-mode.
pub async fn start_daemon() -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid_file()? {
        if is_process_alive(pid) {
            println!("Daemon already running (pid: {})", pid);
            return Ok(());
        }
        // Stale PID file — clean it up
        remove_pid_file()?;
    }

    // Ensure data directory exists
    paths::wmux_data_dir()?;

    // Get the path to the current executable
    let exe_path =
        std::env::current_exe().context("Failed to determine current executable path")?;

    // Spawn detached process with --daemon-mode flag
    // DETACHED_PROCESS (0x00000008) | CREATE_NO_WINDOW (0x08000000)
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let child = std::process::Command::new(&exe_path)
        .arg("--daemon-mode")
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .with_context(|| format!("Failed to spawn daemon process: {}", exe_path.display()))?;

    let pid = child.id();
    println!("Daemon started (pid: {})", pid);

    Ok(())
}

/// Run the daemon main loop.
///
/// This is the entry point when the binary is invoked with --daemon-mode.
/// Writes PID file, initializes logging, starts the control server, and
/// waits for shutdown.
pub async fn run_daemon() -> Result<()> {
    // Write PID file
    let pid = unsafe { GetCurrentProcessId() };
    write_pid_file(pid)?;

    // Initialize tracing to log file
    let log_path = paths::log_file()?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("Failed to open log file: {}", log_path.display()))?;

    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_ansi(false)
        .with_target(false)
        .init();

    info!("wmux daemon started (pid: {})", pid);

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Start the control server
    let pipe_name = paths::control_pipe();
    let server_handle = tokio::spawn(async move {
        if let Err(e) =
            wmux::ipc::server::ControlServer::start(&pipe_name, shutdown_rx, shutdown_tx).await
        {
            tracing::error!("Control server error: {}", e);
        }
    });

    // Also listen for ctrl_c as a backup shutdown mechanism
    let shutdown_tx_ctrlc = {
        let (tx, _) = tokio::sync::watch::channel(false);
        tx
    };
    // Note: ctrl_c won't fire for DETACHED_PROCESS, but useful during development
    tokio::select! {
        _ = server_handle => {
            info!("Control server exited");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("wmux daemon received ctrl_c signal");
            let _ = shutdown_tx_ctrlc;
        }
    }

    // Clean shutdown
    info!("wmux daemon shutting down");
    remove_pid_file()?;

    Ok(())
}

/// Print the daemon status using IPC if available, falling back to PID file.
pub async fn daemon_status() -> Result<()> {
    let pipe_name = paths::control_pipe();

    // Try IPC first
    match wmux::ipc::client::send_request(&pipe_name, &wmux::ipc::protocol::Request::Status).await
    {
        Ok(wmux::ipc::protocol::Response::Status {
            running,
            pid,
            session_count,
        }) => {
            println!(
                "Daemon is running (pid: {}, sessions: {})",
                pid, session_count
            );
            let _ = running;
        }
        Ok(other) => {
            println!("Unexpected response from daemon: {:?}", other);
        }
        Err(_) => {
            // Fall back to PID file check
            match read_pid_file()? {
                Some(pid) => {
                    if is_process_alive(pid) {
                        println!(
                            "Daemon is running (pid: {}, pipe not responding)",
                            pid
                        );
                    } else {
                        remove_pid_file()?;
                        println!("Daemon is not running (cleaned stale PID file)");
                    }
                }
                None => {
                    println!("Daemon is not running");
                }
            }
        }
    }

    Ok(())
}

/// Stop the daemon process using IPC if available, falling back to process kill.
pub async fn kill_server() -> Result<()> {
    let pipe_name = paths::control_pipe();

    // Try IPC first
    match wmux::ipc::client::send_request(&pipe_name, &wmux::ipc::protocol::Request::KillServer)
        .await
    {
        Ok(wmux::ipc::protocol::Response::Ok { message }) => {
            println!("Daemon stopped ({})", message);
            // Give daemon a moment to clean up, then verify
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            // Clean up PID file if daemon didn't get to it
            let _ = remove_pid_file();
        }
        Ok(other) => {
            println!("Unexpected response: {:?}", other);
        }
        Err(_) => {
            // Fall back to process kill via PID
            match read_pid_file()? {
                Some(pid) => {
                    if is_process_alive(pid) {
                        unsafe {
                            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                                .context("Failed to open daemon process for termination")?;
                            TerminateProcess(handle, 1)
                                .context("Failed to terminate daemon process")?;
                            let _ = CloseHandle(handle);
                        }
                    }
                    remove_pid_file()?;
                    println!("Daemon stopped (killed via PID)");
                }
                None => {
                    println!("Daemon is not running");
                }
            }
        }
    }

    Ok(())
}
