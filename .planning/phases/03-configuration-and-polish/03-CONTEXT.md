# Phase 3: Configuration and Polish - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

TOML configuration file support and CLI polish. Users can configure their default shell. All commands expose accurate help text and exit with appropriate error codes. This is the final phase before v1 release.

</domain>

<decisions>
## Implementation Decisions

### Config File
- Config location: `%APPDATA%\wmux\config.toml` (Windows convention — separate from runtime data at `%LOCALAPPDATA%`).
- v1 config contains ONLY `default_shell` setting. Other settings (scrollback_lines, prefix_key, auto_start_daemon) deferred to v2.
- If config file doesn't exist, wmux silently uses defaults — no warning, no error, no auto-creation.
- Config is read once at daemon startup. Changes require daemon restart.

### CLI Polish
- All subcommands expose accurate `--help` text via clap derive macros (already in place from Phase 1).
- Exit codes: 0 for success, 1 for general error, 2 for usage error (clap default behavior).
- Error messages should be actionable — tell the user what went wrong and how to fix it.

### Claude's Discretion
- TOML parsing library choice (toml crate is standard)
- Default shell detection logic (check COMSPEC, fall back to powershell)
- Exact help text wording
- Error message formatting (colored vs plain)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `paths::wmux_data_dir()` (src/paths.rs) — resolves %LOCALAPPDATA%\wmux, need analogous `wmux_config_dir()` for %APPDATA%\wmux
- `Cli` struct with clap derive (src/cli.rs) — already has subcommands, help text via clap
- `ConPtySession::new(cols, rows, shell)` (src/session/conpty.rs) — already accepts optional shell parameter

### Established Patterns
- Centralized path resolution in `paths.rs`
- clap derive for CLI parsing
- `anyhow::Result` for error propagation
- serde for serialization (JSON currently, TOML will follow same pattern)

### Integration Points
- `paths.rs` — add `config_file()` function returning `%APPDATA%\wmux\config.toml`
- `ConPtySession::new()` — pass configured default_shell instead of hardcoded default
- `daemon/lifecycle.rs` — load config at daemon startup, pass to SessionManager
- `session/manager.rs` — use configured shell when creating new sessions

</code_context>

<specifics>
## Specific Ideas

- Minimal v1 config — just default_shell. Keep it simple, expand later.
- %APPDATA% for config, %LOCALAPPDATA% for runtime — standard Windows separation.

</specifics>

<deferred>
## Deferred Ideas

- scrollback_lines config option — v2
- prefix_key customization — v2
- auto_start_daemon on login — v2
- Full keybinding customization — v2
- Theme/color configuration — v2

</deferred>

---

*Phase: 03-configuration-and-polish*
*Context gathered: 2026-03-28*
