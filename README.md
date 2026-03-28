<p align="center">
  <br>
  <code>&nbsp;╦ ╦╔╦╗╦ ╦═╗ ╦&nbsp;</code><br>
  <code>&nbsp;║║║║║║║ ║╔╩╦╝&nbsp;</code><br>
  <code>&nbsp;╚╩╝╩ ╩╚═╝╩ ╚═&nbsp;</code><br>
  <br>
  <strong>A native Windows terminal multiplexer — tmux for Windows.</strong>
  <br><br>
  <a href="https://github.com/bahri-hirfanoglu/wmux/actions"><img src="https://github.com/bahri-hirfanoglu/wmux/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/wmux"><img src="https://img.shields.io/crates/v/wmux.svg" alt="crates.io"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
  <a href="https://github.com/bahri-hirfanoglu/wmux/releases"><img src="https://img.shields.io/github/v/release/bahri-hirfanoglu/wmux?include_prereleases" alt="GitHub Release"></a>
</p>

---

wmux lets you create persistent terminal sessions on Windows that survive terminal closes. Detach from a session, close your terminal, and reattach later — your processes keep running.

Built in Rust, wmux uses **ConPTY** for native pseudo-terminal support and **Named Pipes** for IPC, with a background daemon that manages all sessions.

## Features

- **Persistent sessions** — shell processes survive terminal closes
- **Detach / reattach** — disconnect and reconnect at any time
- **Split panes** — horizontal and vertical splits via Windows Terminal
- **Pane navigation** — switch between panes with keyboard shortcuts
- **Pane resizing** — resize panes with Alt+Arrow keys
- **tmux-style keybindings** — `Ctrl+B` prefix, familiar to tmux users
- **Background daemon** — lightweight process manages all sessions
- **Auto-start daemon** — daemon starts automatically when needed
- **TOML configuration** — customize shell and behavior
- **Native Windows** — no WSL, no Cygwin, just Windows

## Quick Start

```
wmux new            # create a new session (auto-starts daemon)
wmux attach         # attach to the most recent session
# Ctrl+B, d        # detach from the session
wmux ls             # list all sessions
```

## Installation

### From crates.io

```
cargo install wmux
```

### From source

```
git clone https://github.com/bahri-hirfanoglu/wmux.git
cd wmux
cargo build --release
```

The binary will be at `target/release/wmux.exe`. Add it to your `PATH` or copy it to a directory that is already in your `PATH`.

## Usage

### Session management

```bash
# Create a new session (daemon starts automatically if not running)
wmux new

# List all active sessions
wmux ls

# Attach to the most recent session
wmux attach

# Attach to a specific session by ID
wmux attach <session-id>

# Kill a specific session
wmux kill-session <session-id>
```

### Pane management

```bash
# Split the current pane horizontally (top/bottom)
wmux split -H

# Split the current pane vertically (left/right)
wmux split -v

# Kill the current pane
wmux kill-pane

# Kill a specific pane by ID
wmux kill-pane --pane-id <id>
```

### Daemon management

```bash
# Manually start the daemon
wmux daemon-start

# Check daemon and session status
wmux status

# Stop the daemon and all sessions
wmux kill-server
```

## Key Bindings

All key bindings use the `Ctrl+B` prefix (press `Ctrl+B`, release, then press the action key).

| Key | Action |
|-----|--------|
| `Ctrl+B`, `d` | Detach from the current session |
| `Ctrl+B`, `"` | Split pane horizontally (top/bottom) |
| `Ctrl+B`, `%` | Split pane vertically (left/right) |
| `Ctrl+B`, `x` | Kill the current pane |
| `Ctrl+B`, `Arrow` | Navigate between panes |
| `Ctrl+B`, `Alt+Arrow` | Resize the current pane |

## Configuration

wmux reads its configuration from:

```
%APPDATA%\wmux\config.toml
```

### Example config

```toml
# Override the default shell
default_shell = "pwsh.exe"

# Other examples:
# default_shell = "cmd.exe"
# default_shell = "C:\\Program Files\\Git\\bin\\bash.exe"
```

If no config file exists, wmux uses sensible defaults.

## Architecture

```
┌──────────┐    Named Pipe     ┌──────────────┐    ConPTY     ┌───────────┐
│  wmux    │◄─────────────────►│  wmux daemon │◄─────────────►│  Shell    │
│  client  │   (JSON/IPC)      │  (background)│  (pseudo-tty) │  process  │
└──────────┘                   └──────────────┘               └───────────┘
```

- **Client** (`wmux` CLI) — sends commands to the daemon via Named Pipes
- **Daemon** — long-running background process that manages sessions and panes, communicates with shell processes through ConPTY
- **ConPTY** — Windows pseudo-terminal API (available since Windows 10 1809) that provides proper terminal emulation
- **Named Pipes** — Windows IPC mechanism used for communication between client and daemon, with length-prefixed JSON messages

## Requirements

- **Windows 10 version 1809+** or **Windows 11** (for ConPTY support)
- **Windows Terminal** (required for split pane functionality)
- **Rust 1.70+** (to build from source)

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

## Author

[Bahri Hirfanoglu](https://github.com/bahri-hirfanoglu)
