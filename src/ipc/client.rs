use anyhow::{bail, Context, Result};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::{sleep, Duration};
use tracing::info;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE,
    ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    ENABLE_WINDOW_INPUT, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

use super::protocol::{read_message, write_message, NavDirection, Request, Response, SplitDirection};

/// Result of handling a prefix key sequence.
#[allow(dead_code)]
enum PrefixAction {
    /// Ctrl+B then 'd' — detach from session.
    Detach,
    /// Key was handled (navigation, resize, split, kill). No bytes to forward.
    Handled,
    /// Not a recognized prefix command — forward these bytes to the shell.
    Forward(Vec<u8>),
}

/// Default resize amount in cells for Prefix + Alt+arrow.
const RESIZE_AMOUNT: u32 = 5;

/// Default number of lines visible in scroll mode viewport.
#[allow(dead_code)]
const SCROLL_PAGE_SIZE: usize = 50;

/// Ensure the daemon is running, starting it if needed.
/// Returns Ok(()) if daemon is reachable after this call.
pub async fn ensure_daemon_running() -> Result<()> {
    let pipe_name = crate::paths::control_pipe();

    // Try to connect to existing daemon — send a quick Ping to verify
    if let Ok(resp) = send_request(&pipe_name, &Request::Ping).await {
        if matches!(resp, Response::Pong) {
            return Ok(());
        }
    }

    // Daemon not running — auto-start it
    eprintln!("Starting daemon...");
    crate::daemon::lifecycle::start_daemon().await?;

    // Wait for daemon to be ready (up to 3 seconds)
    for _ in 0..30 {
        sleep(Duration::from_millis(100)).await;
        if ClientOptions::new().open(&pipe_name).is_ok() {
            return Ok(());
        }
    }

    Err(anyhow::anyhow!(
        "Daemon started but not responding. Check logs at {:?}",
        crate::paths::log_file().unwrap_or_default()
    ))
}

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

    // 4. Set console codepage to UTF-8 so ConPTY output renders correctly
    let _cp_guard = set_utf8_codepage();

    // 5. Put the local terminal into raw mode
    let _raw_guard = enter_raw_mode()?;

    // 6. Get stdout handle for writing output
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

    // 7. Spawn a dedicated stdin reader thread with a channel.
    // This avoids spawn_blocking overhead on every keystroke.
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let stdin_reader = std::thread::spawn(move || {
        let handle = HANDLE(stdin_raw as *mut _);
        loop {
            let mut buf = vec![0u8; 256];
            let mut bytes_read: u32 = 0;
            let result = unsafe {
                ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None)
            };
            match result {
                Ok(()) if bytes_read > 0 => {
                    buf.truncate(bytes_read as usize);
                    if stdin_tx.blocking_send(buf).is_err() {
                        break; // receiver dropped — client exiting
                    }
                }
                _ => break, // stdin closed or error
            }
        }
    });

    // 8. Bidirectional streaming loop with prefix key detection
    let mut prefix_active = false;
    let mut pending_split_direction: Option<String> = None;

    loop {

        // Read output from daemon
        let output_task = read_message::<_, Response>(&mut reader);

        tokio::select! {
            // User typed something on stdin (from dedicated reader thread)
            stdin_result = stdin_rx.recv() => {
                match stdin_result {
                    Some(data) if !data.is_empty() => {
                        let mut to_send: Vec<u8> = Vec::new();
                        let mut should_detach = false;
                        let mut i = 0;

                        while i < data.len() {
                            let byte = data[i];

                            if prefix_active {
                                prefix_active = false;

                                // Check for escape sequences (arrow keys, Alt+arrow)
                                if byte == 0x1B && i + 2 < data.len() && data[i + 1] == b'[' {
                                    // Could be arrow key (ESC [ A/B/C/D) or Alt+arrow (ESC [ 1 ; 3 A/B/C/D)
                                    if i + 4 < data.len() && data[i + 2] == b'1' && data[i + 3] == b';' && data[i + 4] == b'3' && i + 5 < data.len() {
                                        // Alt+arrow: ESC [ 1 ; 3 {letter} — resize pane
                                        let action = handle_prefix_alt_arrow(data[i + 5], session_id, &mut writer).await;
                                        i += 6;
                                        match action {
                                            PrefixAction::Forward(bytes) => to_send.extend_from_slice(&bytes),
                                            PrefixAction::Handled => {}
                                            PrefixAction::Detach => { should_detach = true; break; }
                                        }
                                        continue;
                                    } else if matches!(data[i + 2], b'A' | b'B' | b'C' | b'D') {
                                        // Arrow key: ESC [ {letter} — navigate pane
                                        let action = handle_prefix_arrow(data[i + 2], session_id, &mut writer).await;
                                        i += 3;
                                        match action {
                                            PrefixAction::Forward(bytes) => to_send.extend_from_slice(&bytes),
                                            PrefixAction::Handled => {}
                                            PrefixAction::Detach => { should_detach = true; break; }
                                        }
                                        continue;
                                    }
                                }

                                // Single-byte prefix commands
                                match byte {
                                    b'd' => {
                                        // Prefix + d = detach
                                        should_detach = true;
                                        break;
                                    }
                                    b'"' => {
                                        // Prefix + " = horizontal split
                                        pending_split_direction = Some("horizontal".to_string());
                                        handle_prefix_split(SplitDirection::Horizontal, session_id, &mut writer).await;
                                    }
                                    b'%' => {
                                        // Prefix + % = vertical split
                                        pending_split_direction = Some("vertical".to_string());
                                        handle_prefix_split(SplitDirection::Vertical, session_id, &mut writer).await;
                                    }
                                    b'x' => {
                                        // Prefix + x = kill pane (with confirmation)
                                        handle_prefix_kill_pane(session_id, &mut writer, stdin_raw, stdout_handle).await;
                                    }
                                    b'[' => {
                                        // Prefix + [ = enter scroll mode
                                        enter_scroll_mode(
                                            session_id,
                                            &mut reader,
                                            &mut writer,
                                            stdin_raw,
                                            stdout_handle,
                                        ).await;
                                    }
                                    _ => {
                                        // Not a recognized prefix command — forward both bytes
                                        to_send.push(0x02); // Ctrl+B
                                        to_send.push(byte);
                                    }
                                }
                            } else if byte == 0x02 {
                                // Ctrl+B pressed — activate prefix mode
                                prefix_active = true;
                            } else {
                                to_send.push(byte);
                            }

                            i += 1;
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
                    Some(_) => {
                        // Empty data — shouldn't happen with our reader
                        break;
                    }
                    None => {
                        // Channel closed — stdin reader thread exited
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
                    Ok(Response::PaneInfo { session_id: sid, pane_id, pid: _ }) => {
                        // Response to a prefix-key split request — invoke wt.exe split-pane
                        let exe_path = std::env::current_exe()
                            .unwrap_or_else(|_| std::path::PathBuf::from("wmux.exe"));
                        let attach_cmd = format!(
                            "\"{}\" attach {} --pane {}",
                            exe_path.display(),
                            sid,
                            pane_id
                        );
                        let dir = pending_split_direction.take().unwrap_or_else(|| "vertical".to_string());
                        if let Err(e) = crate::wt::wt_split_pane(&dir, &attach_cmd) {
                            info!("Failed to create WT split pane: {}", e);
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

    // Clean up: drop stdin channel to signal reader thread to exit,
    // then restore console mode and codepage.
    drop(stdin_rx);
    let _ = stdin_reader.join();
    drop(_raw_guard);

    Ok(())
}

/// Map an ANSI arrow letter to a direction string for wt.exe commands.
fn arrow_letter_to_direction(letter: u8) -> Option<&'static str> {
    match letter {
        b'A' => Some("up"),
        b'B' => Some("down"),
        b'C' => Some("right"),
        b'D' => Some("left"),
        _ => None,
    }
}

/// Map an ANSI arrow letter to NavDirection for the IPC protocol.
fn arrow_letter_to_nav(letter: u8) -> Option<NavDirection> {
    match letter {
        b'A' => Some(NavDirection::Up),
        b'B' => Some(NavDirection::Down),
        b'C' => Some(NavDirection::Right),
        b'D' => Some(NavDirection::Left),
        _ => None,
    }
}

/// Handle Prefix + arrow key: navigate to adjacent pane.
async fn handle_prefix_arrow<W: tokio::io::AsyncWrite + Unpin>(
    letter: u8,
    session_id: &str,
    writer: &mut W,
) -> PrefixAction {
    if let (Some(dir_str), Some(nav_dir)) = (arrow_letter_to_direction(letter), arrow_letter_to_nav(letter)) {
        // Tell WT to move focus visually
        if let Err(e) = crate::wt::wt_move_focus(dir_str) {
            info!("wt_move_focus failed: {}", e);
        }
        // Notify daemon to update its active pane tracking
        let req = Request::NavigatePane {
            session_id: session_id.to_string(),
            direction: nav_dir,
        };
        let _ = write_message(writer, &req).await;
        PrefixAction::Handled
    } else {
        // Unknown escape sequence after prefix — forward all bytes
        let bytes = vec![0x02, 0x1B, b'[', letter];
        PrefixAction::Forward(bytes)
    }
}

/// Handle Prefix + Alt+arrow: resize active pane.
async fn handle_prefix_alt_arrow<W: tokio::io::AsyncWrite + Unpin>(
    letter: u8,
    session_id: &str,
    writer: &mut W,
) -> PrefixAction {
    if let Some(dir_str) = arrow_letter_to_direction(letter) {
        // Tell WT to resize visually
        if let Err(e) = crate::wt::wt_resize_pane(dir_str, RESIZE_AMOUNT) {
            info!("wt_resize_pane failed: {}", e);
        }
        // Notify daemon — compute approximate new size
        // The daemon will need to ResizePseudoConsole for the active pane.
        // We send a ResizePane with delta-adjusted dimensions (best-effort).
        let req = Request::NavigatePane {
            session_id: session_id.to_string(),
            direction: arrow_letter_to_nav(letter).unwrap_or(NavDirection::Right),
        };
        // Note: actual ConPTY resize happens when WT sends SIGWINCH equivalent.
        // We just update daemon tracking here.
        let _ = write_message(writer, &req).await;
        PrefixAction::Handled
    } else {
        let bytes = vec![0x02, 0x1B, b'[', b'1', b';', b'3', letter];
        PrefixAction::Forward(bytes)
    }
}

/// Handle Prefix + " or Prefix + %: split pane.
async fn handle_prefix_split<W: tokio::io::AsyncWrite + Unpin>(
    direction: SplitDirection,
    session_id: &str,
    writer: &mut W,
) {
    let req = Request::SplitPane {
        session_id: session_id.to_string(),
        direction: direction.clone(),
    };
    match write_message(writer, &req).await {
        Ok(()) => {
            info!("Split pane request sent via prefix key");
        }
        Err(e) => {
            info!("Failed to send split pane request: {}", e);
        }
    }
}

/// Handle Prefix + x: kill pane with y/n confirmation.
async fn handle_prefix_kill_pane<W: tokio::io::AsyncWrite + Unpin>(
    session_id: &str,
    writer: &mut W,
    stdin_raw: isize,
    stdout_handle: HANDLE,
) {
    // Write confirmation prompt to stdout
    let prompt = b"\r\nkill-pane? (y/n) ";
    let stdout_raw = stdout_handle.0 as isize;
    let _ = tokio::task::spawn_blocking(move || {
        let handle = HANDLE(stdout_raw as *mut _);
        let mut written: u32 = 0;
        unsafe {
            let _ = WriteFile(handle, Some(prompt), Some(&mut written), None);
        }
    }).await;

    // Read single byte for confirmation
    let confirm_result = tokio::task::spawn_blocking(move || -> Result<u8> {
        let handle = HANDLE(stdin_raw as *mut _);
        let mut buf = [0u8; 1];
        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None)
                .context("ReadFile for confirmation failed")?;
        }
        Ok(buf[0])
    }).await;

    match confirm_result {
        Ok(Ok(b'y')) | Ok(Ok(b'Y')) => {
            // Get current pane ID from env var
            let pane_id = std::env::var("WMUX_PANE_ID")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let req = Request::KillPane {
                session_id: session_id.to_string(),
                pane_id,
            };
            let _ = write_message(writer, &req).await;
            // Echo confirmation
            let msg = b"\r\nPane killed.\r\n";
            let stdout_raw2 = stdout_handle.0 as isize;
            let _ = tokio::task::spawn_blocking(move || {
                let handle = HANDLE(stdout_raw2 as *mut _);
                let mut written: u32 = 0;
                unsafe {
                    let _ = WriteFile(handle, Some(msg), Some(&mut written), None);
                }
            }).await;
        }
        _ => {
            // Cancelled — echo newline and continue
            let msg = b"\r\n";
            let stdout_raw2 = stdout_handle.0 as isize;
            let _ = tokio::task::spawn_blocking(move || {
                let handle = HANDLE(stdout_raw2 as *mut _);
                let mut written: u32 = 0;
                unsafe {
                    let _ = WriteFile(handle, Some(msg), Some(&mut written), None);
                }
            }).await;
        }
    }
}

/// Enter scroll mode: display scrollback buffer and navigate with keyboard/mouse.
///
/// Prefix+[ enters, q exits. Arrow keys and Page Up/Down scroll.
/// Mouse wheel scrolling is handled via ANSI mouse escape sequences.
async fn enter_scroll_mode<R, W>(
    session_id: &str,
    reader: &mut R,
    writer: &mut W,
    stdin_raw: isize,
    stdout_handle: HANDLE,
) where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let pane_id = std::env::var("WMUX_PANE_ID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // Request initial scrollback data from daemon
    let req = Request::EnterScrollMode {
        session_id: session_id.to_string(),
        pane_id,
    };
    if let Err(e) = write_message(writer, &req).await {
        info!("Failed to send EnterScrollMode: {}", e);
        return;
    }

    // Read response with initial scrollback data
    let response: Result<Response, _> = read_message(reader).await;
    let (mut scroll_offset, mut total_lines) = match response {
        Ok(Response::ScrollModeData { data, offset, total_lines }) => {
            // Clear screen and display scrollback content
            write_to_stdout(stdout_handle, b"\x1B[2J\x1B[H").await;
            write_to_stdout(stdout_handle, &data).await;
            write_scroll_status(stdout_handle, offset, total_lines).await;
            (offset, total_lines)
        }
        Ok(Response::Error { message }) => {
            info!("Scroll mode error: {}", message);
            return;
        }
        _ => {
            info!("Unexpected response for scroll mode");
            return;
        }
    };

    // Scroll mode input loop — normal keystrokes are NOT forwarded to shell
    loop {
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

        match stdin_task.await {
            Ok(Ok(data)) if !data.is_empty() => {
                let mut exit_scroll = false;
                let mut new_offset: Option<i64> = None;

                let mut j = 0;
                while j < data.len() {
                    let byte = data[j];

                    // Check for ANSI escape sequences (arrow keys, page up/down, mouse)
                    if byte == 0x1B && j + 2 < data.len() && data[j + 1] == b'[' {
                        match data[j + 2] {
                            b'A' => {
                                // Arrow Up — scroll up 1 line
                                new_offset = Some((scroll_offset as i64) - 1);
                                j += 3;
                                continue;
                            }
                            b'B' => {
                                // Arrow Down — scroll down 1 line
                                new_offset = Some((scroll_offset as i64) + 1);
                                j += 3;
                                continue;
                            }
                            b'5' if j + 3 < data.len() && data[j + 3] == b'~' => {
                                // Page Up
                                new_offset = Some((scroll_offset as i64) - SCROLL_PAGE_SIZE as i64);
                                j += 4;
                                continue;
                            }
                            b'6' if j + 3 < data.len() && data[j + 3] == b'~' => {
                                // Page Down
                                new_offset = Some((scroll_offset as i64) + SCROLL_PAGE_SIZE as i64);
                                j += 4;
                                continue;
                            }
                            b'M' if j + 5 < data.len() => {
                                // Mouse event (X10 mode): ESC [ M button col row
                                let button = data[j + 3];
                                if button == 96 {
                                    // Mouse wheel up
                                    new_offset = Some((scroll_offset as i64) - 3);
                                } else if button == 97 {
                                    // Mouse wheel down
                                    new_offset = Some((scroll_offset as i64) + 3);
                                }
                                j += 6;
                                continue;
                            }
                            _ => {}
                        }
                    }

                    match byte {
                        b'q' | b'Q' => {
                            exit_scroll = true;
                            break;
                        }
                        b'k' => {
                            // Vim-style up
                            new_offset = Some((scroll_offset as i64) - 1);
                        }
                        b'j' => {
                            // Vim-style down
                            new_offset = Some((scroll_offset as i64) + 1);
                        }
                        b'g' => {
                            // Go to top
                            new_offset = Some(0);
                        }
                        b'G' => {
                            // Go to bottom
                            new_offset = Some(total_lines.saturating_sub(SCROLL_PAGE_SIZE) as i64);
                        }
                        _ => {
                            // Ignore all other keys in scroll mode
                        }
                    }

                    j += 1;
                }

                if exit_scroll {
                    // Notify daemon we're leaving scroll mode
                    let req = Request::ExitScrollMode {
                        session_id: session_id.to_string(),
                        pane_id,
                    };
                    let _ = write_message(writer, &req).await;
                    // Clear scroll view so normal output can resume
                    write_to_stdout(stdout_handle, b"\x1B[2J\x1B[H").await;
                    break;
                }

                if let Some(target) = new_offset {
                    // Clamp offset to valid range
                    let clamped = target
                        .max(0)
                        .min(total_lines.saturating_sub(1) as i64) as i32;
                    scroll_offset = clamped as usize;

                    // Request scroll data from daemon
                    let req = Request::ScrollBack {
                        session_id: session_id.to_string(),
                        pane_id,
                        lines: clamped,
                    };
                    if let Err(e) = write_message(writer, &req).await {
                        info!("Failed to send ScrollBack: {}", e);
                        break;
                    }

                    // Read scroll response
                    match read_message::<_, Response>(reader).await {
                        Ok(Response::ScrollModeData { data, offset, total_lines: tl }) => {
                            scroll_offset = offset;
                            total_lines = tl;
                            write_to_stdout(stdout_handle, b"\x1B[2J\x1B[H").await;
                            write_to_stdout(stdout_handle, &data).await;
                            write_scroll_status(stdout_handle, offset, total_lines).await;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            info!("Scroll mode read error: {}", e);
                            break;
                        }
                    }
                }
            }
            _ => break,
        }
    }
}

/// Write bytes to stdout via spawn_blocking.
async fn write_to_stdout(stdout_handle: HANDLE, data: &[u8]) {
    let stdout_raw = stdout_handle.0 as isize;
    let data = data.to_vec();
    let _ = tokio::task::spawn_blocking(move || {
        let handle = HANDLE(stdout_raw as *mut _);
        let mut written: u32 = 0;
        unsafe {
            let _ = WriteFile(handle, Some(&data), Some(&mut written), None);
        }
    }).await;
}

/// Write scroll position indicator to the top of the screen.
async fn write_scroll_status(stdout_handle: HANDLE, offset: usize, total: usize) {
    let status = format!(
        "\x1B[s\x1B[1;1H\x1B[7m [scroll: {}/{} | q:quit arrows:scroll] \x1B[27m\x1B[u",
        offset + 1,
        total
    );
    write_to_stdout(stdout_handle, status.as_bytes()).await;
}

/// Guard that restores the original console codepage when dropped.
struct CodepageGuard {
    original_input_cp: u32,
    original_output_cp: u32,
}

impl Drop for CodepageGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Console::SetConsoleCP(self.original_input_cp);
            let _ = windows::Win32::System::Console::SetConsoleOutputCP(self.original_output_cp);
        }
    }
}

/// Set console input and output codepage to UTF-8 (65001).
/// Returns a guard that restores original codepages on drop.
fn set_utf8_codepage() -> CodepageGuard {
    unsafe {
        let original_input_cp = windows::Win32::System::Console::GetConsoleCP();
        let original_output_cp = windows::Win32::System::Console::GetConsoleOutputCP();
        let _ = windows::Win32::System::Console::SetConsoleCP(65001);
        let _ = windows::Win32::System::Console::SetConsoleOutputCP(65001);
        CodepageGuard {
            original_input_cp,
            original_output_cp,
        }
    }
}

/// Enter raw console mode, returning a guard that restores the original mode on drop.
fn enter_raw_mode() -> Result<ConsoleRawModeGuard> {
    unsafe {
        let stdin_handle = GetStdHandle(STD_INPUT_HANDLE)?;
        let mut original_mode = CONSOLE_MODE::default();
        GetConsoleMode(stdin_handle, &mut original_mode)
            .context("Failed to get console mode")?;

        // Raw mode: disable line buffering, echo, and Ctrl+C processing.
        // Enable VT input for proper escape sequence passthrough (arrow keys etc).
        // Enable window input for resize events.
        let raw_mode = (original_mode
            & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
            | ENABLE_VIRTUAL_TERMINAL_INPUT
            | ENABLE_WINDOW_INPUT;
        SetConsoleMode(stdin_handle, raw_mode)
            .context("Failed to set raw console mode")?;

        Ok(ConsoleRawModeGuard {
            stdin_handle,
            original_mode,
        })
    }
}
