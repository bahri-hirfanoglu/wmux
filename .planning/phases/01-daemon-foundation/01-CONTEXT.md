# Phase 1: Daemon Foundation - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Background daemon process with Named Pipe IPC and ConPTY shell spawning. Single self-contained binary that can run as both client and daemon. Crash recovery restores sessions after unexpected shutdown. This phase establishes the engine — no session management UI or pane splitting yet.

</domain>

<decisions>
## Implementation Decisions

### Daemon Lifecycle
- v1 runs as a normal background process (not a Windows Service). Windows Service support deferred to later.
- On-demand startup: daemon starts automatically when any wmux command needs it. Optional auto-start on login via config.
- Shutdown via explicit `wmux kill-server` only — daemon does NOT auto-exit when sessions end.
- Kill-server with active sessions shows warning + confirmation prompt ("3 active sessions. Kill anyway? [y/N]").
- `wmux status` command shows daemon health: uptime, session count, resource usage.
- PID file at `%LOCALAPPDATA%\wmux\wmux.pid` (Windows convention).
- Log file at `%LOCALAPPDATA%\wmux\wmux.log`.
- Single-user design for v1 — no multi-user isolation.
- If daemon already running, new instance connects to existing (no error, no duplicate).

### IPC Protocol
- JSON message format over Named Pipes — human-readable, debug-friendly, serde_json.
- Length-prefixed framing: 4-byte message length + JSON payload. No newline-delimited.
- Bidirectional streaming: both client and daemon can send messages at any time (needed for terminal I/O).
- Per-session pipes: `\\.\pipe\wmux-{session_id}` for each session's terminal I/O.
- Daemon control pipe: `\\.\pipe\wmux-ctl` for session creation, listing, kill-server, and other management commands.

### Crash Recovery
- Session state persisted to `%LOCALAPPDATA%\wmux\state.json`.
- State written to disk on every structural change (session create/kill, pane split/close) — no periodic timer.
- Recovery scope: session metadata + child process re-adoption. Scrollback buffer is NOT persisted (lost on crash).
- Orphan child processes are adopted back by checking stored PIDs on daemon restart. If process still alive, re-attach; if dead, restart shell in that session.

### CLI-Daemon Relationship
- Single binary: `wmux` acts as both client and daemon. `wmux daemon start` launches daemon mode.
- Client auto-starts daemon if not running — transparent to user (tmux behavior).
- Flat subcommand structure: `wmux new`, `wmux ls`, `wmux attach`, `wmux detach`, `wmux kill-server`, `wmux status`, `wmux kill-session`, `wmux kill-pane`, `wmux split`.
- CLI parsing with `clap` (derive macros).

### Claude's Discretion
- Exact async runtime choice (tokio vs async-std)
- Internal daemon architecture (actor model, event loop, etc.)
- State.json schema design
- Error message wording and formatting
- Named Pipe security attributes

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project

### Established Patterns
- None — first phase establishes patterns

### Integration Points
- ConPTY: CreatePseudoConsole(), ResizePseudoConsole() via windows-rs
- Named Pipes: CreateNamedPipeW(), ConnectNamedPipe() via windows-rs
- Process management: CreateProcessW() via windows-rs
- CLI: clap derive macros for argument parsing

</code_context>

<specifics>
## Specific Ideas

- tmux-like behavior is the reference model: on-demand daemon, transparent startup, flat commands
- %LOCALAPPDATA% for all runtime data (Windows convention over XDG)
- JSON over Named Pipes — prioritize debuggability over raw performance at this stage

</specifics>

<deferred>
## Deferred Ideas

- Windows Service mode — after v1 background process proves stable
- Multi-user daemon isolation — if demand emerges
- Scrollback persistence across crashes — complex, evaluate after basic recovery works

</deferred>

---

*Phase: 01-daemon-foundation*
*Context gathered: 2026-03-28*
