#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run { task: String },
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let Some(command) = args.next() else {
        return Err("no command given".to_string());
    };
    if command != "run" {
        return Err(format!("unknown command: {command}"));
    }
    let Some(task) = args.next() else {
        return Err("run requires a task description".to_string());
    };
    Ok(Command::Run { task })
}
