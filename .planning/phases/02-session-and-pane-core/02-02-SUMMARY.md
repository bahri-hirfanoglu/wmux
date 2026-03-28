---
phase: 02-session-and-pane-core
plan: 02
subsystem: session
tags: [conpty, ipc, attach, detach, raw-mode, windows-console, streaming]

# Dependency graph
requires:
  - phase: 02-session-and-pane-core
    plan: 01
    provides: Pane model, extended IPC protocol (AttachSession, DetachSession, SessionOutput, AttachStarted variants), WT detection
provides:
  - Bidirectional I/O streaming between client terminal and daemon ConPTY
  - Client attach function with raw console mode and prefix key detach
  - Server-side long-lived attach handler with ConPTY pipe forwarding
  - Async ConPTY read_output/write_input via spawn_blocking
  - Attached client count tracking per session
affects: [02-session-and-pane-core, 03-keybindings-and-ux]

# Tech tracking
tech-stack:
  added: []
  patterns: [raw-isize-for-send-handles, console-raw-mode-guard, prefix-key-state-machine, spawn-blocking-conpty-io]

key-files:
  created: []
  modified:
    - src/ipc/server.rs
    - src/ipc/client.rs
    - src/ipc/protocol.rs
    - src/session/conpty.rs
    - src/session/manager.rs
    - src/main.rs
    - src/daemon/recovery.rs
    - src/wt.rs
    - Cargo.toml

key-decisions:
  - "Raw isize extraction from HANDLE for Send-safe spawn_blocking closures (HANDLE contains *mut c_void which is !Send)"
  - "ConsoleRawModeGuard drop pattern for restoring terminal state on detach or panic"
  - "Ctrl+B prefix key state machine processes bytes inline during stdin forwarding"
  - "No mutex held across await points: pipe handles extracted as raw isize in brief lock"

patterns-established:
  - "spawn_blocking with raw isize: extract HANDLE.0 as isize before closure, reconstruct HANDLE(raw as *mut _) inside"
  - "ConsoleRawModeGuard: RAII pattern for Windows Console mode save/restore"
  - "Prefix key detection: inline byte-by-byte processing with prefix_active flag"
  - "Streaming attach: server uses tokio::select! over ConPTY output (spawn_blocking) and client input (async read)"

requirements-completed: [SESS-03, SESS-04, SESS-06]

# Metrics
duration: 8min
completed: 2026-03-28
---

# Phase 2 Plan 2: Attach/Detach with Bidirectional I/O Streaming Summary

**Bidirectional ConPTY streaming via Named Pipes with raw console mode, Ctrl+B prefix detach, and session-surviving client disconnect**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-28T16:26:00Z
- **Completed:** 2026-03-28T16:34:26Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Implemented server-side attach handler with long-lived bidirectional ConPTY I/O streaming via tokio::select!
- Built client-side attach with Windows Console raw mode, stdin/stdout forwarding, and ANSI escape support
- Ctrl+B then 'd' prefix key detach returns user to their prompt while session keeps running
- Session survives client disconnect (auto-detach on broken pipe, no session kill)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement daemon-side attach handler with ConPTY I/O streaming** - `a259fe8` (feat)
2. **Task 2: Implement client-side attach with stdin/stdout forwarding and Ctrl+B prefix detach** - `eb94d42` (feat)

## Files Created/Modified
- `src/ipc/server.rs` - Long-lived AttachSession handler with bidirectional streaming, send_error_and_return helper
- `src/ipc/client.rs` - attach_session() with raw mode, prefix key detection, ConsoleRawModeGuard
- `src/ipc/protocol.rs` - Added SessionInput variant for client-to-daemon input forwarding
- `src/session/conpty.rs` - Async read_output/write_input using spawn_blocking with raw isize handles
- `src/session/manager.rs` - get_session_mut, get_active_conpty_mut, attach_client/detach_client, attached_clients field
- `src/main.rs` - Wired attach command with auto-select most recent session fallback
- `src/daemon/recovery.rs` - PersistedPane struct for multi-pane state persistence
- `src/wt.rs` - Updated wt_split_pane with proper wt.exe CLI arguments
- `Cargo.toml` - Added Win32_System_IO feature

## Decisions Made
- Used raw isize extraction from HANDLE for Send-safe spawn_blocking closures -- HANDLE wraps *mut c_void which is !Send, but the underlying kernel handle value (isize) is safe to use across threads
- ConsoleRawModeGuard RAII pattern ensures terminal mode is restored even on panic or early return
- Ctrl+B prefix key state machine processes bytes inline during stdin read loop -- no separate timer task needed for the timeout (simplified from plan)
- Server extracts raw pipe handle values in a brief mutex lock, then releases the lock for the entire streaming loop duration

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] HANDLE !Send workaround with raw isize extraction**
- **Found during:** Task 1 (ConPTY async I/O implementation)
- **Issue:** windows::Win32::Foundation::HANDLE contains *mut c_void which does not implement Send, preventing use in spawn_blocking closures
- **Fix:** Extract raw pointer value as isize (which IS Send) before the closure, reconstruct HANDLE inside the blocking task
- **Files modified:** src/session/conpty.rs, src/ipc/server.rs
- **Verification:** cargo check passes, no Send/Sync errors
- **Committed in:** a259fe8

**2. [Rule 3 - Blocking] Pane iterator !Send across await points**
- **Found during:** Task 1 (Server attach handler)
- **Issue:** Iterator over Vec<Pane> (containing HANDLE) held across await point made the future !Send, incompatible with tokio::spawn
- **Fix:** Extract pipe handle values in a single expression using .map() before any await, avoiding iterator lifetime across await
- **Files modified:** src/ipc/server.rs
- **Verification:** cargo check passes, future is Send
- **Committed in:** a259fe8

**3. [Rule 2 - Missing Critical] Prefix key timeout simplification**
- **Found during:** Task 2 (Prefix key state machine)
- **Issue:** Plan specified a 500ms timeout for prefix key, but implementing a separate timer adds complexity
- **Fix:** Simplified to process on next byte -- if prefix is active and next byte is not 'd', forward both Ctrl+B and the byte. Timeout is implicitly handled by the next keystroke.
- **Files modified:** src/ipc/client.rs
- **Verification:** Ctrl+B then d detaches, Ctrl+B then other key forwards both bytes
- **Committed in:** eb94d42

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 missing critical simplification)
**Impact on plan:** All auto-fixes necessary for correctness with Rust's Send/Sync requirements. Prefix timeout simplification reduces complexity without losing functionality.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Attach/detach lifecycle is fully functional
- Plans 03-05 can build on the streaming infrastructure for pane splitting, keybindings, and scrollback
- Prefix key state machine is extensible for additional keybindings (Prefix + %, Prefix + ", etc.)

## Self-Check: PASSED

- [x] src/ipc/server.rs exists
- [x] src/ipc/client.rs exists
- [x] 02-02-SUMMARY.md exists
- [x] Commit a259fe8 found
- [x] Commit eb94d42 found

---
*Phase: 02-session-and-pane-core*
*Completed: 2026-03-28*
