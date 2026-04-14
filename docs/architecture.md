# Architecture Notes

## Priorities

1. Session isolation under concurrent requests
2. Safe state transitions
3. Recoverable local runtime behavior
4. Slack-first UX in v1 without coupling the core model to Slack
5. Platform-specific UX without platform-specific core logic

## Core rule

Only the session actor may mutate session state.

All inbound actions are normalized into messages:
- user command
- approval decision
- runtime event
- interrupt
- recovery trigger

These are delivered to a per-session mailbox and handled sequentially.

## Canonical model

- `ProjectId`
- `SessionId`
- `TurnId`
- `TransportBinding`
- `RuntimeHandleId`

Transport-specific concepts such as Slack thread timestamps stay outside the core model and are mapped through bindings.

## Product stance

- public product: `Remote Claude Code`
- first transport: Slack
- internal core model: transport-neutral
- concurrency target: multiple project channels and multiple session spaces without state corruption

## Initial crate boundaries

- `core-model`: ids, enums, commands, events
- `core-service`: session actor and orchestration traits
- `policy`: risk classification and approval requirements
- `runtime-local`: tmux and local process integration
- `session-store`: persistence and recovery
- `transport-slack`: Slack event adapter and message rendering
- `app`: runtime bootstrap and dependency wiring
