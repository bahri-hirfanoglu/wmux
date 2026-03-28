# Requirements: wmux

**Defined:** 2026-03-28
**Core Value:** Terminal sessions survive disconnection — users can detach, close their terminal, and reattach without losing state or processes.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Daemon

- [x] **DAEMON-01**: wmux-daemon runs as a background process, independent of any terminal window
- [x] **DAEMON-02**: Daemon manages all active sessions and their child processes
- [x] **DAEMON-03**: Daemon recovers sessions after unexpected crash or restart
- [x] **DAEMON-04**: Daemon communicates with clients via Named Pipes (\\.\pipe\wmux-*)

### Session

- [x] **SESS-01**: User can create a new session with `wmux new`
- [x] **SESS-02**: User can list all active sessions with `wmux ls`
- [x] **SESS-03**: User can attach to an existing session with `wmux attach`
- [x] **SESS-04**: User can detach from a session with `wmux detach` or keybinding
- [x] **SESS-05**: User can kill a session with `wmux kill-session`
- [x] **SESS-06**: Sessions persist after client disconnects — processes keep running in daemon

### Pane

- [x] **PANE-01**: User can split current pane horizontally with `wmux split -h`
- [x] **PANE-02**: User can split current pane vertically with `wmux split -v`
- [x] **PANE-03**: User can navigate between panes with keybindings
- [x] **PANE-04**: User can resize panes with keybindings
- [x] **PANE-05**: Each pane runs an independent shell process via ConPTY
- [x] **PANE-06**: User can close a pane with `wmux kill-pane`
- [x] **PANE-07**: User can scroll back through pane output history

### Configuration

- [x] **CONF-01**: User can configure wmux via TOML file (~/.config/wmux/config.toml)
- [x] **CONF-02**: User can set default shell (PowerShell, CMD, Git Bash, etc.)

### CLI

- [x] **CLI-01**: `wmux` binary is a single self-contained executable
- [ ] **CLI-02**: CLI provides help text and usage info for all commands
- [ ] **CLI-03**: CLI exits with appropriate error codes and messages

### Integration

- [x] **INTG-01**: wmux leverages Windows Terminal Pane API for rendering
- [x] **INTG-02**: Pane splits and layout are managed through WT's native pane system
- [x] **INTG-03**: wmux works on Windows 10 1809+ (ConPTY requirement)

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
| DAEMON-01 | Phase 1 | Complete |
| DAEMON-02 | Phase 1 | Complete |
| DAEMON-03 | Phase 1 | Complete |
| DAEMON-04 | Phase 1 | Complete |
| CLI-01 | Phase 1 | Complete |
| INTG-03 | Phase 1 | Complete |
| SESS-01 | Phase 2 | Complete |
| SESS-02 | Phase 2 | Complete |
| SESS-03 | Phase 2 | Complete |
| SESS-04 | Phase 2 | Complete |
| SESS-05 | Phase 2 | Complete |
| SESS-06 | Phase 2 | Complete |
| PANE-01 | Phase 2 | Complete |
| PANE-02 | Phase 2 | Complete |
| PANE-03 | Phase 2 | Complete |
| PANE-04 | Phase 2 | Complete |
| PANE-05 | Phase 2 | Complete |
| PANE-06 | Phase 2 | Complete |
| PANE-07 | Phase 2 | Complete |
| INTG-01 | Phase 2 | Complete |
| INTG-02 | Phase 2 | Complete |
| CONF-01 | Phase 3 | Complete |
| CONF-02 | Phase 3 | Complete |
| CLI-02 | Phase 3 | Pending |
| CLI-03 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-28*
*Last updated: 2026-03-28 after roadmap creation*
