# Manual Smoke Test

Use this before claiming the Rust prototype is ready for real Slack use.

## Preconditions

1. `.env.local` exists in this workspace root.
2. `data/channel-projects.json` exists.
3. The mapped `projectRoot` is a real local directory.
4. `tmux` and `claude` are installed and available on `PATH`.
5. For public channels, reinstall the Slack app after manifest changes so `chat:write.public` is granted. For private channels, invite the bot to the channel first.

Start from the example file:

```bash
cp data/channel-projects.example.json data/channel-projects.json
```

Then replace:
- `channelId`
- `projectRoot`
- `projectLabel`

## Doctor

Run:

```bash
/opt/homebrew/opt/rustup/bin/cargo run -p rcc -- doctor
```

Expected:
- every line prints `[OK]`

If anything prints `[FAIL]`, fix that before running Slack.

## Slack Run

Run:

```bash
/opt/homebrew/opt/rustup/bin/cargo run -p rcc
```

Expected:
- process stays up
- no immediate startup error

## Session Start

In the mapped Slack channel:

1. Run `/cc`
2. Confirm a root message appears in the channel
3. Confirm a `Working...` message appears inside the new thread

Expected:
- a new SQLite state file appears under `.local/state.db`
- a new hook file appears under `.local/hooks/`
- a new tmux session exists

Useful checks:

```bash
tmux ls
ls .local/hooks
```

## Thread Reply

Reply inside the thread with a short prompt such as:

```text
say hello and stop
```

Expected:
- the working status remains associated with the same thread
- Claude runs in the mapped project directory
- when Claude stops, the status updates to `Ready for next prompt.`
- the final assistant reply is posted back into the thread

## Failure Path

Send a prompt that should fail fast or interrupt it manually.

Expected:
- the status updates to `Failed.` for a runtime failure
- a failure message is posted into the thread

## Concurrency Check

1. Start two sessions in two different mapped channels
2. Send prompts to both threads close together

Expected:
- separate tmux sessions
- separate hook files
- no cross-posting between threads
- final replies return to the correct owning thread
