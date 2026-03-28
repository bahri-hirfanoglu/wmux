# Roadmap: wmux

## Overview

wmux is built in three phases. Phase 1 establishes the daemon process and IPC foundation — the engine that keeps sessions alive. Phase 2 delivers the full session and pane experience through the CLI, leveraging Windows Terminal and ConPTY. Phase 3 finishes the product with TOML configuration and CLI polish so it can ship to real users.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Daemon Foundation** - Background daemon process with Named Pipe IPC and ConPTY scaffolding
- [ ] **Phase 2: Session and Pane Core** - Full session lifecycle and pane management via CLI and Windows Terminal
- [ ] **Phase 3: Configuration and Polish** - TOML config, CLI help/errors, and distribution readiness

## Phase Details

### Phase 1: Daemon Foundation
**Goal**: A background daemon process is running, accepting client connections via Named Pipes, and can spawn shell processes through ConPTY
**Depends on**: Nothing (first phase)
**Requirements**: DAEMON-01, DAEMON-02, DAEMON-03, DAEMON-04, INTG-03, CLI-01
**Success Criteria** (what must be TRUE):
  1. `wmux-daemon` starts as a background process and remains alive after the launching terminal is closed
  2. A client process can connect to the daemon via a Named Pipe and exchange messages
  3. The daemon can spawn and manage a shell process through ConPTY on Windows 10 1809+
  4. The daemon recovers its state and restarts child processes after an unexpected crash or restart
  5. `wmux` binary exists as a single self-contained executable that can reach the daemon
**Plans:** 3/4 plans executed

Plans:
- [ ] 01-01-PLAN.md — Project scaffold, CLI, and daemon lifecycle (start/stop/status)
- [ ] 01-02-PLAN.md — Named Pipe IPC protocol and client-daemon communication
- [ ] 01-03-PLAN.md — ConPTY shell spawning and session management
- [ ] 01-04-PLAN.md — Crash recovery with state persistence

### Phase 2: Session and Pane Core
**Goal**: Users can create, manage, and persist terminal sessions with multiple panes through the wmux CLI and Windows Terminal
**Depends on**: Phase 1
**Requirements**: SESS-01, SESS-02, SESS-03, SESS-04, SESS-05, SESS-06, PANE-01, PANE-02, PANE-03, PANE-04, PANE-05, PANE-06, PANE-07, INTG-01, INTG-02
**Success Criteria** (what must be TRUE):
  1. User can create a session with `wmux new`, list sessions with `wmux ls`, attach with `wmux attach`, detach with `wmux detach`, and kill with `wmux kill-session`
  2. After detaching or closing the terminal, the session's processes continue running in the daemon and can be reattached
  3. User can split a pane horizontally with `wmux split -h` and vertically with `wmux split -v`, producing independent shell processes in each pane via ConPTY
  4. User can navigate between panes and resize them via keybindings, and close a pane with `wmux kill-pane`
  5. User can scroll back through pane output history
**Plans**: TBD

### Phase 3: Configuration and Polish
**Goal**: wmux is configurable via TOML and presents a professional CLI surface ready for public distribution
**Depends on**: Phase 2
**Requirements**: CONF-01, CONF-02, CLI-02, CLI-03
**Success Criteria** (what must be TRUE):
  1. User can create `~/.config/wmux/config.toml` to configure their default shell and wmux reads it on startup
  2. `wmux --help` and subcommand help flags display accurate usage information for all commands
  3. CLI exits with non-zero codes on errors and prints actionable error messages
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Daemon Foundation | 3/4 | In Progress|  |
| 2. Session and Pane Core | 0/TBD | Not started | - |
| 3. Configuration and Polish | 0/TBD | Not started | - |
