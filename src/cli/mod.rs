#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub task: String,
    pub yes: bool,
    pub branch: bool,
    pub pr: bool,
    pub agent: Option<String>,
    pub context: Option<String>,
    pub resume: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(RunOptions),
    Doctor,
    Help,
}

pub fn parse(args: impl Iterator<Item = String>) -> Result<Command, String> {
    let args: Vec<String> = args.collect();
    let Some(command) = args.first() else {
        return Err("no command given".to_string());
    };
    match command.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "doctor" => Ok(Command::Doctor),
        "run" => parse_run(&args[1..]),
        other => Err(format!("unknown command: {other}")),
    }
}

fn parse_run(args: &[String]) -> Result<Command, String> {
    let mut options = RunOptions::default();
    let mut task: Option<String> = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--yes" | "-y" => options.yes = true,
            "--branch" => options.branch = true,
            "--pr" => options.pr = true,
            "--agent" => {
                index += 1;
                options.agent = Some(value(args, index, "--agent")?);
            }
            "--context" => {
                index += 1;
                options.context = Some(value(args, index, "--context")?);
            }
            "--resume" => {
                index += 1;
                options.resume = Some(value(args, index, "--resume")?);
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag: {flag}")),
            _ if task.is_some() => return Err("run takes a single task description".to_string()),
            _ => task = Some(args[index].clone()),
        }
        index += 1;
    }

    if task.is_none() && options.resume.is_none() {
        return Err("run requires a task description".to_string());
    }
    options.task = task.unwrap_or_default();
    Ok(Command::Run(options))
}

fn value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}
