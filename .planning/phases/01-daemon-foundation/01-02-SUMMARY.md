---
phase: 01-daemon-foundation
plan: 02
subsystem: ipc
tags: [rust, tokio, named-pipes, serde-json, ipc, windows]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Daemon lifecycle, CLI dispatch, paths module"
provides:
  - "Bidirectional JSON-over-Named-Pipe IPC (client/server)"
  - "Length-prefixed framing protocol (4-byte LE + JSON)"
  - "Request/Response message types for all daemon commands"
  - "ControlServer with async connection handling and shutdown channel"
  - "Client send_request with retry and PID-file fallback"
affects: [01-03, 01-04, 02-session-management]

# Tech tracking
tech-stack:
  added: [tokio-named-pipes]
  patterns: [length-prefixed-framing, watch-channel-shutdown, ipc-with-pid-fallback]

key-files:
  created:
    - src/ipc/mod.rs
    - src/ipc/protocol.rs
    - src/ipc/server.rs
    - src/ipc/client.rs
    - src/lib.rs
    - tests/ipc_protocol_test.rs
  modified:
    - src/daemon/lifecycle.rs
    - src/main.rs

key-decisions:
  - "Used tokio::net::windows::named_pipe instead of raw windows-rs for async pipe I/O"
  - "Client retries pipe connection up to 5 times with 50ms delay for busy/not-ready pipes"
  - "IPC commands fall back to PID file check when pipe is not responding"

patterns-established:
  - "IPC pattern: length-prefixed JSON framing via read_message/write_message"
  - "Server pattern: watch channel for cooperative shutdown signaling"
  - "Client pattern: send_request with retry + PID-file fallback"

requirements-completed: [DAEMON-04]

# Metrics
duration: 3min
completed: 2026-03-28
---

# Phase 1 Plan 2: Named Pipe IPC Layer Summary

**Bidirectional JSON-over-Named-Pipe IPC with 4-byte length-prefixed framing using tokio async named pipes**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-28T14:58:55Z
- **Completed:** 2026-03-28T15:02:39Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Defined complete IPC protocol with Request/Response enums covering all daemon commands (Ping, Status, KillServer, session operations)
- Implemented Named Pipe server (ControlServer) with async connection handling, cooperative shutdown via watch channel
- Implemented client send_request with retry logic and PID-file fallback for robustness
- Wired status and kill-server CLI commands through IPC, replacing direct process manipulation
- 8 unit tests for protocol framing covering round-trips, multi-message, and incomplete data edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Define IPC message protocol with length-prefixed framing** - `5bb81ce` (feat)
2. **Task 2: Implement Named Pipe server and client, wire into daemon** - `cd22f6a` (feat)

## Files Created/Modified
- `src/ipc/mod.rs` - Module declarations for protocol, server, client
- `src/ipc/protocol.rs` - Request/Response enums, SessionInfo struct, read_message/write_message framing
- `src/ipc/server.rs` - ControlServer with Named Pipe listener, per-connection handler, shutdown support
- `src/ipc/client.rs` - send_request function with retry logic and error handling
- `src/lib.rs` - Library crate exposing ipc module for integration tests
- `tests/ipc_protocol_test.rs` - 8 tests for protocol serialization and framing
- `src/daemon/lifecycle.rs` - Integrated ControlServer, IPC-based status/kill-server with fallback
- `src/main.rs` - Updated status/kill-server dispatch to async IPC calls

## Decisions Made
- Used tokio's built-in `tokio::net::windows::named_pipe` for async pipe I/O instead of raw windows-rs overlapped I/O -- simpler, integrates with tokio runtime
- Added retry loop in client (5 attempts, 50ms delay) to handle pipe busy/not-ready race conditions
- IPC commands fall back to PID file when pipe is unresponsive, ensuring status/kill always work even if pipe is down
- Created lib.rs to expose ipc module publicly for integration test access

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Named Pipe IPC fully operational, ready for ConPTY integration (Plan 03)
- All future commands route through the established Request/Response protocol
- Session operation placeholders (NewSession, ListSessions, etc.) ready for Phase 2 implementation

## Self-Check: PASSED

- All 8 source/test files verified present
- Commit 5bb81ce (Task 1) verified in git log
- Commit cd22f6a (Task 2) verified in git log
- All 8 protocol tests pass
- End-to-end verified: daemon-start -> status (IPC) -> kill-server (IPC) -> status (not running)

---
*Phase: 01-daemon-foundation*
*Completed: 2026-03-28*
