---
phase: 01-daemon-foundation
plan: 03
subsystem: session
tags: [rust, conpty, windows, session-management, createpseudoconsole]

# Dependency graph
requires:
  - phase: 01-02
    provides: "Bidirectional JSON-over-Named-Pipe IPC (client/server)"
provides:
  - "ConPTY pseudo console wrapper (CreatePseudoConsole + CreateProcessW)"
  - "Session manager with create, list, kill, kill-all operations"
  - "CLI commands: wmux new, wmux ls, wmux kill-session"
  - "Daemon-owned shell processes that outlive client connections"
affects: [01-04, 02-session-management]

# Tech tracking
tech-stack:
  added: [conpty, createpseudoconsole]
  patterns: [arc-mutex-session-manager, conpty-pipe-pair, handle-cleanup-on-drop]

key-files:
  created:
    - src/session/mod.rs
    - src/session/conpty.rs
    - src/session/manager.rs
  modified:
    - src/ipc/server.rs
    - src/daemon/lifecycle.rs
    - src/main.rs
    - src/cli.rs
    - src/lib.rs
    - Cargo.toml

key-decisions:
  - "ConPTY pipes: daemon keeps input_write and output_read; child gets input_read and output_write (closed after spawn)"
  - "SessionManager shared via Arc<tokio::sync::Mutex> for async-safe access from IPC handlers"
  - "Default shell: powershell.exe with cmd.exe fallback (detected via `where` command)"
  - "Kill-server cleans up all active sessions before daemon exit"

patterns-established:
  - "ConPTY pattern: CreatePipe pairs + CreatePseudoConsole + STARTUPINFOEXW with attribute list"
  - "Session ownership: daemon holds all handles, Drop impl ensures cleanup"
  - "Arc<Mutex<SessionManager>> passed to ControlServer for shared mutable access"

requirements-completed: [DAEMON-02, INTG-03]

# Metrics
duration: 6min
completed: 2026-03-28
---

# Phase 1 Plan 3: ConPTY Shell Spawning and Session Management Summary

**ConPTY shell spawning with session lifecycle management -- daemon creates, tracks, and cleans up real terminal sessions**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-28T15:05:01Z
- **Completed:** 2026-03-28T15:10:40Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Implemented ConPTY wrapper using CreatePseudoConsole + CreateProcessW to spawn shell processes owned by the daemon
- Built SessionManager with create/list/kill/kill-all operations, tracked via HashMap with auto-incrementing IDs
- Wired wmux new, wmux ls, wmux kill-session commands end-to-end through IPC
- Verified full session lifecycle: new -> ls (2 sessions) -> status (2 sessions) -> kill-session 1 -> ls (1 session) -> kill-server (cleanup)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ConPTY wrapper and session manager** - `8319c23` (feat)
2. **Task 2: Wire session commands through IPC and test end-to-end** - `b7c51d0` (feat)

## Files Created/Modified
- `src/session/mod.rs` - Module declarations and re-exports for SessionManager
- `src/session/conpty.rs` - ConPTY wrapper: CreatePseudoConsole, CreateProcessW, pipe pairs, handle cleanup
- `src/session/manager.rs` - SessionManager: create, list, kill sessions with ConPtySession instances
- `src/ipc/server.rs` - Handle NewSession, ListSessions, KillSession with Arc<Mutex<SessionManager>>
- `src/daemon/lifecycle.rs` - Create SessionManager in run_daemon, pass to ControlServer
- `src/main.rs` - Wire wmux new/ls/kill-session CLI commands through IPC client
- `src/cli.rs` - Add session ID argument to KillSession variant
- `src/lib.rs` - Expose session module publicly
- `Cargo.toml` - Added clippy lints section

## Decisions Made
- ConPTY pipe management: daemon keeps input_write (to send to shell) and output_read (to receive from shell), child-side ends closed after spawn
- SessionManager wrapped in Arc<tokio::sync::Mutex> for async-safe sharing between daemon main loop and IPC connection handlers
- Default shell detection: tries powershell.exe first (via `where` command), falls back to cmd.exe
- Kill-server performs session cleanup (kill_all) before daemon exit to prevent orphaned shell processes
- Drop trait implemented on ConPtySession to ensure handle cleanup even if kill() is not called explicitly

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ConPTY sessions fully operational, ready for terminal I/O streaming (Plan 04)
- Session pipe handles (pipe_in, pipe_out) available for future attach/detach I/O forwarding
- All session operations route through established IPC protocol

## Self-Check: PASSED

- All 3 session source files verified present
- Commit 8319c23 (Task 1) verified in git log
- Commit b7c51d0 (Task 2) verified in git log
- End-to-end verified: daemon-start -> new (x2) -> ls (2 sessions) -> status (2 sessions) -> kill-session 1 -> ls (1 session) -> kill-server

---
*Phase: 01-daemon-foundation*
*Completed: 2026-03-28*
