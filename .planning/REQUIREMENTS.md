# Requirements: wmux

**Defined:** 2026-03-28
**Core Value:** Terminal sessions survive disconnection — users can detach, close their terminal, and reattach without losing state or processes.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Daemon

- [ ] **DAEMON-01**: wmux-daemon runs as a background process, independent of any terminal window
- [ ] **DAEMON-02**: Daemon manages all active sessions and their child processes
- [ ] **DAEMON-03**: Daemon recovers sessions after unexpected crash or restart
- [ ] **DAEMON-04**: Daemon communicates with clients via Named Pipes (\\.\pipe\wmux-*)

### Session

- [ ] **SESS-01**: User can create a new session with `wmux new`
- [ ] **SESS-02**: User can list all active sessions with `wmux ls`
- [ ] **SESS-03**: User can attach to an existing session with `wmux attach`
- [ ] **SESS-04**: User can detach from a session with `wmux detach` or keybinding
- [ ] **SESS-05**: User can kill a session with `wmux kill-session`
- [ ] **SESS-06**: Sessions persist after client disconnects — processes keep running in daemon

### Pane

- [ ] **PANE-01**: User can split current pane horizontally with `wmux split -h`
- [ ] **PANE-02**: User can split current pane vertically with `wmux split -v`
- [ ] **PANE-03**: User can navigate between panes with keybindings
- [ ] **PANE-04**: User can resize panes with keybindings
- [ ] **PANE-05**: Each pane runs an independent shell process via ConPTY
- [ ] **PANE-06**: User can close a pane with `wmux kill-pane`
- [ ] **PANE-07**: User can scroll back through pane output history

### Configuration

- [ ] **CONF-01**: User can configure wmux via TOML file (~/.config/wmux/config.toml)
- [ ] **CONF-02**: User can set default shell (PowerShell, CMD, Git Bash, etc.)

### CLI

- [ ] **CLI-01**: `wmux` binary is a single self-contained executable
- [ ] **CLI-02**: CLI provides help text and usage info for all commands
- [ ] **CLI-03**: CLI exits with appropriate error codes and messages

### Integration

- [ ] **INTG-01**: wmux leverages Windows Terminal Pane API for rendering
- [ ] **INTG-02**: Pane splits and layout are managed through WT's native pane system
- [ ] **INTG-03**: wmux works on Windows 10 1809+ (ConPTY requirement)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Session Enhancements

- **SESS-07**: User can name sessions (`wmux new -s dev`)
- **SESS-08**: User can rename sessions

### Pane Enhancements

- **PANE-08**: Predefined layout templates (e.g., main-horizontal, tiled)

### Configuration Enhancements

- **CONF-03**: User can customize keybindings
- **CONF-04**: User can customize status bar theme/colors

### Integration Enhancements

- **INTG-04**: WSL shell deep integration (beyond wsl.exe spawn)
- **INTG-05**: Remote session attach via SSH

### Distribution

- **DIST-01**: Available via cargo install
- **DIST-02**: Available via winget
- **DIST-03**: Available via scoop
- **DIST-04**: GitHub releases with pre-built binaries

## Out of Scope

| Feature | Reason |
|---------|--------|
| Custom TUI renderer | Using Windows Terminal's rendering — no need to reimplement |
| Plugin/extension system | Premature complexity for v1 |
| Mobile/web client | CLI-only tool |
| Video/media terminal | Text-only terminal multiplexer |
| Cross-platform (Linux/macOS) | tmux already exists there — wmux fills Windows gap |
| Real-time collaboration | Not a pairing tool |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| (populated during roadmap creation) | | |

**Coverage:**
- v1 requirements: 18 total
- Mapped to phases: 0
- Unmapped: 18 ⚠️

---
*Requirements defined: 2026-03-28*
*Last updated: 2026-03-28 after initial definition*
