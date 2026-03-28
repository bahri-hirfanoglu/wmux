---
phase: 01-daemon-foundation
plan: 01
subsystem: daemon
tags: [rust, tokio, clap, windows-rs, daemon, pid-management]

# Dependency graph
requires: []
provides:
  - "Single wmux binary with CLI subcommand dispatch"
  - "Daemon lifecycle: start, status, kill-server"
  - "PID file management at %LOCALAPPDATA%\\wmux\\wmux.pid"
  - "Centralized path resolution for data/log/state/pipe paths"
  - "Detached background process via DETACHED_PROCESS | CREATE_NO_WINDOW"
affects: [01-02, 01-03, 01-04, 02-session-management]

# Tech tracking
tech-stack:
  added: [tokio, clap, serde, windows-rs, tracing, tracing-subscriber, anyhow]
  patterns: [clap-derive-cli, windows-process-management, pid-file-lifecycle]

key-files:
  created:
    - Cargo.toml
    - src/main.rs
    - src/cli.rs
    - src/paths.rs
    - src/daemon/mod.rs
    - src/daemon/lifecycle.rs
  modified: []

key-decisions:
  - "Hidden --daemon-mode flag on Cli struct for internal daemon re-spawn (not a hidden subcommand)"
  - "Flat subcommand names (daemon-start, kill-server) rather than nested (daemon start)"
  - "DETACHED_PROCESS | CREATE_NO_WINDOW creation flags for daemon backgrounding"

patterns-established:
  - "CLI pattern: clap derive with flat subcommand naming"
  - "Path pattern: all wmux paths resolved via src/paths.rs functions"
  - "Daemon pattern: self-re-spawn with hidden flag, PID file tracking"

requirements-completed: [CLI-01, DAEMON-01]

# Metrics
duration: 3min
completed: 2026-03-28
---

# Phase 1 Plan 1: Project Bootstrap and Daemon Lifecycle Summary

**Rust project with single wmux binary providing daemon lifecycle (start/status/kill) via detached Windows process with PID file tracking**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-28T14:53:11Z
- **Completed:** 2026-03-28T14:56:08Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Scaffolded complete Rust project with all Phase 1 dependencies and flat CLI subcommand structure
- Implemented daemon lifecycle with start (detached process spawn), status (PID + process liveness check), and kill-server (TerminateProcess)
- PID file management with creation on daemon start, removal on clean kill, and stale file cleanup
- File-based tracing logging to %LOCALAPPDATA%\wmux\wmux.log

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold Rust project with CLI and daemon module structure** - `05d527e` (feat)
2. **Task 2: Implement daemon lifecycle -- start, status, kill-server** - `80f85b1` (feat)

## Files Created/Modified
- `Cargo.toml` - Project manifest with tokio, clap, serde, windows-rs, tracing, anyhow
- `src/main.rs` - Entry point with CLI parse, daemon-mode check, command dispatch
- `src/cli.rs` - Clap derive CLI with all wmux subcommands (daemon-start, status, kill-server, new, ls, attach, detach, kill-session, kill-pane, split)
- `src/paths.rs` - Centralized path resolution for data dir, PID file, log file, state file, control pipe
- `src/daemon/mod.rs` - Module declaration with lifecycle re-export
- `src/daemon/lifecycle.rs` - Full daemon process management: start, run, status, kill with Windows API integration

## Decisions Made
- Used hidden `--daemon-mode` arg on the Cli struct rather than a hidden subcommand, keeping the implementation simpler
- Flat subcommand naming (daemon-start, kill-server) as specified in plan, matching the project's tmux-inspired UX
- DETACHED_PROCESS | CREATE_NO_WINDOW flags ensure daemon survives terminal close without requiring Windows Service registration

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Daemon process running and controllable, ready for Named Pipe IPC (Plan 02)
- CLI structure established with placeholder variants for all future commands
- Path resolution centralized, ready for state file and pipe usage

## Self-Check: PASSED

- All 6 source files verified present
- Commit 05d527e (Task 1) verified in git log
- Commit 80f85b1 (Task 2) verified in git log
- Full daemon lifecycle tested: start -> status (running) -> duplicate start (idempotent) -> kill-server -> status (not running)

---
*Phase: 01-daemon-foundation*
*Completed: 2026-03-28*
