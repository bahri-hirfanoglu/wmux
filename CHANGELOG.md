# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-06-01

### Added

- Background daemon for managing terminal sessions
- Session creation, listing, attaching, and detaching
- Horizontal and vertical pane splitting via Windows Terminal
- Pane navigation with `Ctrl+B` + Arrow keys
- Pane resizing with `Ctrl+B` + Alt+Arrow keys
- Kill pane (`Ctrl+B`, `x`) and kill session commands
- Auto-start daemon when running `wmux new` or `wmux attach`
- TOML configuration file support (`%APPDATA%\wmux\config.toml`)
- Configurable default shell
- ConPTY-based pseudo-terminal support
- Named Pipe IPC between client and daemon
- Status bar showing session info and keybinding hints

[0.1.0]: https://github.com/bahri-hirfanoglu/wmux/releases/tag/v0.1.0
