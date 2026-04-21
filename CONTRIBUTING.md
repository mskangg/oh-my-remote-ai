# Contributing to oh-my-remote-ai

Thank you for your interest in contributing!

## Before You Start

- Open an issue first for anything beyond small typo fixes.
- Check existing issues and PRs to avoid duplicating work.

## Development Setup

```bash
git clone https://github.com/mskangg/remote-claude-code.git
cd remote-claude-code

# Copy env template and fill in your Slack credentials
cp .env.local.example .env.local   # if it exists, otherwise see docs/slack-setup.md

# Build
cargo build

# Run tests
cargo test --workspace

# Lint
cargo clippy --all-targets --all-features
```

## Crate Structure

| Crate | Role |
|---|---|
| `crates/app` | Binary entrypoint, CLI, bootstrap wiring |
| `crates/application` | Use-cases, Slack UX rules, orchestration |
| `crates/transport-slack` | Socket Mode WebSocket, Slack API adapter |
| `crates/runtime-local` | tmux session management, hook file polling |
| `crates/session-store` | SQLite persistence |
| `crates/core-service` | Session actor, state machine |
| `crates/core-model` | Domain identifiers and message types |

Dependency rules: `app` → `application` → `transport-slack` / `runtime-local` / `session-store` → `core-service` → `core-model`. Product policy lives in `application`; infrastructure crates carry no business logic.

## Coding Standards

- New features and bug fixes must include tests.
- Run `cargo clippy --all-targets --all-features` — zero warnings expected.
- Match the existing code style; `cargo fmt` is the formatter.
- Prefer `thiserror` for library errors; `anyhow` is for the binary only.
- No `eprintln!` in library crates — use `tracing::*` instead.

## Pull Request Guidelines

1. Keep PRs focused. One concern per PR.
2. Write a clear description explaining *why*, not just *what*.
3. All CI checks must pass.
4. PRs without tests for new behaviour will not be merged.

## Reporting Bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md) and include:
- OS and `rcc --version`
- Steps to reproduce
- Expected vs actual behaviour
- Relevant logs (`RUST_LOG=debug rcc`)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
