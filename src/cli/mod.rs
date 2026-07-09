#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run { task: String },
    Doctor,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let Some(command) = args.next() else {
        return Err("no command given".to_string());
    };
    match command.as_str() {
        "doctor" => Ok(Command::Doctor),
        "run" => parse_run(args.next()),
        other => Err(format!("unknown command: {other}")),
    }
}

fn parse_run(task: Option<String>) -> Result<Command, String> {
    let Some(task) = task else {
        return Err("run requires a task description".to_string());
    };
    Ok(Command::Run { task })
}
