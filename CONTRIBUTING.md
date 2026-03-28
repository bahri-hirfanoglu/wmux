# Contributing to wmux

Thank you for your interest in contributing to wmux! This document provides guidelines and instructions for contributing.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```
   git clone https://github.com/<your-username>/wmux.git
   cd wmux
   ```
3. **Create a branch** for your changes:
   ```
   git checkout -b my-feature
   ```

## Building from Source

wmux requires:

- **Windows 10 version 1809+** or **Windows 11**
- **Rust 1.70+** (install via [rustup](https://rustup.rs/))

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run the binary
cargo run -- <args>
```

## Code Style

- Run `cargo fmt` before committing to ensure consistent formatting
- Run `cargo clippy` and address any warnings
- Follow existing code conventions in the project

```bash
cargo fmt --check   # check formatting without modifying files
cargo clippy        # lint the codebase
```

## Making Changes

1. Make your changes in small, focused commits
2. Write clear commit messages that explain *why* the change was made
3. Add tests for new functionality where applicable
4. Make sure all existing tests pass with `cargo test`
5. Ensure `cargo clippy` produces no warnings

## Pull Requests

1. Push your branch to your fork:
   ```
   git push origin my-feature
   ```
2. Open a Pull Request against the `main` branch
3. Fill in the PR description with:
   - What the change does
   - Why it is needed
   - How it was tested
4. Wait for review — maintainers may request changes

### PR Guidelines

- Keep PRs focused on a single change
- Rebase on `main` if your branch falls behind
- Avoid unrelated formatting or refactoring changes in the same PR

## Reporting Issues

When filing an issue, please include:

- **Windows version** (e.g., Windows 11 23H2)
- **Terminal** being used (e.g., Windows Terminal 1.19)
- **wmux version** (`wmux --version`)
- **Steps to reproduce** the issue
- **Expected behavior** vs. **actual behavior**
- Any relevant **error messages** or **log output**

Debug logs can be enabled by setting the environment variable:

```
set RUST_LOG=debug
wmux daemon-start
```

## License

By contributing to wmux, you agree that your contributions will be licensed under the same dual license as the project: MIT OR Apache-2.0.
