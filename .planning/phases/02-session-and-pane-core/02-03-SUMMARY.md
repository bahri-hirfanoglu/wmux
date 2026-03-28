---
phase: 02-session-and-pane-core
plan: 03
subsystem: pane
tags: [conpty, wt-exe, split-pane, kill-pane, env-var-context]

# Dependency graph
requires:
  - phase: 02-session-and-pane-core
    provides: Pane struct, extended IPC protocol, wt.exe CLI helpers, SessionManager CRUD
provides:
  - End-to-end split pane via wt.exe with daemon-side ConPTY creation
  - Kill pane with last-pane-kills-session semantics
  - WMUX_SESSION_ID/WMUX_PANE_ID env var session context propagation
  - Multi-pane crash recovery persistence (PersistedPane)
  - --pane flag on attach for split-spawned WT panes
affects: [02-session-and-pane-core, 03-keybindings-and-ux]

# Tech tracking
tech-stack:
  added: []
  patterns: [env-var-session-context, wt-split-attach-pattern, multi-pane-persistence]

key-files:
  created: []
  modified:
    - src/main.rs
    - src/cli.rs
    - tests/recovery_test.rs

key-decisions:
  - "WMUX_SESSION_ID env var for session context propagation between attach and split/kill-pane"
  - "Split creates daemon pane first, then invokes wt.exe split-pane running wmux attach targeting new pane"
  - "WMUX_PANE_ID env var tracks current pane for kill-pane default target"
  - "PersistedPane struct with backward-compatible serde defaults for v1 state files"

patterns-established:
  - "Env var context: attach sets WMUX_SESSION_ID/WMUX_PANE_ID, child commands read them"
  - "Split flow: daemon creates ConPTY pane -> client spawns wt.exe split-pane running attach to new pane"

requirements-completed: [PANE-01, PANE-02, PANE-06, INTG-02]

# Metrics
duration: 9min
completed: 2026-03-28
---

# Phase 2 Plan 3: Pane Splitting and Lifecycle Summary

**End-to-end pane splitting via wt.exe with WMUX_SESSION_ID env var context, multi-pane persistence, and kill-pane with last-pane-kills-session semantics**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-28T16:26:06Z
- **Completed:** 2026-03-28T16:35:49Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Wired split command end-to-end: reads session from WMUX_SESSION_ID, creates daemon pane via IPC, invokes wt.exe split-pane with attach command targeting new pane
- Kill-pane reads session/pane context from env vars instead of hardcoded values, supports WMUX_PANE_ID for current pane default
- Updated attach command to set WMUX_SESSION_ID and WMUX_PANE_ID env vars, added --pane flag for split-spawned panes
- Updated recovery tests for multi-pane PersistedPane schema

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement wt.exe split-pane and daemon-side pane creation** - `eafeba8` (feat)
2. **Task 2: Wire split and kill-pane CLI commands end-to-end** - `670d073` (feat)

## Files Created/Modified
- `src/main.rs` - Split uses WMUX_SESSION_ID + wt.exe split-pane with attach; KillPane reads env vars; Attach sets env vars and accepts --pane
- `src/cli.rs` - Added --pane optional arg to Attach command for split-spawned WT panes
- `tests/recovery_test.rs` - Updated test fixtures with PersistedPane data for multi-pane persistence

## Decisions Made
- WMUX_SESSION_ID environment variable is the mechanism for session context propagation. Set during attach, read by split/kill-pane. Works because env vars are inherited by child processes in the same terminal.
- WMUX_PANE_ID is set alongside session ID to track which pane the current terminal is attached to, used as default target for kill-pane.
- Split flow is two-step: (1) send SplitPane to daemon to create ConPTY pane and get pane_id, (2) call wt.exe split-pane to create WT visual pane running `wmux attach <session> --pane <pane_id>`.
- PersistedPane struct uses serde defaults for backward compatibility with v1 state files that lack the panes array.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed HANDLE Send safety for spawn_blocking closures**
- **Found during:** Task 1
- **Issue:** ConPTY async read/write methods (added by 02-02) captured HANDLE in closures passed to spawn_blocking, but HANDLE contains *mut c_void which is !Send
- **Fix:** Extract raw pointer as isize (which is Send) and reconstruct HANDLE inside the blocking closure
- **Files modified:** src/session/conpty.rs, src/ipc/server.rs (already committed by 02-02 linter fixup)
- **Verification:** cargo check passes

**2. [Rule 3 - Blocking] Added Win32_System_IO feature for ReadFile/WriteFile**
- **Found during:** Task 1
- **Issue:** windows crate ReadFile/WriteFile require Win32_System_IO feature flag
- **Fix:** Added feature to Cargo.toml (already committed by 02-02)
- **Files modified:** Cargo.toml

---

**Total deviations:** 2 auto-fixed (both blocking issues from 02-02 concurrent changes)
**Impact on plan:** Fixes were required for compilation. Core 02-03 plan logic was unaffected.

## Issues Encountered
- 02-02 plan had already committed most Task 1 foundational work (PersistedPane, multi-pane recovery, wt.rs improvements, async ConPTY I/O). Task 1 scope was reduced to updating recovery tests.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Split and kill-pane commands fully wired end-to-end
- Pane navigation (Plan 04) and keybindings (Plan 05) can build on the env var context pattern
- Scrollback buffer implementation (Plan 04) can use existing ConPTY read_output infrastructure

---
*Phase: 02-session-and-pane-core*
*Completed: 2026-03-28*
