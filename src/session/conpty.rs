use std::mem;
use std::ptr;

use anyhow::{Context, Result};
use tracing::info;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
    TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW, STARTUPINFOW,
};

/// A ConPTY session wrapping a pseudo console and its child shell process.
pub struct ConPtySession {
    hpc: HPCON,
    process_handle: HANDLE,
    thread_handle: HANDLE,
    process_id: u32,
    /// Write end of the input pipe (daemon writes here to send input to the shell).
    #[allow(dead_code)]
    pipe_in: HANDLE,
    /// Read end of the output pipe (daemon reads here to get output from the shell).
    #[allow(dead_code)]
    pipe_out: HANDLE,
    /// Attribute list buffer — must live as long as the process is alive.
    _attr_list_buf: Vec<u8>,
    /// Whether this session has already been cleaned up.
    closed: bool,
    /// Shell executable path used to create this session.
    shell: String,
    /// Terminal column count.
    cols: i16,
    /// Terminal row count.
    rows: i16,
}

// HANDLE is Send-safe (it's just an isize wrapper for a kernel object).
// ConPtySession is only accessed under a Mutex, so Sync is not needed.
unsafe impl Send for ConPtySession {}


impl ConPtySession {
    /// Spawn a new shell process inside a ConPTY pseudo console.
    ///
    /// `cols` and `rows` set the initial terminal size.
    /// `shell` overrides the default shell (powershell.exe, fallback cmd.exe).
    pub fn new(cols: i16, rows: i16, shell: Option<&str>) -> Result<Self> {
        unsafe {
            // 1. Create two pipe pairs: input and output
            let mut input_read = HANDLE::default();
            let mut input_write = HANDLE::default();
            let mut output_read = HANDLE::default();
            let mut output_write = HANDLE::default();

            // Use 1MB pipe buffers to prevent shell blocking when nobody is reading output.
            // Without this, powershell's startup prompt fills the default 4KB buffer and
            // blocks indefinitely, making attach appear frozen.
            const PIPE_BUFFER_SIZE: u32 = 1024 * 1024;
            CreatePipe(&mut input_read, &mut input_write, None, PIPE_BUFFER_SIZE)
                .context("Failed to create input pipe pair")?;
            CreatePipe(&mut output_read, &mut output_write, None, PIPE_BUFFER_SIZE)
                .context("Failed to create output pipe pair")?;

            // 2. Create pseudo console
            let size = COORD { X: cols, Y: rows };
            let hpc = CreatePseudoConsole(size, input_read, output_write, 0)
                .context("Failed to create pseudo console")?;

            // 3. Set up STARTUPINFOEXW with the pseudo console attribute
            let mut attr_list_size: usize = 0;
            // First call to get required size
            let _ = InitializeProcThreadAttributeList(
                LPPROC_THREAD_ATTRIBUTE_LIST(ptr::null_mut()),
                1,
                0,
                &mut attr_list_size,
            );

            let mut attr_list_buf = vec![0u8; attr_list_size];
            let attr_list =
                LPPROC_THREAD_ATTRIBUTE_LIST(attr_list_buf.as_mut_ptr() as *mut _);

            InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_list_size)
                .context("Failed to initialize proc thread attribute list")?;

            // CRITICAL: lpValue must be the HPCON value itself (it's an opaque handle),
            // NOT a pointer to it. HPCON.0 is already a pointer-sized value.
            // Passing &hpc (pointer-to-pointer) causes STATUS_DLL_INIT_FAILED (0xC0000142).
            UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(hpc.0 as *const _),
                mem::size_of::<HPCON>(),
                None,
                None,
            )
            .context("Failed to set pseudo console attribute")?;

            let startup_info = STARTUPINFOEXW {
                StartupInfo: STARTUPINFOW {
                    cb: mem::size_of::<STARTUPINFOEXW>() as u32,
                    ..Default::default()
                },
                lpAttributeList: attr_list,
            };

            // 4. Determine shell to launch
            let shell_cmd = if let Some(s) = shell {
                s.to_string()
            } else {
                // Try powershell first, fallback to cmd
                if which_shell("powershell.exe") {
                    "powershell.exe".to_string()
                } else {
                    "cmd.exe".to_string()
                }
            };

            let mut cmd_line: Vec<u16> = shell_cmd.encode_utf16().chain(std::iter::once(0)).collect();

            // 5. Create the child process
            let mut proc_info = PROCESS_INFORMATION::default();

            // IMPORTANT: When using EXTENDED_STARTUPINFO_PRESENT, Windows expects
            // lpStartupInfo to point to the STARTUPINFOEXW struct. We cast the
            // pointer to &STARTUPINFOW since that's what the Rust binding expects,
            // but the actual memory layout includes lpAttributeList right after.
            let startup_ptr = &startup_info as *const STARTUPINFOEXW as *const STARTUPINFOW;
            CreateProcessW(
                None,
                windows::core::PWSTR(cmd_line.as_mut_ptr()),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                None,
                None,
                startup_ptr,
                &mut proc_info,
            )
            .context("Failed to create shell process")?;

            let process_id = proc_info.dwProcessId;

            // Close the pipe ends that were given to CreatePseudoConsole.
            // ConPTY internally duplicates these handles, so closing our copies is safe
            // and necessary to avoid handle leaks.
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(output_write);

            // 7. Verify the child process is still alive after a brief pause.
            //    If the process exits immediately, ConPTY setup likely failed.
            std::thread::sleep(std::time::Duration::from_millis(200));
            let alive_check = WaitForSingleObject(proc_info.hProcess, 0);
            if alive_check != WAIT_TIMEOUT {
                // Process already exited — get exit code for diagnostics
                let mut exit_code: u32 = 0;
                let _ = windows::Win32::System::Threading::GetExitCodeProcess(
                    proc_info.hProcess,
                    &mut exit_code,
                );
                let _ = CloseHandle(proc_info.hProcess);
                let _ = CloseHandle(proc_info.hThread);
                anyhow::bail!(
                    "Shell '{}' exited immediately (exit code: {}). ConPTY may not be working correctly.",
                    shell_cmd,
                    exit_code
                );
            }

            info!(
                "ConPTY session created: shell='{}', pid={}, cols={}, rows={}",
                shell_cmd, process_id, cols, rows
            );

            Ok(ConPtySession {
                hpc,
                process_handle: proc_info.hProcess,
                thread_handle: proc_info.hThread,
                process_id,
                pipe_in: input_write,
                pipe_out: output_read,
                _attr_list_buf: attr_list_buf,
                closed: false,
                shell: shell_cmd.clone(),
                cols,
                rows,
            })
        }
    }

    /// Return the child process ID.
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    /// Return the terminal column count.
    pub fn cols(&self) -> i16 {
        self.cols
    }

    /// Return the terminal row count.
    pub fn rows(&self) -> i16 {
        self.rows
    }

    /// Return the shell executable path.
    pub fn shell(&self) -> &str {
        &self.shell
    }

    /// Resize the pseudo console to new dimensions.
    pub fn resize(&mut self, cols: i16, rows: i16) -> Result<()> {
        use windows::Win32::System::Console::ResizePseudoConsole;
        let size = COORD { X: cols, Y: rows };
        unsafe {
            ResizePseudoConsole(self.hpc, size)
                .context("Failed to resize pseudo console")?;
        }
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Return the raw write-end handle for the ConPTY input pipe.
    ///
    /// The caller can use this HANDLE (which is Copy) for I/O without
    /// holding a borrow on ConPtySession across await points.
    pub fn pipe_in_handle(&self) -> HANDLE {
        self.pipe_in
    }

    /// Return the raw read-end handle for the ConPTY output pipe.
    pub fn pipe_out_handle(&self) -> HANDLE {
        self.pipe_out
    }

    /// Asynchronously read output from the ConPTY output pipe.
    ///
    /// Uses `spawn_blocking` because anonymous pipes from CreatePipe()
    /// do NOT support overlapped I/O -- synchronous ReadFile in a
    /// blocking thread is the standard ConPTY pattern.
    pub async fn read_output(&self, buf: &mut [u8]) -> Result<usize> {
        // Extract raw pointer as isize (which is Send) to avoid HANDLE's !Send bound.
        let raw = self.pipe_out.0 as isize;
        let buf_len = buf.len();
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let handle = HANDLE(raw as *mut _);
            let mut tmp = vec![0u8; buf_len];
            let mut bytes_read: u32 = 0;
            unsafe {
                ReadFile(handle, Some(&mut tmp), Some(&mut bytes_read), None)
                    .context("ReadFile from ConPTY output pipe failed")?;
            }
            tmp.truncate(bytes_read as usize);
            Ok(tmp)
        })
        .await
        .context("spawn_blocking for ConPTY read panicked")??;

        let n = result.len();
        buf[..n].copy_from_slice(&result);
        Ok(n)
    }

    /// Asynchronously write input to the ConPTY input pipe.
    pub async fn write_input(&self, data: &[u8]) -> Result<usize> {
        let raw = self.pipe_in.0 as isize;
        let data = data.to_vec();
        let bytes_written = tokio::task::spawn_blocking(move || -> Result<usize> {
            let handle = HANDLE(raw as *mut _);
            let mut written: u32 = 0;
            unsafe {
                WriteFile(handle, Some(&data), Some(&mut written), None)
                    .context("WriteFile to ConPTY input pipe failed")?;
            }
            Ok(written as usize)
        })
        .await
        .context("spawn_blocking for ConPTY write panicked")??;

        Ok(bytes_written)
    }

    /// Check if the child process is still running.
    pub fn is_alive(&self) -> bool {
        if self.closed {
            return false;
        }
        unsafe {
            let result = WaitForSingleObject(self.process_handle, 0);
            result == WAIT_TIMEOUT
        }
    }

    /// Terminate the child process and close all handles.
    pub fn kill(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        unsafe {
            // Terminate the child process if still alive
            if self.is_alive() {
                let _ = TerminateProcess(self.process_handle, 1);
                // Wait briefly for process to actually exit
                WaitForSingleObject(self.process_handle, 1000);
            }

            // Close pseudo console
            ClosePseudoConsole(self.hpc);

            // Close process and thread handles
            let _ = CloseHandle(self.process_handle);
            let _ = CloseHandle(self.thread_handle);

            // Close pipe handles
            let _ = CloseHandle(self.pipe_in);
            let _ = CloseHandle(self.pipe_out);

            // Clean up attribute list
            DeleteProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(
                self._attr_list_buf.as_mut_ptr() as *mut _,
            ));

            self.closed = true;

            info!("ConPTY session killed: pid={}", self.process_id);
        }
        Ok(())
    }
}

impl Drop for ConPtySession {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.kill();
        }
    }
}

/// Check if a shell executable exists on PATH.
fn which_shell(name: &str) -> bool {
    std::process::Command::new("where")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
