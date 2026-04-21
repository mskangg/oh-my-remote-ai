use core_model::UserCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRisk {
    Safe,
    ApprovalRequired,
}

/// Keywords that require explicit user approval before the agent proceeds.
///
/// Adding a new keyword is a data change here — `classify` itself never needs
/// to change (Open–Closed Principle).
const APPROVAL_REQUIRED_KEYWORDS: &[&str] = &["commit", "delete", "remove", "edit"];

pub fn classify(command: &UserCommand) -> CommandRisk {
    let text = command.text.to_lowercase();
    if APPROVAL_REQUIRED_KEYWORDS.iter().any(|kw| text.contains(kw)) {
        CommandRisk::ApprovalRequired
    } else {
        CommandRisk::Safe
    }
}
