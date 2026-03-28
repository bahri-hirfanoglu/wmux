//! Minimal ConPTY test — run with: cargo run --example conpty_test
//! Tests if ConPTY can spawn cmd.exe and read its output.

use std::mem;
use std::ptr;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW, STARTUPINFOW,
};

fn main() {
    println!("=== ConPTY Minimal Test ===\n");

    unsafe {
        // 1. Create pipe pairs
        let mut input_read = HANDLE::default();
        let mut input_write = HANDLE::default();
        let mut output_read = HANDLE::default();
        let mut output_write = HANDLE::default();

        CreatePipe(&mut input_read, &mut input_write, None, 1024 * 1024)
            .expect("CreatePipe input failed");
        CreatePipe(&mut output_read, &mut output_write, None, 1024 * 1024)
            .expect("CreatePipe output failed");

        println!(
            "Pipes created: input_read={:?}, input_write={:?}, output_read={:?}, output_write={:?}",
            input_read, input_write, output_read, output_write
        );

        // 2. Create pseudo console
        let size = COORD { X: 80, Y: 24 };
        let hpc = CreatePseudoConsole(size, input_read, output_write, 0)
            .expect("CreatePseudoConsole failed");

        println!("PseudoConsole created: hpc={:?}", hpc);

        // 3. Close the pipe ends given to CreatePseudoConsole
        let _ = CloseHandle(input_read);
        let _ = CloseHandle(output_write);

        // 4. Set up attribute list
        let mut attr_size: usize = 0;
        let _ = InitializeProcThreadAttributeList(
            LPPROC_THREAD_ATTRIBUTE_LIST(ptr::null_mut()),
            1,
            0,
            &mut attr_size,
        );
        println!("Attribute list size: {}", attr_size);

        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr() as *mut _);

        InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size)
            .expect("InitializeProcThreadAttributeList failed");

        println!(
            "HPCON value: {:#x}, size: {}",
            hpc.0,
            mem::size_of::<HPCON>()
        );
        println!("HPCON as ptr: {:?}", hpc.0 as *const std::ffi::c_void);

        // CRITICAL: lpValue must be the HPCON value itself (it's an opaque handle/pointer),
        // NOT a pointer to it. HPCON.0 is already a pointer-sized value.
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc.0 as *const _),
            mem::size_of::<HPCON>(),
            None,
            None,
        )
        .expect("UpdateProcThreadAttribute failed");

        println!("Attribute list updated with PSEUDOCONSOLE");

        // 5. Create process
        let startup_info = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: mem::size_of::<STARTUPINFOEXW>() as u32,
                ..Default::default()
            },
            lpAttributeList: attr_list,
        };

        let shell = "cmd.exe /c echo CONPTY_WORKS & timeout /t 5";
        let mut cmd_line: Vec<u16> = shell.encode_utf16().chain(std::iter::once(0)).collect();

        let mut proc_info = PROCESS_INFORMATION::default();

        let create_result = CreateProcessW(
            None,
            windows::core::PWSTR(cmd_line.as_mut_ptr()),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            None,
            None,
            &startup_info as *const STARTUPINFOEXW as *const STARTUPINFOW,
            &mut proc_info,
        );

        match create_result {
            Ok(()) => println!("Process created: pid={}", proc_info.dwProcessId),
            Err(e) => {
                println!("CreateProcessW FAILED: {}", e);
                return;
            }
        }

        // 6. Check if process is alive after 500ms
        thread::sleep(Duration::from_millis(500));
        let wait_result = WaitForSingleObject(proc_info.hProcess, 0);
        if wait_result == WAIT_TIMEOUT {
            println!("Process is ALIVE after 500ms");
        } else {
            let mut exit_code: u32 = 0;
            let _ = GetExitCodeProcess(proc_info.hProcess, &mut exit_code);
            println!("Process is DEAD after 500ms (exit code: {})", exit_code);
        }

        // 7. Try to read output
        println!("\nAttempting to read ConPTY output (5 second timeout)...");

        let out_raw = output_read.0 as isize;
        let read_thread = thread::spawn(move || {
            let handle = HANDLE(out_raw as *mut _);
            let mut buf = vec![0u8; 4096];
            let mut bytes_read: u32 = 0;
            let result = ReadFile(handle, Some(&mut buf), Some(&mut bytes_read), None);
            match result {
                Ok(()) => {
                    buf.truncate(bytes_read as usize);
                    let text = String::from_utf8_lossy(&buf);
                    println!("Read {} bytes:\n---\n{}\n---", bytes_read, text);
                    true
                }
                Err(e) => {
                    println!("ReadFile FAILED: {}", e);
                    false
                }
            }
        });

        match read_thread.join() {
            Ok(true) => println!("\n=== SUCCESS: ConPTY is working! ==="),
            Ok(false) => println!("\n=== FAILED: ReadFile error ==="),
            Err(_) => println!("\n=== FAILED: Read thread panicked ==="),
        }

        // Cleanup
        let _ = CloseHandle(proc_info.hProcess);
        let _ = CloseHandle(proc_info.hThread);
        ClosePseudoConsole(hpc);
        DeleteProcThreadAttributeList(attr_list);
        let _ = CloseHandle(input_write);
        let _ = CloseHandle(output_read);
    }
}
