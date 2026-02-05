use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

mod role_name;

const PROMPT_FILE: &str = "prompts/juliet.md";

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

fn main() {
    let args: Vec<String> = env::args().collect();

    let user_input = if args.len() > 1 {
        Some(args[1..].join(" "))
    } else {
        None
    };

    let prompt = match fs::read_to_string(PROMPT_FILE) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {PROMPT_FILE}: {err}");
            std::process::exit(1);
        }
    };

    let full_prompt = build_prompt(&prompt, user_input.as_deref());

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
    fn build_prompt_appends_user_input() {
        let base = "Base prompt";
        assert_eq!(build_prompt(base, None), base.to_string());
        assert_eq!(
            build_prompt(base, Some("hello")),
            "Base prompt\n\nUser input:\nhello".to_string()
        );
    }
}
