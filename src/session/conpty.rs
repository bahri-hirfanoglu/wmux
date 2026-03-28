use std::mem;
use std::ptr;

use anyhow::{Context, Result};
use tracing::info;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
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

            CreatePipe(&mut input_read, &mut input_write, None, 0)
                .context("Failed to create input pipe pair")?;
            CreatePipe(&mut output_read, &mut output_write, None, 0)
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

            UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(&hpc.0 as *const _ as *const _),
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

            CreateProcessW(
                None,
                windows::core::PWSTR(cmd_line.as_mut_ptr()),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                None,
                None,
                &startup_info.StartupInfo as *const STARTUPINFOW,
                &mut proc_info,
            )
            .context("Failed to create shell process")?;

            let process_id = proc_info.dwProcessId;

            // 6. Close the child-side pipe ends — daemon keeps input_write and output_read
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(output_write);

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
            })
        }
    }

    /// Return the child process ID.
    pub fn process_id(&self) -> u32 {
        self.process_id
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
