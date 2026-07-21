use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct JumpCommand;

impl SlashCommand for JumpCommand {
    fn name(&self) -> &str {
        "jump"
    }

    fn description(&self) -> &str {
        "Jump to a turn in the conversation"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn available_in_minimal(&self) -> bool {
        false
    }

    fn usage(&self) -> &str {
        "/jump"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::JumpShowPicker)
    }
}
