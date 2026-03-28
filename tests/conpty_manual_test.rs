//! Manual test: spawn powershell via ConPTY and read its output.
//! Run with: cargo test --test conpty_manual_test -- --nocapture

use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::ReadFile;

#[test]
fn test_conpty_output() {
    // Spawn a ConPTY session
    let session = wmux::session::conpty::ConPtySession::new(80, 24, Some("cmd.exe"))
        .expect("Failed to create ConPTY");

    println!("ConPTY created, pid={}, alive={}", session.process_id(), session.is_alive());

    // Wait a moment for cmd.exe to start
    thread::sleep(Duration::from_secs(2));

    println!("After 2s sleep, alive={}", session.is_alive());

    // Try to read output
    let handle = session.pipe_out_handle();
    let raw = handle.0 as isize;

    let read_thread = thread::spawn(move || {
        let handle = HANDLE(raw as *mut _);
        let mut buf = vec![0u8; 4096];
        let mut bytes_read: u32 = 0;
        let result = unsafe {
            ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None)
        };
        match result {
            Ok(()) => {
                buf.truncate(bytes_read as usize);
                println!("Read {} bytes: {:?}", bytes_read, String::from_utf8_lossy(&buf));
            }
            Err(e) => {
                println!("ReadFile failed: {}", e);
            }
        }
    });

    // Wait max 5 seconds
    let _ = read_thread.join();
}
