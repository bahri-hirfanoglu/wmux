# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-28)

**Core value:** Terminal sessions survive disconnection — users can detach, close their terminal, and reattach without losing state or processes.
**Current focus:** Phase 1 — Daemon Foundation

## Current Position

Phase: 1 of 3 (Daemon Foundation)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-03-28 — Roadmap created, ready to plan Phase 1

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: -

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Init: Rust + windows-rs for memory safety and native Windows API access
- Init: Delegate rendering to Windows Terminal — no custom TUI
- Init: Named Pipes (\\.\pipe\wmux-*) for IPC — native Windows async IPC
- Init: tmux-style daemon (true process persistence, not layout restore)
- Init: Local-only v1 — remote attach deferred to v2

### Pending Todos

None yet.

### Blockers/Concerns

- UAC/permissions: Daemon running as background process may require elevated privileges or Windows Service setup — needs investigation in Phase 1
- ConPTY API surface: CreatePseudoConsole() / ResizePseudoConsole() integration complexity unknown until Phase 1

## Session Continuity

Last session: 2026-03-28
Stopped at: Roadmap created, STATE.md initialized
Resume file: None
