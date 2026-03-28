---
phase: 02-session-and-pane-core
plan: 05
subsystem: session
tags: [scrollback, ring-buffer, scroll-mode, terminal-history, conpty]

# Dependency graph
requires:
  - phase: 02-session-and-pane-core
    plan: 02
    provides: Attach I/O streaming loop where scrollback capture happens
provides:
  - ScrollbackBuffer ring buffer (10k lines per pane)
  - Scrollback capture in ConPTY output streaming loop
  - Scroll mode with Prefix+[ entry, q exit, keyboard and mouse navigation
  - ScrollModeData IPC response for scroll content delivery
affects: [03-keybindings-and-ux]

# Tech tracking
tech-stack:
  added: []
  patterns: [ring-buffer-scrollback, scroll-mode-state-machine, build-scroll-response-sync-helper]

key-files:
  created:
    - src/session/scrollback.rs
    - tests/scrollback_test.rs
  modified:
    - src/session/mod.rs
    - src/session/pane.rs
    - src/ipc/server.rs
    - src/ipc/client.rs
    - src/ipc/protocol.rs

key-decisions:
  - "Ring buffer uses Vec<Vec<u8>> with head/count tracking for O(1) push and O(1) indexed access"
  - "push_bytes() buffers partial lines (no trailing newline) and prepends to next call"
  - "build_scroll_response() is a sync helper to avoid holding Pane iterator (contains HANDLE) across await points"
  - "Scroll mode renders 50 lines per page with ANSI clear-screen between scroll requests"

patterns-established:
  - "Sync helper pattern: extract data from !Send types in a non-async function, then use result across await"
  - "Scroll mode client state: dedicated input loop that intercepts all keys and does not forward to shell"

requirements-completed: [PANE-07]

# Metrics
duration: 8min
completed: 2026-03-28
---

# Phase 2 Plan 5: Scrollback Buffer and Scroll Mode Summary

**10k-line ring buffer per pane with Prefix+[ scroll mode supporting arrow keys, page up/down, vim keys, and mouse wheel**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-28T16:39:05Z
- **Completed:** 2026-03-28T16:47:00Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Implemented ScrollbackBuffer ring buffer with 10,000 line capacity, O(1) push/access, partial line buffering
- Integrated scrollback capture in the attach streaming loop (output captured before forwarding to client)
- Full scroll mode: Prefix+[ enters, q exits, arrow keys scroll line-by-line, Page Up/Down scroll by page, vim j/k/g/G, mouse wheel
- 9 unit tests covering capacity, wraparound, byte splitting, partial lines, edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ring buffer for scrollback** - `7504395` (feat)
2. **Task 2: Integrate scrollback capture and implement scroll mode** - `686131b` (feat)

## Files Created/Modified
- `src/session/scrollback.rs` - ScrollbackBuffer ring buffer with push_line, push_bytes, get_line, get_lines
- `src/session/mod.rs` - Added `pub mod scrollback` declaration
- `src/session/pane.rs` - Added scrollback field to Pane with accessor methods
- `src/ipc/server.rs` - Scrollback capture in attach loop, EnterScrollMode/ScrollBack/ExitScrollMode handlers, build_scroll_response helper
- `src/ipc/client.rs` - Full scroll mode implementation replacing stub (enter_scroll_mode, write_to_stdout, write_scroll_status helpers)
- `src/ipc/protocol.rs` - ScrollModeData response variant
- `tests/scrollback_test.rs` - 9 unit tests for ring buffer behavior

## Decisions Made
- Ring buffer uses Vec<Vec<u8>> with modular arithmetic for head/wrap tracking -- simple, efficient, no external dependencies
- push_bytes() splits on 0x0A newline boundaries and buffers incomplete lines internally for correct line reconstruction
- build_scroll_response() is a synchronous function that extracts all scroll data while the mutex is held, returning owned data -- avoids the !Send HANDLE-across-await problem
- Scroll mode sends 50 lines per request with ANSI escape clear-screen between pages for clean rendering

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Existing scroll mode stub in client.rs from Plan 02-03**
- **Found during:** Task 2
- **Issue:** Plan 02-03 already added `enter_scroll_mode` as a stub function and the Prefix+[ handler in the key dispatch. Plan 02-05 expected to add these from scratch.
- **Fix:** Replaced the stub implementation with the full scroll mode, no structural changes needed to the prefix key dispatcher.
- **Files modified:** src/ipc/client.rs
- **Verification:** cargo check passes, scroll mode functional
- **Committed in:** 686131b

**2. [Rule 3 - Blocking] HANDLE !Send in scroll mode server handlers**
- **Found during:** Task 2
- **Issue:** Inline scroll mode handlers in the attach loop held iterators over Pane (contains HANDLE) across await points, making the future !Send
- **Fix:** Extracted all scroll data access into build_scroll_response() sync helper that returns owned Response before any await
- **Files modified:** src/ipc/server.rs
- **Verification:** cargo check passes with no Send/Sync errors
- **Committed in:** 686131b

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for correctness with Rust's async Send requirements and existing codebase state. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Scrollback buffer and scroll mode complete -- Phase 2 fully done
- Phase 3 (keybindings and UX) can build on the scroll mode infrastructure for additional navigation features
- Prefix key state machine is extensible for future keybindings

## Self-Check: PASSED

- [x] src/session/scrollback.rs exists
- [x] tests/scrollback_test.rs exists
- [x] Commit 7504395 found
- [x] Commit 686131b found
- [x] 02-05-SUMMARY.md exists

---
*Phase: 02-session-and-pane-core*
*Completed: 2026-03-28*
