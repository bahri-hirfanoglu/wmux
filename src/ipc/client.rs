use anyhow::{bail, Context, Result};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::{sleep, Duration};
use tracing::info;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE,
    ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

use super::protocol::{read_message, write_message, Request, Response};

/// Send a request to the daemon via Named Pipe and return the response.
///
/// Connects to \\.\pipe\wmux-ctl, writes the request, reads the response.
pub async fn send_request(pipe_name: &str, request: &Request) -> Result<Response> {
    // Named pipes on Windows may need a retry if the server hasn't called
    // ConnectNamedPipe yet for the next instance.
    let mut pipe = None;
    for attempt in 0..5 {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => {
                pipe = Some(client);
                break;
            }
            Err(e) if attempt < 4 => {
                // Pipe might be busy or not yet ready — brief retry
                let _ = e;
                sleep(Duration::from_millis(50)).await;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to connect to daemon pipe '{}': {}. Is the daemon running?",
                    pipe_name,
                    e
                ));
            }
        }
    }

    let mut pipe = pipe.unwrap();
    let (mut reader, mut writer) = tokio::io::split(&mut pipe);

    write_message(&mut writer, request)
        .await
        .context("Failed to send request to daemon")?;

    let response: Response = read_message(&mut reader)
        .await
        .context("Failed to read response from daemon")?;

    Ok(response)
}

/// Guard that restores the original console mode when dropped.
struct ConsoleRawModeGuard {
    stdin_handle: HANDLE,
    original_mode: CONSOLE_MODE,
}

impl Drop for ConsoleRawModeGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetConsoleMode(self.stdin_handle, self.original_mode);
        }
    }
}

/// Attach to a session on the daemon, streaming I/O bidirectionally.
///
/// Puts the local terminal into raw mode, forwards stdin to the daemon
/// and daemon output to stdout. Detach via Ctrl+B then 'd'.
pub async fn attach_session(pipe_name: &str, session_id: &str) -> Result<()> {
    // 1. Connect to the daemon's Named Pipe
    let mut pipe = None;
    for attempt in 0..5 {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => {
                pipe = Some(client);
                break;
            }
            Err(e) if attempt < 4 => {
                let _ = e;
                sleep(Duration::from_millis(50)).await;
            }
            Err(e) => {
                bail!(
                    "Failed to connect to daemon pipe '{}': {}. Is the daemon running?",
                    pipe_name,
                    e
                );
            }
        }
    }

    let mut pipe = pipe.unwrap();
    let (mut reader, mut writer) = tokio::io::split(&mut pipe);

    // 2. Send AttachSession request
    let req = Request::AttachSession {
        session_id: session_id.to_string(),
    };
    write_message(&mut writer, &req)
        .await
        .context("Failed to send attach request")?;

    // 3. Read confirmation
    let response: Response = read_message(&mut reader)
        .await
        .context("Failed to read attach response")?;

    match &response {
        Response::AttachStarted {
            session_id: sid,
            pane_count,
        } => {
            info!(
                "Attached to session {} ({} pane(s))",
                sid, pane_count
            );
        }
        Response::Error { message } => {
            bail!("Attach failed: {}", message);
        }
        other => {
            bail!("Unexpected response to attach: {:?}", other);
        }
    }

    // 4. Put the local terminal into raw mode
    let _raw_guard = enter_raw_mode()?;

    // 5. Get stdout handle for writing output
    let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE)? };

    // Enable virtual terminal processing on stdout for ANSI escape sequences
    unsafe {
        let mut stdout_mode = CONSOLE_MODE::default();
        let _ = GetConsoleMode(stdout_handle, &mut stdout_mode);
        let _ = SetConsoleMode(
            stdout_handle,
            stdout_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        );
    }

    // Get stdin handle for reading input
    let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE)? };
    let stdin_raw = stdin_handle.0 as isize;

    // 6. Bidirectional streaming loop with prefix key detection
    let mut prefix_active = false;

    loop {
        // Spawn blocking stdin read
        let stdin_r = stdin_raw;
        let stdin_task = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let handle = HANDLE(stdin_r as *mut _);
            let mut buf = vec![0u8; 256];
            let mut bytes_read: u32 = 0;
            unsafe {
                ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None)
                    .context("ReadFile from stdin failed")?;
            }
            buf.truncate(bytes_read as usize);
            Ok(buf)
        });

        // Read output from daemon
        let output_task = read_message::<_, Response>(&mut reader);

        tokio::select! {
            // User typed something on stdin
            stdin_result = stdin_task => {
                match stdin_result {
                    Ok(Ok(data)) if !data.is_empty() => {
                        // Process each byte for prefix key detection
                        let mut to_send: Vec<u8> = Vec::new();
                        let mut should_detach = false;

                        for &byte in &data {
                            if prefix_active {
                                prefix_active = false;
                                if byte == b'd' {
                                    // Prefix + d = detach
                                    should_detach = true;
                                    break;
                                } else {
                                    // Not a recognized prefix command — forward both
                                    to_send.push(0x02); // Ctrl+B
                                    to_send.push(byte);
                                }
                            } else if byte == 0x02 {
                                // Ctrl+B pressed — activate prefix mode
                                prefix_active = true;
                            } else {
                                to_send.push(byte);
                            }
                        }

                        if should_detach {
                            // Send detach request to daemon
                            let detach_req = Request::DetachSession {
                                session_id: session_id.to_string(),
                            };
                            let _ = write_message(&mut writer, &detach_req).await;
                            break;
                        }

                        if !to_send.is_empty() {
                            let input_req = Request::SessionInput { data: to_send };
                            if let Err(e) = write_message(&mut writer, &input_req).await {
                                info!("Failed to send input to daemon: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(Ok(_)) => {
                        // Empty read — stdin closed
                        break;
                    }
                    Ok(Err(e)) => {
                        info!("stdin read error: {}", e);
                        break;
                    }
                    Err(e) => {
                        info!("stdin task panicked: {}", e);
                        break;
                    }
                }
            }

            // Daemon sent output
            output_result = output_task => {
                match output_result {
                    Ok(Response::SessionOutput { data }) => {
                        // Write to local stdout
                        let stdout_raw = stdout_handle.0 as isize;
                        let write_result = tokio::task::spawn_blocking(move || -> Result<()> {
                            let handle = HANDLE(stdout_raw as *mut _);
                            let mut written: u32 = 0;
                            unsafe {
                                WriteFile(handle, Some(&data), Some(&mut written), None)
                                    .context("WriteFile to stdout failed")?;
                            }
                            Ok(())
                        }).await;

                        match write_result {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => {
                                info!("stdout write error: {}", e);
                                break;
                            }
                            Err(e) => {
                                info!("stdout task panicked: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(Response::Ok { message }) => {
                        // Detach confirmation or session ended
                        info!("Daemon message: {}", message);
                        break;
                    }
                    Ok(Response::Error { message }) => {
                        eprintln!("Error from daemon: {}", message);
                        break;
                    }
                    Ok(_other) => {
                        // Ignore unexpected responses during streaming
                    }
                    Err(e) => {
                        // Daemon disconnected
                        info!("Daemon connection lost: {}", e);
                        break;
                    }
                }
            }
        }
    }

    // ConsoleRawModeGuard restores console mode on drop
    drop(_raw_guard);

    Ok(())
}

/// Enter raw console mode, returning a guard that restores the original mode on drop.
fn enter_raw_mode() -> Result<ConsoleRawModeGuard> {
    unsafe {
        let stdin_handle = GetStdHandle(STD_INPUT_HANDLE)?;
        let mut original_mode = CONSOLE_MODE::default();
        GetConsoleMode(stdin_handle, &mut original_mode)
            .context("Failed to get console mode")?;

        // Disable line input, echo, and processed input (Ctrl+C handling)
        // This puts the console in raw mode where every keystroke is sent immediately.
        let raw_mode = original_mode
            & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
        SetConsoleMode(stdin_handle, raw_mode)
            .context("Failed to set raw console mode")?;

        Ok(ConsoleRawModeGuard {
            stdin_handle,
            original_mode,
        })
    }
}
