use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
enum JulietCommand {
    Ask { input: Option<String> },
    Next,
    Feedback { message: String },
}

fn parse_args(args: &[String]) -> Result<JulietCommand, String> {
    if args.is_empty() {
        return Err("missing command".to_string());
    }

    match args[0].as_str() {
        "ask" => {
            let input = if args.len() > 1 {
                Some(args[1..].join(" "))
            } else {
                None
            };
            Ok(JulietCommand::Ask { input })
        }
        "next" => {
            if args.len() > 1 {
                return Err("unexpected arguments for 'next'".to_string());
            }
            Ok(JulietCommand::Next)
        }
        "feedback" => {
            if args.len() < 2 {
                return Err("missing feedback message".to_string());
            }
            Ok(JulietCommand::Feedback {
                message: args[1..].join(" "),
            })
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn prompt_path(cmd: &JulietCommand) -> &'static str {
    match cmd {
        JulietCommand::Ask { .. } => "prompts/ask.md",
        JulietCommand::Next => "prompts/next.md",
        JulietCommand::Feedback { .. } => "prompts/feedback.md",
    }
}

fn build_prompt(base: &str, user_input: Option<&str>) -> String {
    if let Some(input) = user_input {
        format!("{base}\n\nUser input:\n{input}")
    } else {
        base.to_string()
    }
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

fn usage() -> &'static str {
    "Usage:\n  juliet ask [PRD_PATH]\n  juliet next\n  juliet feedback <MESSAGE>"
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = match parse_args(&args[1..]) {
        Ok(cmd) => cmd,
        Err(message) => {
            eprintln!("{message}\n\n{}", usage());
            std::process::exit(2);
        }
    };

    let prompt_file = prompt_path(&cmd);
    let prompt = match fs::read_to_string(prompt_file) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {prompt_file}: {err}");
            std::process::exit(1);
        }
    };

    let user_input = match &cmd {
        JulietCommand::Ask { input } => input.as_deref(),
        JulietCommand::Next => None,
        JulietCommand::Feedback { message } => Some(message.as_str()),
    };

    let full_prompt = build_prompt(&prompt, user_input);

    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            std::process::exit(1);
        }
    };

    let exit_code = match run_codex(&full_prompt, &cwd) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("failed to run codex: {err}");
            std::process::exit(1);
        }
    };

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_handles_ask_with_optional_input() {
        let args = vec!["ask".to_string()];
        assert_eq!(
            parse_args(&args).unwrap(),
            JulietCommand::Ask { input: None }
        );

        let args = vec!["ask".to_string(), "~/prds/foo.md".to_string()];
        assert_eq!(
            parse_args(&args).unwrap(),
            JulietCommand::Ask {
                input: Some("~/prds/foo.md".to_string())
            }
        );

        let args = vec![
            "ask".to_string(),
            "path".to_string(),
            "with".to_string(),
            "spaces".to_string(),
        ];
        assert_eq!(
            parse_args(&args).unwrap(),
            JulietCommand::Ask {
                input: Some("path with spaces".to_string())
            }
        );
    }

    #[test]
    fn parse_args_enforces_next_has_no_args() {
        let args = vec!["next".to_string()];
        assert_eq!(parse_args(&args).unwrap(), JulietCommand::Next);

        let args = vec!["next".to_string(), "extra".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_args_requires_feedback_message() {
        let args = vec!["feedback".to_string()];
        assert!(parse_args(&args).is_err());

        let args = vec!["feedback".to_string(), "ok".to_string(), "add".to_string()];
        assert_eq!(
            parse_args(&args).unwrap(),
            JulietCommand::Feedback {
                message: "ok add".to_string()
            }
        );
    }

    #[test]
    fn build_prompt_appends_user_input() {
        let base = "Base prompt";
        assert_eq!(build_prompt(base, None), base.to_string());
        assert_eq!(
            build_prompt(base, Some("hello")),
            "Base prompt\n\nUser input:\nhello".to_string()
        );
    }
}
