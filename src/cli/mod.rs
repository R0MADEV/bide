#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run { task: String, yes: bool },
    Doctor,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let Some(command) = args.next() else {
        return Err("no command given".to_string());
    };
    match command.as_str() {
        "doctor" => Ok(Command::Doctor),
        "run" => parse_run(args),
        other => Err(format!("unknown command: {other}")),
    }
}

fn parse_run(args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut task: Option<String> = None;
    let mut yes = false;

    for arg in args {
        if arg == "--yes" || arg == "-y" {
            yes = true;
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!("unknown flag: {arg}"));
        }
        if task.is_some() {
            return Err("run takes a single task description".to_string());
        }
        task = Some(arg);
    }

    let Some(task) = task else {
        return Err("run requires a task description".to_string());
    };
    Ok(Command::Run { task, yes })
}
