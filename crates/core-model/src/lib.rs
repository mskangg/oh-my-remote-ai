//! Domain model for Remote Claude Code.
//!
//! Defines the core identifiers ([`SessionId`], [`TurnId`]), state machine
//! types ([`SessionState`]), and message envelope ([`SessionMsg`]) that all
//! other crates depend on.  This crate has no external runtime dependencies.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub Uuid);
impl ProjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);
impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub Uuid);
impl TurnId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for TurnId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransportBinding {
    pub project_space_id: String,
    pub session_space_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportStatusMessage {
    pub binding: TransportBinding,
    pub status_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Starting,
    Idle,
    Running { active_turn: TurnId },
    WaitingForApproval,
    Cancelling { active_turn: TurnId },
    Completed,
    Failed { reason: String },
}

impl SessionState {
    /// One-line label for the session list UI (e.g., "Ready for next prompt.").
    ///
    /// Callers must not pattern-match on `SessionState` just to get a display
    /// string — ask the state to describe itself instead (Tell Don't Ask).
    ///
    /// Note: The `application` crate uses a separate `INITIAL_THINKING_STATUS`
    /// constant for the per-turn live Slack status bubble.  The two strings
    /// happen to share the same "⏳ Working..." text today but are intentionally
    /// independent — they describe different UX surfaces and may diverge.
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Idle => "Ready for next prompt.",
            Self::Starting | Self::Running { .. } | Self::Cancelling { .. } => "⏳ Working...",
            Self::Completed => "Completed.",
            Self::Failed { .. } => "Failed.",
            Self::WaitingForApproval => "Waiting for approval.",
        }
    }

    /// `true` while the session is in a busy phase (Starting, Running, or
    /// Cancelling).  Use instead of matching on individual variants when the
    /// caller only cares about "busy vs. not busy", e.g., to decide whether
    /// to show a working-status indicator in the session list.
    ///
    /// Note: `WaitingForApproval` is intentionally excluded — the session is
    /// blocked on the user, not actively processing work.
    pub fn is_in_progress(&self) -> bool {
        matches!(self, Self::Starting | Self::Running { .. } | Self::Cancelling { .. })
    }

    /// `true` when the runtime is actively emitting events — i.e., a tmux
    /// session is running and the AI agent has started executing a turn.
    ///
    /// Unlike `is_in_progress()`, this excludes `Starting`, which is a
    /// transient pre-launch phase before the runtime has sent any events.
    /// Use this predicate in the session-state observer to gate reactions to
    /// `RuntimeProgress` events: such events cannot arrive during `Starting`.
    pub fn is_runtime_active(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Cancelling { .. })
    }

    /// `true` when the session is quiescent and ready for the next command.
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// `true` when the session has entered a terminal failure state.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCommand {
    pub text: String,
}

/// The AI coding agent to use for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    #[default]
    ClaudeCode,
    Codex,
    Gemini,
}

impl AgentType {
    /// Determine agent type from a Slack slash command (e.g. "/cc", "/cx", "/gm").
    pub fn from_slash_command(cmd: &str) -> Self {
        match cmd {
            "/cx" => Self::Codex,
            "/gm" => Self::Gemini,
            _ => Self::ClaudeCode, // "/cc" and anything else → Claude Code
        }
    }

    /// Human-readable name for UI display.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMsg {
    UserCommand(UserCommand),
    SendKey { key: String },
    ApprovalGranted,
    ApprovalRejected,
    RuntimeProgress { text: String },
    RuntimeCompleted { turn_id: TurnId, summary: String },
    RuntimeFailed { turn_id: TurnId, error: String },
    Interrupt,
    Terminate,
    /// Start or recover the session's tmux process with the given shell command.
    Recover { launch_command: String },
}
