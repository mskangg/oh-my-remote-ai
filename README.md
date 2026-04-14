# Remote Claude Code

Rust prototype workspace for `remote-claude-code`.

Goals:
- keep the current TypeScript implementation untouched
- prototype a Rust runtime focused on session isolation and concurrency safety
- make it easy to move this directory into a separate public repository later

Current status:
- Rust prototype boots as its own Slack Socket Mode app
- per-session actor isolation, SQLite storage, tmux launch, and hook polling are wired
- Slack `/cc` session start, thread reply routing, working status, and final reply flow exist
- `rcc doctor` is available
- `rcc setup slack` is not implemented yet

Planned product shape:
- run Claude Code in the user's real environment
- continue work from anywhere through chat
- project space = workspace
- session space = agent session
- local-first runtime
- Slack-first in v1
- one active chat transport per deployment
- strong per-session isolation under concurrent requests

## Workspace layout

```text
.worktrees/remote-claude-code/
  Cargo.toml
  data/
  docs/
  slack/
  crates/
    app/
    core-model/
    core-service/
    policy/
    runtime-local/
    session-store/
    transport-slack/
```

## Positioning

- product name: `Remote Claude Code`
- repo name: `remote-claude-code`
- binary name: `rcc`
- tagline: `Run Claude Code in your real environment. Continue it from anywhere.`

## Why this lives under `.worktrees/`

- ignored by the current repository
- safe to iterate without polluting the TypeScript implementation
- easy to extract into a new public repository once the Rust design settles

## Manual Run

1. Copy [data/channel-projects.example.json](/Users/mskangg/Workspace/slack-remote/.worktrees/remote-claude-code/data/channel-projects.example.json:1) to `data/channel-projects.json` and replace the channel ID and project path.
2. Make sure `.env.local` exists in `.worktrees/remote-claude-code/`.
3. Run:

```bash
/opt/homebrew/opt/rustup/bin/cargo run -p rcc -- doctor
/opt/homebrew/opt/rustup/bin/cargo run -p rcc
```

4. In Slack, run `/cc` inside a mapped channel and continue inside the created thread.

The step-by-step smoke test lives in [docs/manual-smoke-test.md](/Users/mskangg/Workspace/slack-remote/.worktrees/remote-claude-code/docs/manual-smoke-test.md:1).

## Next steps

1. Implement `rcc setup slack`
2. Add a real end-to-end manual smoke pass against Slack
3. Add richer progress updates from hook events
4. Add approval flow
5. Improve startup and runtime diagnostics
