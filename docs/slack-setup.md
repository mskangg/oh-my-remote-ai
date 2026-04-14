# Slack Setup

This project uses a bring-your-own Slack app model for v1.

The goal is not to make users click through Slack settings manually. The setup flow should stay as close as possible to:

1. Run `rcc setup slack`
2. Create a Slack app from the bundled manifest
3. Paste the generated secrets back into the setup wizard
4. Run `rcc doctor`

## Manifest-first setup

The bundled manifest lives at:

`slack/app-manifest.json`

The bundled scopes include `chat:write.public` so `/cc` can create the session root message in mapped public channels. Private channels still require inviting the bot before testing.

The setup command should point the user to Slack's "Create app from manifest" flow and print the manifest path clearly.

## Setup wizard contract

`rcc setup slack` should:

1. Print the product summary in one sentence
2. Explain that the user will create a Slack app in their own workspace
3. Open or print the Slack manifest creation URL
4. Point to `slack/app-manifest.json`
5. Prompt for:
   - `SLACK_BOT_TOKEN`
   - `SLACK_SIGNING_SECRET`
   - `SLACK_APP_TOKEN`
   - `SLACK_ALLOWED_USER_ID`
6. Write a local env file without printing secret values
7. Create a channel-project mapping template if missing
8. Run health checks

## Doctor checks

`rcc doctor` should verify:

1. required Slack env vars are present
2. the bot token is valid via Slack auth test
3. Socket Mode app token is present
4. `tmux` is installed
5. the configured state database path is writable
6. the Slack manifest file exists
7. the channel-project mapping file exists

## UX rules

- never print secrets back to the terminal
- always say exactly which step the user is on
- keep setup copy short enough for a screenshot or demo GIF
- if setup fails, show the next corrective action, not a stack trace dump

## Product note

The current Slack transport is Slack-first, but the core runtime must stay transport-neutral.
