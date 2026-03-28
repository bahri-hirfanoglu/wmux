---
phase: 02-session-and-pane-core
plan: 01
subsystem: session
tags: [conpty, ipc, pane, windows-terminal, clap]

# Dependency graph
requires:
  - phase: 01-daemon-foundation
    provides: ConPtySession, SessionManager, IPC protocol, CLI skeleton
provides:
  - Pane struct with ConPTY ownership and lifecycle management
  - Extended IPC protocol with all Phase 2 request/response variants
  - Windows Terminal detection module (WT_SESSION env var)
  - Updated CLI arguments for attach, split, kill-pane with flags
  - SessionManager pane CRUD operations (add, kill, resize, navigate)
affects: [02-session-and-pane-core, 03-keybindings-and-ux]

# Tech tracking
tech-stack:
  added: []
  patterns: [pane-per-conpty, wt-session-detection, split-direction-enum]

key-files:
  created:
    - src/session/pane.rs
    - src/wt.rs
  modified:
    - src/session/manager.rs
    - src/session/mod.rs
    - src/ipc/protocol.rs
    - src/ipc/server.rs
    - src/cli.rs
    - src/main.rs
    - src/lib.rs
    - src/session/conpty.rs
    - tests/ipc_protocol_test.rs

key-decisions:
  - "Pane IDs are session-scoped u32 assigned incrementally, not globally unique"
  - "ConPtySession::resize() added using ResizePseudoConsole Win32 API"
  - "WT detection via WT_SESSION env var (standard WT detection method)"
  - "Split/KillPane commands use session '1' placeholder until session context is wired in Plan 02"

patterns-established:
  - "Pane-per-ConPTY: each pane owns its own ConPtySession for process isolation"
  - "Session contains Vec<Pane> with active_pane index for focus tracking"
  - "WT gate pattern: require_windows_terminal() before WT-dependent commands"

requirements-completed: [SESS-01, PANE-05, INTG-01]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 2 Plan 1: Session and Pane Data Model Summary

**Pane-aware session model with ConPTY ownership, extended IPC protocol for 7 new command types, Windows Terminal detection, and CLI argument updates**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T16:18:03Z
- **Completed:** 2026-03-28T16:23:14Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Created Pane struct that owns a ConPtySession per pane, with lifecycle management and active-pane tracking
- Extended IPC protocol with SplitPane, KillPane, NavigatePane, ResizePane, ScrollBack, EnterScrollMode, ExitScrollMode requests and PaneInfo, SessionOutput, AttachStarted responses
- Built Windows Terminal detection module using WT_SESSION environment variable with wt.exe CLI command helpers
- Updated CLI with proper arguments: attach [session-id], split -h/-v, kill-pane --pane-id N

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Pane model and extend IPC protocol** - `d4b318a` (feat)
2. **Task 2: Windows Terminal detection and CLI updates** - `f65f5d8` (feat)

## Files Created/Modified
- `src/session/pane.rs` - Pane struct with ConPTY ownership, lifecycle methods, from_conpty for recovery
- `src/wt.rs` - WT detection (is_windows_terminal, require_windows_terminal) and wt.exe command wrappers
- `src/session/manager.rs` - Session uses Vec<Pane>, added add_pane/kill_pane/resize_pane/set_active_pane
- `src/session/conpty.rs` - Added resize() method using ResizePseudoConsole
- `src/ipc/protocol.rs` - SplitDirection, NavDirection enums; 7 new Request variants, 3 new Response variants
- `src/ipc/server.rs` - Handlers for SplitPane, KillPane, ResizePane IPC commands
- `src/cli.rs` - Updated Attach/Split/KillPane with proper arguments and flags
- `src/main.rs` - Wired Split/KillPane through IPC, WT gate on attach/split/kill-pane
- `src/lib.rs` - Added pub mod wt
- `src/session/mod.rs` - Added pub mod pane and re-export
- `tests/ipc_protocol_test.rs` - Updated SessionInfo to include pane_count field

## Decisions Made
- Pane IDs are session-scoped (0, 1, 2...) not globally unique -- simpler addressing model matching WT's pane index system
- Added ConPtySession::resize() using ResizePseudoConsole Win32 API to support pane resize operations
- WT detection uses WT_SESSION environment variable -- the standard and reliable detection method
- Split and KillPane commands currently use session "1" as placeholder until session context tracking is wired in Plan 02

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added ConPtySession::resize() method**
- **Found during:** Task 1 (Pane model creation)
- **Issue:** Plan called for resize_pane in manager but ConPtySession had no resize method
- **Fix:** Added resize() method using ResizePseudoConsole Win32 API, updating internal cols/rows state
- **Files modified:** src/session/conpty.rs
- **Verification:** cargo check passes
- **Committed in:** d4b318a (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added Pane::from_conpty() for crash recovery**
- **Found during:** Task 1 (Session manager update)
- **Issue:** restore_session() needed to wrap existing ConPtySession in a Pane without spawning new shell
- **Fix:** Added Pane::from_conpty(id, conpty) constructor for recovery path
- **Files modified:** src/session/pane.rs
- **Verification:** cargo check passes, recovery_test passes
- **Committed in:** d4b318a (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 missing critical)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 2 data structures and IPC protocol are in place
- Plans 02-05 can build against the Pane model, extended protocol, and WT detection module
- Attach/detach wiring needed in Plan 02
- Keybinding interception needed in later plans

---
*Phase: 02-session-and-pane-core*
*Completed: 2026-03-28*
