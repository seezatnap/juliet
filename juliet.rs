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
const DEFAULT_PROMPT_SEED: &str = include_str!("prompts/juliet.md");
const OPERATOR_PLACEHOLDER: &str =
    "<!-- TODO: Replace with role-specific instructions and expected operator input. -->";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitOutcome {
    Initialized,
    AlreadyExists,
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

fn role_prompt_template(role_name: &str, default_prompt_seed: &str) -> String {
    format!(
        "# {role_name}\n\n{OPERATOR_PLACEHOLDER}\n\n## Default Prompt Seed\n\n{default_prompt_seed}"
    )
}

fn ensure_role_prompt_exists(
    project_root: &Path,
    role_name: &str,
    default_prompt_seed: &str,
) -> io::Result<()> {
    let prompt_path = role_state::role_prompt_path(project_root, role_name);
    if prompt_path.exists() {
        if prompt_path.is_file() {
            return Ok(());
        }

        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "expected file path for role prompt, found non-file: {}",
                prompt_path.display()
            ),
        ));
    }

    if let Some(parent_dir) = prompt_path.parent() {
        fs::create_dir_all(parent_dir)?;
    }

    fs::write(
        prompt_path,
        role_prompt_template(role_name, default_prompt_seed),
    )
}

fn initialize_role(
    project_root: &Path,
    role_name: &str,
    default_prompt_seed: &str,
) -> Result<InitOutcome, String> {
    role_name::validate_role_name(role_name)?;

    let prompt_path = role_state::role_prompt_path(project_root, role_name);
    let prompt_exists = prompt_path.is_file();
    let state_exists = role_state::role_state_exists(project_root, role_name);

    if prompt_exists && state_exists {
        return Ok(InitOutcome::AlreadyExists);
    }

    ensure_role_prompt_exists(project_root, role_name, default_prompt_seed).map_err(|err| {
        format!(
            "failed to initialize prompt for role {role_name} at {}: {err}",
            prompt_path.display()
        )
    })?;
    role_state::create_role_state(project_root, role_name)
        .map_err(|err| format!("failed to initialize state for role {role_name}: {err}"))?;

    Ok(InitOutcome::Initialized)
}

fn run_init_command(role_name: &str) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    match initialize_role(&cwd, role_name, DEFAULT_PROMPT_SEED) {
        Ok(InitOutcome::Initialized) => {
            println!("Initialized role: {role_name}");
            0
        }
        Ok(InitOutcome::AlreadyExists) => {
            println!("Role already exists: {role_name}");
            0
        }
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
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
        CliCommand::Launch { role_name, engine } => {
            run_launch_command(role_name.as_deref(), engine)
        }
    };

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time drift should not occur in tests")
                .as_nanos();
            let path =
                env::temp_dir().join(format!("juliet-cli-{name}-{}-{timestamp}", process::id()));
            fs::create_dir_all(&path).expect("test directory should be created");

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

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

    #[test]
    fn initialize_role_rejects_invalid_role_name() {
        let temp = TestDir::new("invalid-role");
        let err = initialize_role(temp.path(), "Invalid_Name", "seed prompt")
            .expect_err("invalid role name should fail");

        assert_eq!(
            err,
            "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens."
        );
    }

    #[test]
    fn initialize_role_creates_prompt_template_and_state_structure() {
        let temp = TestDir::new("fresh-init");
        let role_name = "director-of-engineering";

        let outcome =
            initialize_role(temp.path(), role_name, "## Seeded prompt").expect("init should work");
        assert_eq!(outcome, InitOutcome::Initialized);

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        let prompt_contents =
            fs::read_to_string(prompt_path).expect("role prompt should be readable");
        assert!(prompt_contents.contains("# director-of-engineering"));
        assert!(prompt_contents.contains(OPERATOR_PLACEHOLDER));
        assert!(prompt_contents.contains("## Seeded prompt"));

        let role_dir = role_state::role_state_dir(temp.path(), role_name);
        assert!(role_dir.is_dir());
        assert!(role_dir.join("session.md").is_file());
        assert!(role_dir.join("needs-from-operator.md").is_file());
        assert!(role_dir.join("projects.md").is_file());
        assert!(role_dir.join("processes.md").is_file());
        assert!(role_dir.join("artifacts").is_dir());
    }

    #[test]
    fn initialize_role_is_idempotent_when_prompt_and_state_both_exist() {
        let temp = TestDir::new("already-exists");
        let role_name = "director-of-marketing";

        let first = initialize_role(temp.path(), role_name, "seed prompt one")
            .expect("first init should succeed");
        assert_eq!(first, InitOutcome::Initialized);

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        let session_path = role_state::role_state_dir(temp.path(), role_name).join("session.md");
        fs::write(&prompt_path, "custom prompt").expect("prompt should be mutable for test");
        fs::write(&session_path, "custom session").expect("state file should be mutable for test");

        let second = initialize_role(temp.path(), role_name, "seed prompt two")
            .expect("second init should be idempotent");
        assert_eq!(second, InitOutcome::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&prompt_path).expect("prompt should still exist"),
            "custom prompt"
        );
        assert_eq!(
            fs::read_to_string(&session_path).expect("session should still exist"),
            "custom session"
        );
    }

    #[test]
    fn initialize_role_creates_missing_state_when_prompt_already_exists() {
        let temp = TestDir::new("prompt-only");
        let role_name = "operations";
        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::create_dir_all(prompt_path.parent().expect("prompts dir should exist"))
            .expect("prompts directory should be created");
        fs::write(&prompt_path, "# custom operations prompt").expect("prompt should be created");

        let outcome = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("init should create missing state");
        assert_eq!(outcome, InitOutcome::Initialized);
        assert_eq!(
            fs::read_to_string(&prompt_path).expect("prompt should remain unchanged"),
            "# custom operations prompt"
        );

        let role_dir = role_state::role_state_dir(temp.path(), role_name);
        assert!(role_dir.is_dir());
        assert!(role_dir.join("session.md").is_file());
        assert!(role_dir.join("needs-from-operator.md").is_file());
        assert!(role_dir.join("projects.md").is_file());
        assert!(role_dir.join("processes.md").is_file());
        assert!(role_dir.join("artifacts").is_dir());
    }

    #[test]
    fn initialize_role_creates_missing_prompt_when_state_already_exists() {
        let temp = TestDir::new("state-only");
        let role_name = "program-manager";
        role_state::create_role_state(temp.path(), role_name)
            .expect("state scaffold should be created");

        let session_path = role_state::role_state_dir(temp.path(), role_name).join("session.md");
        fs::write(&session_path, "existing session data").expect("session should be writable");

        let outcome =
            initialize_role(temp.path(), role_name, "## Embedded seed").expect("init should work");
        assert_eq!(outcome, InitOutcome::Initialized);
        assert_eq!(
            fs::read_to_string(&session_path).expect("session should remain unchanged"),
            "existing session data"
        );

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        let prompt_contents = fs::read_to_string(prompt_path).expect("prompt should be readable");
        assert!(prompt_contents.contains("# program-manager"));
        assert!(prompt_contents.contains(OPERATOR_PLACEHOLDER));
        assert!(prompt_contents.contains("## Embedded seed"));
    }
}
