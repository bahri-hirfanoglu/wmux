# wmux

## What This Is

A native Windows terminal multiplexer built in Rust. wmux adds tmux-style session management, persistence, and detach/attach capabilities on top of Windows Terminal's pane system using ConPTY. It targets developers and DevOps/SysAdmin professionals who need persistent terminal sessions on Windows.

## Core Value

Terminal sessions survive disconnection — users can detach, close their terminal, and reattach to running sessions without losing state or processes.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Session daemon runs in background, keeps processes alive after terminal closes
- [ ] Users can create, list, attach, and detach sessions via CLI
- [ ] Multiple panes per session with horizontal/vertical splits via Windows Terminal Pane API
- [ ] Each pane runs an independent shell process through ConPTY
- [ ] Named Pipes IPC between client and daemon
- [ ] TOML-based configuration (keybindings, default shell, layout)
- [ ] Customizable keybinding system
- [ ] Scrollback buffer per pane
- [ ] Crash recovery — daemon restores sessions after unexpected shutdown
- [ ] Distribution via cargo install, winget, scoop, and GitHub releases

### Out of Scope

- WSL deep integration — v2, shell spawning via wsl.exe sufficient for now
- Remote session attach (SSH) — v2
- Custom TUI renderer — using Windows Terminal's rendering
- Mobile/web client — CLI only
- Plugin/extension system — premature complexity
- Video/media terminal support — text-only

## Context

- Windows has no native tmux equivalent. WSL users can use tmux inside WSL, but native Windows shells (PowerShell, CMD, Git Bash) have no multiplexer option.
- Microsoft's ConPTY (Pseudo Console) API provides the foundation — CreatePseudoConsole() for PTY creation, ResizePseudoConsole() for resize events.
- Windows Terminal has a Tab/Pane API and extension points that wmux can leverage instead of building its own renderer.
- The `windows-rs` crate (officially maintained by Microsoft) provides idiomatic Rust bindings for all Windows APIs needed.
- Named Pipes (\\.\pipe\wmux-*) are the standard Windows IPC mechanism and support async I/O.
- Session persistence requires running as a Windows Service or background process — UAC permission management is critical.

## Constraints

- **Tech stack**: Rust + windows-rs — non-negotiable, chosen for memory safety and native Windows API access
- **Platform**: Windows 10+ only (ConPTY requires Windows 10 1809+)
- **Rendering**: Delegates to Windows Terminal — wmux does not implement its own terminal emulator
- **License**: MIT + Apache 2.0 dual license (Rust ecosystem standard)
- **Config format**: TOML

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust over C++ | Memory safety, windows-rs official support, modern tooling | — Pending |
| Windows Terminal Pane API over custom TUI | Leverage existing renderer, reduce scope, better UX integration | — Pending |
| Named Pipes for IPC | Native Windows IPC, async support, per-session addressing | — Pending |
| tmux-style daemon over snapshot restore | True persistence — processes stay alive, not just layout restore | — Pending |
| Local-only v1 | Reduce complexity, remote attach deferred to v2 | — Pending |
| TOML config | Rust ecosystem standard, human-readable, Cargo.toml consistency | — Pending |

---
*Last updated: 2026-03-28 after initialization*
