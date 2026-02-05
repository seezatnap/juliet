use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

mod role_name;
mod role_state;

const PROMPT_FILE: &str = "prompts/juliet.md";
const GENERAL_USAGE: &str = "Usage: juliet <command> [options]\nCommands:\n  juliet init --role <name>\n  juliet --role <name> <claude|codex>\n  juliet <claude|codex>";
const INIT_USAGE: &str = "Usage: juliet init --role <name>";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Engine {
    Claude,
    Codex,
}

impl Engine {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CliCommand {
    Init { role_name: String },
    Launch {
        role_name: Option<String>,
        engine: Engine,
    },
}

#[derive(Debug, Eq, PartialEq)]
enum CliError {
    Usage,
    InitUsage,
}

impl CliError {
    fn message(&self) -> &'static str {
        match self {
            Self::Usage => GENERAL_USAGE,
            Self::InitUsage => INIT_USAGE,
        }
    }
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, CliError> {
    if args.is_empty() {
        return Err(CliError::Usage);
    }

    match args[0].as_str() {
        "init" => parse_init_command(args),
        "--role" => parse_explicit_role_launch(args),
        _ => parse_implicit_role_launch(args),
    }
}

fn parse_init_command(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() != 3 || args[1] != "--role" {
        return Err(CliError::InitUsage);
    }

    Ok(CliCommand::Init {
        role_name: args[2].clone(),
    })
}

fn parse_explicit_role_launch(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() != 3 {
        return Err(CliError::Usage);
    }

    let engine = match Engine::parse(&args[2]) {
        Some(parsed) => parsed,
        None => return Err(CliError::Usage),
    };

    Ok(CliCommand::Launch {
        role_name: Some(args[1].clone()),
        engine,
    })
}

fn parse_implicit_role_launch(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() != 1 {
        return Err(CliError::Usage);
    }

    let engine = match Engine::parse(&args[0]) {
        Some(parsed) => parsed,
        None => return Err(CliError::Usage),
    };

    Ok(CliCommand::Launch {
        role_name: None,
        engine,
    })
}

fn run_codex(prompt: &str, cwd: &Path) -> io::Result<i32> {
    let mut child = Command::new("codex")
        .arg("exec")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("-C")
        .arg(cwd)
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    let status = child.wait()?;
    Ok(status.code().unwrap_or(1))
}

fn run_claude(prompt: &str, cwd: &Path) -> io::Result<i32> {
    let status = Command::new("claude")
        .arg("-p")
        .arg(prompt)
        .current_dir(cwd)
        .status()?;

    Ok(status.code().unwrap_or(1))
}

fn run_engine(engine: Engine, prompt: &str, cwd: &Path) -> io::Result<i32> {
    match engine {
        Engine::Claude => run_claude(prompt, cwd),
        Engine::Codex => run_codex(prompt, cwd),
    }
}

fn run_init_command(_role_name: &str) -> i32 {
    0
}

fn run_launch_command(_role_name: Option<&str>, engine: Engine) -> i32 {
    let prompt = match fs::read_to_string(PROMPT_FILE) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {PROMPT_FILE}: {err}");
            return 1;
        }
    };

    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    match run_engine(engine, &prompt, &cwd) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("failed to run engine: {err}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match parse_cli_command(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err.message());
            std::process::exit(1);
        }
    };

    let exit_code = match command {
        CliCommand::Init { role_name } => run_init_command(&role_name),
        CliCommand::Launch { role_name, engine } => run_launch_command(role_name.as_deref(), engine),
    };

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_init_with_role() {
        assert_eq!(
            parse_cli_command(&to_args(&["init", "--role", "director-of-engineering"])),
            Ok(CliCommand::Init {
                role_name: "director-of-engineering".to_string()
            })
        );
    }

    #[test]
    fn parses_explicit_role_launch() {
        assert_eq!(
            parse_cli_command(&to_args(&["--role", "director-of-engineering", "codex"])),
            Ok(CliCommand::Launch {
                role_name: Some("director-of-engineering".to_string()),
                engine: Engine::Codex
            })
        );
    }

    #[test]
    fn parses_implicit_role_launch() {
        assert_eq!(
            parse_cli_command(&to_args(&["claude"])),
            Ok(CliCommand::Launch {
                role_name: None,
                engine: Engine::Claude
            })
        );
    }

    #[test]
    fn usage_error_when_no_arguments_are_provided() {
        let error = parse_cli_command(&to_args(&[])).unwrap_err();
        assert_eq!(error, CliError::Usage);
        assert_eq!(error.message(), GENERAL_USAGE);
    }

    #[test]
    fn usage_error_when_init_missing_role_option() {
        let error = parse_cli_command(&to_args(&["init"])).unwrap_err();
        assert_eq!(error, CliError::InitUsage);
        assert_eq!(error.message(), INIT_USAGE);
    }
}
