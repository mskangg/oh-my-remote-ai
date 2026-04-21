use core_model::{SessionId, SessionState, TransportBinding};

/// An inbound user message from a Slack thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackThreadReply {
    pub channel_id: String,
    pub thread_ts: String,
    pub text: String,
    pub user_id: String,
}

/// Parameters required to open a new agent session from a Slack thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackSessionStart {
    pub channel_id: String,
    pub thread_ts: String,
    /// Shell command used to launch the agent in the tmux session.
    pub launch_command: String,
}

/// A Slack channel mapped to a local project directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackProject {
    pub project_root: String,
    pub project_label: String,
}

/// A session entry returned by the session catalog, used to render the session list UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackListedSession {
    pub session_id: SessionId,
    pub tmux_session_name: String,
    pub thread_ts: String,
    pub project_label: String,
    pub state: SessionState,
}

/// The result of successfully starting a new session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedSlackSession {
    pub session_id: SessionId,
    pub state: SessionState,
    pub binding: TransportBinding,
}

/// Identifies the Slack channel + thread where messages should be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackMessageTarget {
    pub channel_id: String,
    pub thread_ts: String,
}

/// The Slack API response for a successfully posted message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackPostedMessage {
    pub channel_id: String,
    pub message_ts: String,
}

/// An action dispatched from a Slack thread button or slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackThreadAction {
    /// Open the command palette block in the thread.
    OpenCommandPalette,
    /// Send Ctrl-C to the running agent (interrupt current turn).
    Interrupt,
    /// Send an arbitrary key sequence to the tmux pane.
    SendKey { key: String },
    /// Send a text command as if the user typed it.
    SendCommand { text: String },
    /// Terminate the tmux session entirely.
    Terminate,
}

/// Identifies the live Slack status message for a thread (the "Working..." bubble).
///
/// Carries both the thread anchor (`thread_ts`) and the separate message that is
/// being updated/deleted as turns progress (`status_message_ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackThreadStatus {
    pub channel_id: String,
    pub thread_ts: String,
    pub status_message_ts: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlackEnvelope {
    pub(crate) channel: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) thread_ts: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) bot_id: Option<String>,
    pub(crate) subtype: Option<String>,
}
