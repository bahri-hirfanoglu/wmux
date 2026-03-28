# Phase 2: Session and Pane Core - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Full session lifecycle (create, list, attach, detach, kill) and pane management (split, navigate, resize, close) through the wmux CLI and Windows Terminal. Users can detach and reattach to sessions with running processes. Multiple panes per session via WT's pane system.

</domain>

<decisions>
## Implementation Decisions

### Attach/Detach Mechanics
- `wmux attach` opens the session in a Windows Terminal pane — WT renders the terminal output, not a custom TUI.
- Multiple clients can attach to the same session simultaneously (shared view) — all see the same output, all can send input (tmux behavior).
- Detach triggered via keybinding only (Prefix + d).
- Terminal window close (X button) = auto-detach — session processes continue running in daemon.

### Pane Layout Engine
- wmux ONLY works inside Windows Terminal. Running outside WT (cmd.exe, ConHost) should error: "wmux requires Windows Terminal".
- Pane state delegated entirely to Windows Terminal — WT manages its own layout, wmux sends commands.
- Pane control via `wt.exe` CLI commands (`wt.exe split-pane`, etc.) — wmux spawns `wt.exe` subprocesses.
- Pane addressing is index-based (0, 1, 2...) by creation order.

### Keybinding System
- Prefix key: Ctrl+B (tmux default).
- Detach: Prefix + d.
- Split horizontal: Prefix + " (tmux convention).
- Split vertical: Prefix + % (tmux convention).
- Pane navigation: Prefix + arrow keys (←↑→↓).
- Pane resize: Prefix + Alt+arrow keys.
- Kill pane: Prefix + x (with confirmation, tmux convention).
- Scroll mode: Prefix + [ (enter), q (exit) — tmux copy-mode style.

### Scrollback Buffer
- 10,000 lines per pane (default, configurable in future).
- RAM-based ring buffer — simple, fast, lost on crash (acceptable per Phase 1 decision).
- Scroll navigation: keybinding (Prefix + [) AND mouse wheel both work.

### Claude's Discretion
- Exact `wt.exe` CLI command construction and error handling
- Ring buffer implementation details
- How to intercept Ctrl+B in the terminal input stream
- Session pipe data streaming architecture for multi-client broadcast
- Pane index tracking when panes are closed mid-session

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ConPtySession` (src/session/conpty.rs): CreatePseudoConsole wrapper with cols/rows/shell/resize/kill — each pane needs its own instance
- `SessionManager` (src/session/manager.rs): Session CRUD — needs extension for pane management within sessions
- `Request/Response` enums (src/ipc/protocol.rs): IPC messages — need new variants for attach/detach/split/navigate/resize
- `ControlServer` (src/ipc/server.rs): Handles IPC commands — needs new command handlers
- `paths::control_pipe()` + per-session pipe naming — session I/O streaming infrastructure

### Established Patterns
- Tokio async runtime with Named Pipe server
- Arc<Mutex<SessionManager>> for shared state across IPC connections
- serde_json for Request/Response serialization
- Length-prefixed framing for IPC messages
- clap derive macros for CLI subcommands

### Integration Points
- `Session` struct needs a `Vec<Pane>` or pane tree instead of single `ConPtySession`
- `Request` enum needs: Attach, Detach, Split, Navigate, Resize, KillPane, ScrollBack variants
- `Response` enum needs: terminal output streaming, attach confirmation, pane list
- `cli.rs` needs: attach, detach, split, kill-pane subcommands with arguments
- Per-session Named Pipe needs to support bidirectional terminal I/O streaming

</code_context>

<specifics>
## Specific Ideas

- tmux keybinding conventions are the reference model (Ctrl+B prefix, ", %, d, arrow keys, [)
- Windows Terminal is a hard dependency — wmux is a WT-native tool, not a generic terminal multiplexer
- wt.exe CLI is the control interface — no WT API/COM/extension integration, just process spawning

</specifics>

<deferred>
## Deferred Ideas

- Custom keybinding configuration — Phase 3 config scope
- Read-only attach mode — could be useful for demos but not v1 essential
- Vim-style pane navigation (h/j/k/l) — add as configurable alternative later
- Direction-based pane addressing — complement index-based in future

</deferred>

---

*Phase: 02-session-and-pane-core*
*Context gathered: 2026-03-28*
