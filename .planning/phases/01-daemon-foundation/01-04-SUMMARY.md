---
phase: 01-daemon-foundation
plan: 04
subsystem: daemon
tags: [rust, crash-recovery, state-persistence, conpty, windows]

# Dependency graph
requires:
  - phase: 01-03
    provides: "ConPTY session lifecycle and session manager"
provides:
  - "State persistence to state.json on every structural change"
  - "Crash recovery: load persisted state and respawn sessions on daemon startup"
  - "PersistedState/PersistedSession serde-based schema"
  - "Atomic file writes (temp + rename) for state.json"
affects: [02-session-management]

# Tech tracking
tech-stack:
  added: []
  patterns: [atomic-file-write, best-effort-persistence, crash-recovery-respawn]

key-files:
  created:
    - src/daemon/recovery.rs
    - tests/recovery_test.rs
  modified:
    - src/daemon/mod.rs
    - src/daemon/lifecycle.rs
    - src/session/manager.rs
    - src/session/conpty.rs
    - src/lib.rs
    - src/main.rs

key-decisions:
  - "ConPTY handles are process-local and cannot be re-adopted after crash; always respawn shells on recovery"
  - "State persistence is best-effort: log errors but never fail session operations"
  - "Graceful kill-server persists empty state (sessions cleaned up); crash preserves state for recovery"
  - "Binary crate delegates daemon/paths modules to library crate for integration test access"

patterns-established:
  - "Atomic file write: write to .tmp then fs::rename for crash-safe persistence"
  - "Best-effort persistence: save_state errors logged, never propagated"
  - "Recovery pattern: load state -> check PIDs -> respawn shells -> restore IDs"

requirements-completed: [DAEMON-03]

# Metrics
duration: 11min
completed: 2026-03-28
---

# Phase 1 Plan 4: Crash Recovery and State Persistence Summary

**Session state persistence to state.json with automatic crash recovery -- daemon respawns shells and restores session IDs after unexpected shutdown**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-28T15:13:17Z
- **Completed:** 2026-03-28T15:24:25Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Implemented PersistedState schema with serde serialization and atomic file writes (temp + rename)
- Built crash recovery logic: detects live/dead processes, respawns shells via ConPTY, preserves session IDs
- Wired persistence into SessionManager (auto-saves after create/kill) and recovery into daemon startup
- Verified full crash recovery cycle: create 2 sessions -> force-kill daemon -> restart -> 2 sessions recovered
- 5 unit tests for serialization roundtrip, missing/corrupted state file handling, atomic writes

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement state persistence and recovery logic** - `ac61c19` (feat)
2. **Task 2: Wire persistence into session manager and recovery into daemon startup** - `5a29610` (feat)

## Files Created/Modified
- `src/daemon/recovery.rs` - PersistedState schema, save_state (atomic), load_state (graceful), recover_sessions
- `tests/recovery_test.rs` - 5 tests: roundtrip, save/load, missing file, corrupted file, atomic write
- `src/daemon/mod.rs` - Added recovery module declaration
- `src/daemon/lifecycle.rs` - Recovery on daemon startup, changed wmux:: paths to crate:: for lib crate
- `src/session/manager.rs` - to_persisted_state(), restore_session(), set_next_id(), persist after create/kill
- `src/session/conpty.rs` - Added shell/cols/rows fields and getter methods
- `src/lib.rs` - Exposed daemon and paths modules publicly for integration tests
- `src/main.rs` - Use wmux::daemon and wmux::paths from library instead of local mod

## Decisions Made
- ConPTY handles are process-local: after daemon crash, original pseudo-console sessions cannot be re-attached. The pragmatic approach is to always respawn shells and log the situation. Scrollback is lost on crash (per CONTEXT.md).
- State persistence is best-effort: save_state errors are logged but never fail session create/kill operations.
- Restructured module exposure: daemon and paths modules moved to library crate exports so integration tests can access wmux::daemon::recovery.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Module crate reference fix for dual-crate compilation**
- **Found during:** Task 1 (building recovery module)
- **Issue:** lifecycle.rs used `wmux::` paths which fail when compiled as part of the library crate (previously only compiled in binary crate)
- **Fix:** Moved daemon and paths module declarations from binary to library crate, updated lifecycle.rs to use `crate::` paths, updated main.rs to use `wmux::daemon` and `wmux::paths`
- **Files modified:** src/lib.rs, src/main.rs, src/daemon/lifecycle.rs
- **Verification:** cargo build and cargo test both pass
- **Committed in:** ac61c19 (Task 1 commit)

**2. [Rule 1 - Bug] Test isolation fix for parallel test execution**
- **Found during:** Task 1 (running recovery tests)
- **Issue:** All tests used the same temp directory (based on PID), causing interference when run in parallel
- **Fix:** Each test gets a unique directory name with test-specific suffix
- **Files modified:** tests/recovery_test.rs
- **Verification:** All 5 tests pass reliably in parallel
- **Committed in:** ac61c19 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for correct compilation and test reliability. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Daemon foundation complete: lifecycle, IPC, session management, and crash recovery all operational
- Sessions survive daemon restarts with full ID/name preservation
- Ready for Phase 2: terminal I/O streaming (attach/detach) and session management enhancements

## Self-Check: PASSED

- All 8 source files verified present
- Commit ac61c19 (Task 1) verified in git log
- Commit 5a29610 (Task 2) verified in git log
- End-to-end verified: daemon-start -> new (x2) -> force-kill -> daemon-start -> ls (2 sessions recovered) -> kill-server
- Corrupted state.json handled gracefully (daemon starts with 0 sessions)
- All 13 tests pass (8 IPC + 5 recovery)

---
*Phase: 01-daemon-foundation*
*Completed: 2026-03-28*
