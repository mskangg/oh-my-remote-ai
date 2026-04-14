use core_model::UserCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRisk {
    Safe,
    ApprovalRequired,
}

pub fn classify(command: &UserCommand) -> CommandRisk {
    let text = command.text.to_lowercase();

    if text.contains("commit")
        || text.contains("delete")
        || text.contains("remove")
        || text.contains("edit")
    {
        return CommandRisk::ApprovalRequired;
    }

    CommandRisk::Safe
}
