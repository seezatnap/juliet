use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

mod role_name;
mod role_state;

const GENERAL_USAGE: &str = "Usage: juliet <command> [options]\n\nCommands:\n  Initialize a new role:\n    juliet init --role <name>\n\n  Launch a specific role:\n    juliet --role <name> <claude|codex>\n\n  Launch (auto-selects role when only one exists):\n    juliet <claude|codex>\n\n  Reset a role's prompt to default:\n    juliet reset-prompt --role <name>\n\n  Clear a role's history:\n    juliet clear-history --role <name>\n\n  Execute a single non-interactive turn:\n    juliet exec --role <name> <claude|codex> <message...>\n    juliet exec <claude|codex> <message...>";
const INIT_USAGE: &str = "Usage: juliet init --role <name>";
const RESET_PROMPT_USAGE: &str = "Usage: juliet reset-prompt --role <name>";
const CLEAR_HISTORY_USAGE: &str = "Usage: juliet clear-history --role <name>";
const EXEC_USAGE: &str = "Usage: juliet exec [--role <name>] <claude|codex> <message...>";
const DEFAULT_PROMPT_SEED: &str = include_str!("prompts/juliet.md");
const NO_ROLES_CONFIGURED_ERROR: &str = "No roles configured. Run: juliet init --role <name>";
const MULTIPLE_ROLES_FOUND_ERROR: &str =
    "Multiple roles found. Specify one with --role <name>:";
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
    Init {
        role_name: String,
    },
    Launch {
        role_name: Option<String>,
        engine: Engine,
        operator_input: Option<String>,
    },
    ResetPrompt {
        role_name: String,
    },
    ClearHistory {
        role_name: String,
    },
    Exec {
        role_name: Option<String>,
        engine: Engine,
        message: String,
    },
}

#[derive(Debug, Eq, PartialEq)]
enum CliError {
    Usage,
    InitUsage,
    ResetPromptUsage,
    ClearHistoryUsage,
    ExecUsage,
}

impl CliError {
    fn message(&self) -> &'static str {
        match self {
            Self::Usage => GENERAL_USAGE,
            Self::InitUsage => INIT_USAGE,
            Self::ResetPromptUsage => RESET_PROMPT_USAGE,
            Self::ClearHistoryUsage => CLEAR_HISTORY_USAGE,
            Self::ExecUsage => EXEC_USAGE,
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
        "reset-prompt" => parse_reset_prompt_command(args),
        "clear-history" => parse_clear_history_command(args),
        "exec" => parse_exec_command(args),
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

fn parse_reset_prompt_command(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() != 3 || args[1] != "--role" {
        return Err(CliError::ResetPromptUsage);
    }

    Ok(CliCommand::ResetPrompt {
        role_name: args[2].clone(),
    })
}

fn parse_clear_history_command(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() != 3 || args[1] != "--role" {
        return Err(CliError::ClearHistoryUsage);
    }

    Ok(CliCommand::ClearHistory {
        role_name: args[2].clone(),
    })
}

fn parse_exec_command(args: &[String]) -> Result<CliCommand, CliError> {
    // args[0] == "exec"
    if args.len() < 2 {
        return Err(CliError::ExecUsage);
    }

    if args[1] == "--role" {
        // exec --role <name> <engine> <message...>
        if args.len() < 5 {
            return Err(CliError::ExecUsage);
        }

        let engine = match Engine::parse(&args[3]) {
            Some(parsed) => parsed,
            None => return Err(CliError::ExecUsage),
        };

        let message = args[4..].join(" ");
        if message.is_empty() {
            return Err(CliError::ExecUsage);
        }

        Ok(CliCommand::Exec {
            role_name: Some(args[2].clone()),
            engine,
            message,
        })
    } else {
        // exec <engine> <message...>
        if args.len() < 3 {
            return Err(CliError::ExecUsage);
        }

        let engine = match Engine::parse(&args[1]) {
            Some(parsed) => parsed,
            None => return Err(CliError::ExecUsage),
        };

        let message = args[2..].join(" ");
        if message.is_empty() {
            return Err(CliError::ExecUsage);
        }

        Ok(CliCommand::Exec {
            role_name: None,
            engine,
            message,
        })
    }
}

fn parse_explicit_role_launch(args: &[String]) -> Result<CliCommand, CliError> {
    if args.len() < 3 {
        return Err(CliError::Usage);
    }

    let engine = match Engine::parse(&args[2]) {
        Some(parsed) => parsed,
        None => return Err(CliError::Usage),
    };

    Ok(CliCommand::Launch {
        role_name: Some(args[1].clone()),
        engine,
        operator_input: parse_operator_input(&args[3..]),
    })
}

fn parse_implicit_role_launch(args: &[String]) -> Result<CliCommand, CliError> {
    if args.is_empty() {
        return Err(CliError::Usage);
    }

    let engine = match Engine::parse(&args[0]) {
        Some(parsed) => parsed,
        None => return Err(CliError::Usage),
    };

    Ok(CliCommand::Launch {
        role_name: None,
        engine,
        operator_input: parse_operator_input(&args[1..]),
    })
}

fn parse_operator_input(args: &[String]) -> Option<String> {
    if args.is_empty() {
        None
    } else {
        Some(args.join(" "))
    }
}

fn run_codex(prompt: &str, cwd: &Path) -> io::Result<i32> {
    let status = Command::new("codex")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg(prompt)
        .current_dir(cwd)
        .status()?;

    Ok(status.code().unwrap_or(1))
}

fn run_claude(prompt: &str, cwd: &Path) -> io::Result<i32> {
    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg(prompt)
        .env("IS_SANDBOX", "1")
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

fn build_launch_prompt(base: &str, operator_input: Option<&str>) -> String {
    if let Some(input) = operator_input {
        format!("{base}\n\nUser input:\n{input}")
    } else {
        base.to_string()
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
    let state_is_scaffolded = role_state::role_state_is_scaffolded(project_root, role_name);

    if prompt_exists && state_is_scaffolded {
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

fn reset_prompt(
    project_root: &Path,
    role_name: &str,
    default_prompt_seed: &str,
) -> Result<(), String> {
    role_name::validate_role_name(role_name)?;

    if !role_state::role_state_exists(project_root, role_name) {
        return Err(format!("Role '{role_name}' is not initialized."));
    }

    let prompt_path = role_state::role_prompt_path(project_root, role_name);
    let content = role_prompt_template(role_name, default_prompt_seed);
    fs::write(&prompt_path, content).map_err(|err| {
        format!(
            "failed to write prompt for role {role_name} at {}: {err}",
            prompt_path.display()
        )
    })?;

    Ok(())
}

fn clear_history(project_root: &Path, role_name: &str) -> Result<(), String> {
    role_name::validate_role_name(role_name)?;

    if !role_state::role_state_exists(project_root, role_name) {
        return Err(format!("Role '{role_name}' is not initialized."));
    }

    role_state::clear_role_history(project_root, role_name).map_err(|err| {
        format!("failed to clear history for role {role_name}: {err}")
    })?;

    Ok(())
}

fn run_clear_history_command(role_name: &str) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    match clear_history(&cwd, role_name) {
        Ok(()) => {
            println!("history cleared for role '{role_name}'");
            0
        }
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn run_reset_prompt_command(role_name: &str) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    match reset_prompt(&cwd, role_name, DEFAULT_PROMPT_SEED) {
        Ok(()) => {
            println!("prompt reset to default for role '{role_name}'");
            0
        }
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
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

fn stage_explicit_role_prompt(project_root: &Path, role_name: &str) -> Result<String, String> {
    role_name::validate_role_name(role_name)?;

    if !role_state::role_state_exists(project_root, role_name) {
        return Err(format!(
            "Role not found: {role_name}. Run: juliet init --role {role_name}"
        ));
    }

    let prompt_path = role_state::role_prompt_path(project_root, role_name);
    let prompt = fs::read_to_string(&prompt_path)
        .map_err(|err| format!("failed to read {}: {err}", prompt_path.display()))?;

    let runtime_prompt_path = role_state::runtime_prompt_path(project_root, role_name);
    role_state::write_runtime_prompt(project_root, role_name, &prompt).map_err(|err| {
        format!(
            "failed to write runtime prompt for role {role_name} at {}: {err}",
            runtime_prompt_path.display()
        )
    })?;

    Ok(prompt)
}

fn stage_implicit_role_prompt(project_root: &Path) -> Result<String, String> {
    let roles = role_state::discover_configured_roles(project_root)
        .map_err(|err| format!("failed to discover configured roles: {err}"))?;

    match roles.as_slice() {
        [] => Err(NO_ROLES_CONFIGURED_ERROR.to_string()),
        [role] => stage_explicit_role_prompt(project_root, &role.name),
        _ => {
            let role_names = roles
                .iter()
                .map(|role| role.name.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!("{MULTIPLE_ROLES_FOUND_ERROR}\n{role_names}"))
        }
    }
}

fn prepare_launch_prompt(project_root: &Path, role_name: Option<&str>) -> Result<String, String> {
    match role_name {
        Some(name) => stage_explicit_role_prompt(project_root, name),
        None => stage_implicit_role_prompt(project_root),
    }
}

fn run_launch_command_in_dir<F>(
    project_root: &Path,
    role_name: Option<&str>,
    engine: Engine,
    operator_input: Option<&str>,
    engine_runner: F,
) -> i32
where
    F: FnOnce(Engine, &str, &Path) -> io::Result<i32>,
{
    let prompt = match prepare_launch_prompt(project_root, role_name) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let prompt = build_launch_prompt(&prompt, operator_input);

    match engine_runner(engine, &prompt, project_root) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("failed to run engine: {err}");
            1
        }
    }
}

fn run_launch_command(role_name: Option<&str>, engine: Engine, operator_input: Option<&str>) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    run_launch_command_in_dir(&cwd, role_name, engine, operator_input, run_engine)
}

fn run_exec_command_in_dir<F>(
    project_root: &Path,
    role_name: Option<&str>,
    engine: Engine,
    message: &str,
    engine_runner: F,
) -> i32
where
    F: FnOnce(Engine, &str, &Path) -> io::Result<i32>,
{
    let base_prompt = match prepare_launch_prompt(project_root, role_name) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let prompt = build_launch_prompt(&base_prompt, Some(message));

    match engine_runner(engine, &prompt, project_root) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("failed to run engine: {err}");
            1
        }
    }
}

fn run_exec_command(role_name: Option<&str>, engine: Engine, message: &str) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    run_exec_command_in_dir(&cwd, role_name, engine, message, run_engine)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match parse_cli_command(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            println!("{}", err.message());
            std::process::exit(1);
        }
    };

    let exit_code = match command {
        CliCommand::Init { role_name } => run_init_command(&role_name),
        CliCommand::Launch {
            role_name,
            engine,
            operator_input,
        } => run_launch_command(role_name.as_deref(), engine, operator_input.as_deref()),
        CliCommand::ResetPrompt { role_name } => run_reset_prompt_command(&role_name),
        CliCommand::ClearHistory { role_name } => run_clear_history_command(&role_name),
        CliCommand::Exec {
            role_name,
            engine,
            message,
        } => run_exec_command(role_name.as_deref(), engine, &message),
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
                engine: Engine::Codex,
                operator_input: None,
            })
        );
    }

    #[test]
    fn parses_implicit_role_launch() {
        assert_eq!(
            parse_cli_command(&to_args(&["claude"])),
            Ok(CliCommand::Launch {
                role_name: None,
                engine: Engine::Claude,
                operator_input: None,
            })
        );
    }

    #[test]
    fn parses_explicit_role_launch_with_operator_input() {
        assert_eq!(
            parse_cli_command(&to_args(&[
                "--role",
                "director-of-engineering",
                "codex",
                "start",
                "from",
                "~/prds/foo.md",
            ])),
            Ok(CliCommand::Launch {
                role_name: Some("director-of-engineering".to_string()),
                engine: Engine::Codex,
                operator_input: Some("start from ~/prds/foo.md".to_string()),
            })
        );
    }

    #[test]
    fn parses_implicit_role_launch_with_operator_input() {
        assert_eq!(
            parse_cli_command(&to_args(&["claude", "continue", "project", "alpha"])),
            Ok(CliCommand::Launch {
                role_name: None,
                engine: Engine::Claude,
                operator_input: Some("continue project alpha".to_string()),
            })
        );
    }

    // reset-prompt parsing tests

    #[test]
    fn parses_reset_prompt_with_role() {
        assert_eq!(
            parse_cli_command(&to_args(&["reset-prompt", "--role", "director-of-engineering"])),
            Ok(CliCommand::ResetPrompt {
                role_name: "director-of-engineering".to_string()
            })
        );
    }

    #[test]
    fn reset_prompt_usage_error_when_missing_role_option() {
        let error = parse_cli_command(&to_args(&["reset-prompt"])).unwrap_err();
        assert_eq!(error, CliError::ResetPromptUsage);
        assert_eq!(error.message(), RESET_PROMPT_USAGE);
    }

    #[test]
    fn reset_prompt_usage_error_when_missing_role_name() {
        let error = parse_cli_command(&to_args(&["reset-prompt", "--role"])).unwrap_err();
        assert_eq!(error, CliError::ResetPromptUsage);
        assert_eq!(error.message(), RESET_PROMPT_USAGE);
    }

    #[test]
    fn reset_prompt_usage_error_when_extra_args() {
        let error =
            parse_cli_command(&to_args(&["reset-prompt", "--role", "my-role", "extra"])).unwrap_err();
        assert_eq!(error, CliError::ResetPromptUsage);
    }

    #[test]
    fn reset_prompt_usage_error_when_wrong_flag() {
        let error =
            parse_cli_command(&to_args(&["reset-prompt", "--name", "my-role"])).unwrap_err();
        assert_eq!(error, CliError::ResetPromptUsage);
    }

    #[test]
    fn parses_reset_prompt_with_simple_role_name() {
        assert_eq!(
            parse_cli_command(&to_args(&["reset-prompt", "--role", "ops"])),
            Ok(CliCommand::ResetPrompt {
                role_name: "ops".to_string()
            })
        );
    }

    #[test]
    fn parses_reset_prompt_with_numeric_role_name() {
        assert_eq!(
            parse_cli_command(&to_args(&["reset-prompt", "--role", "agent-007"])),
            Ok(CliCommand::ResetPrompt {
                role_name: "agent-007".to_string()
            })
        );
    }

    #[test]
    fn reset_prompt_parser_passes_through_invalid_role_names() {
        // The parser does not validate role names; validation is deferred to execution.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let result = parse_cli_command(&to_args(&["reset-prompt", "--role", bad_name]));
            assert_eq!(
                result,
                Ok(CliCommand::ResetPrompt {
                    role_name: bad_name.to_string()
                }),
                "parser should accept any string as role_name without validation: {bad_name}"
            );
        }
    }

    #[test]
    fn reset_prompt_bad_role_name_rejected_by_validation() {
        // Role name validation rejects names that the parser passes through.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let err = role_name::validate_role_name(bad_name)
                .expect_err(&format!("role name '{bad_name}' should be rejected"));
            assert!(
                err.contains("Invalid role name"),
                "validation error for '{bad_name}' should contain 'Invalid role name': {err}"
            );
        }
    }

    #[test]
    fn reset_prompt_usage_error_when_role_flag_not_second_arg() {
        let error =
            parse_cli_command(&to_args(&["reset-prompt", "my-role", "--role"])).unwrap_err();
        assert_eq!(error, CliError::ResetPromptUsage);
    }

    // clear-history parsing tests

    #[test]
    fn parses_clear_history_with_role() {
        assert_eq!(
            parse_cli_command(&to_args(&["clear-history", "--role", "director-of-engineering"])),
            Ok(CliCommand::ClearHistory {
                role_name: "director-of-engineering".to_string()
            })
        );
    }

    #[test]
    fn clear_history_usage_error_when_missing_role_option() {
        let error = parse_cli_command(&to_args(&["clear-history"])).unwrap_err();
        assert_eq!(error, CliError::ClearHistoryUsage);
        assert_eq!(error.message(), CLEAR_HISTORY_USAGE);
    }

    #[test]
    fn clear_history_usage_error_when_missing_role_name() {
        let error = parse_cli_command(&to_args(&["clear-history", "--role"])).unwrap_err();
        assert_eq!(error, CliError::ClearHistoryUsage);
        assert_eq!(error.message(), CLEAR_HISTORY_USAGE);
    }

    #[test]
    fn clear_history_usage_error_when_extra_args() {
        let error =
            parse_cli_command(&to_args(&["clear-history", "--role", "my-role", "extra"])).unwrap_err();
        assert_eq!(error, CliError::ClearHistoryUsage);
    }

    #[test]
    fn clear_history_usage_error_when_wrong_flag() {
        let error =
            parse_cli_command(&to_args(&["clear-history", "--name", "my-role"])).unwrap_err();
        assert_eq!(error, CliError::ClearHistoryUsage);
    }

    #[test]
    fn parses_clear_history_with_simple_role_name() {
        assert_eq!(
            parse_cli_command(&to_args(&["clear-history", "--role", "qa"])),
            Ok(CliCommand::ClearHistory {
                role_name: "qa".to_string()
            })
        );
    }

    #[test]
    fn parses_clear_history_with_numeric_role_name() {
        assert_eq!(
            parse_cli_command(&to_args(&["clear-history", "--role", "team-42"])),
            Ok(CliCommand::ClearHistory {
                role_name: "team-42".to_string()
            })
        );
    }

    #[test]
    fn clear_history_parser_passes_through_invalid_role_names() {
        // The parser does not validate role names; validation is deferred to execution.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let result = parse_cli_command(&to_args(&["clear-history", "--role", bad_name]));
            assert_eq!(
                result,
                Ok(CliCommand::ClearHistory {
                    role_name: bad_name.to_string()
                }),
                "parser should accept any string as role_name without validation: {bad_name}"
            );
        }
    }

    #[test]
    fn clear_history_bad_role_name_rejected_by_validation() {
        // Role name validation rejects names that the parser passes through.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let err = role_name::validate_role_name(bad_name)
                .expect_err(&format!("role name '{bad_name}' should be rejected"));
            assert!(
                err.contains("Invalid role name"),
                "validation error for '{bad_name}' should contain 'Invalid role name': {err}"
            );
        }
    }

    #[test]
    fn clear_history_usage_error_when_role_flag_not_second_arg() {
        let error =
            parse_cli_command(&to_args(&["clear-history", "my-role", "--role"])).unwrap_err();
        assert_eq!(error, CliError::ClearHistoryUsage);
    }

    // exec parsing tests

    #[test]
    fn parses_exec_with_explicit_role_and_claude() {
        assert_eq!(
            parse_cli_command(&to_args(&[
                "exec", "--role", "my-role", "claude", "do", "the", "thing"
            ])),
            Ok(CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Claude,
                message: "do the thing".to_string(),
            })
        );
    }

    #[test]
    fn parses_exec_with_explicit_role_and_codex() {
        assert_eq!(
            parse_cli_command(&to_args(&[
                "exec", "--role", "my-role", "codex", "fix", "the", "bug"
            ])),
            Ok(CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Codex,
                message: "fix the bug".to_string(),
            })
        );
    }

    #[test]
    fn parses_exec_with_implicit_role_and_claude() {
        assert_eq!(
            parse_cli_command(&to_args(&["exec", "claude", "do", "the", "thing"])),
            Ok(CliCommand::Exec {
                role_name: None,
                engine: Engine::Claude,
                message: "do the thing".to_string(),
            })
        );
    }

    #[test]
    fn parses_exec_with_implicit_role_and_codex() {
        assert_eq!(
            parse_cli_command(&to_args(&["exec", "codex", "fix", "the", "bug"])),
            Ok(CliCommand::Exec {
                role_name: None,
                engine: Engine::Codex,
                message: "fix the bug".to_string(),
            })
        );
    }

    #[test]
    fn parses_exec_with_single_word_message() {
        assert_eq!(
            parse_cli_command(&to_args(&["exec", "claude", "hello"])),
            Ok(CliCommand::Exec {
                role_name: None,
                engine: Engine::Claude,
                message: "hello".to_string(),
            })
        );
    }

    #[test]
    fn parses_exec_explicit_role_with_single_word_message() {
        assert_eq!(
            parse_cli_command(&to_args(&["exec", "--role", "my-role", "codex", "hello"])),
            Ok(CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Codex,
                message: "hello".to_string(),
            })
        );
    }

    #[test]
    fn exec_joins_remaining_args_into_message() {
        assert_eq!(
            parse_cli_command(&to_args(&[
                "exec", "claude", "please", "fix", "all", "the", "bugs"
            ])),
            Ok(CliCommand::Exec {
                role_name: None,
                engine: Engine::Claude,
                message: "please fix all the bugs".to_string(),
            })
        );
    }

    #[test]
    fn exec_usage_error_when_no_args_after_exec() {
        let error = parse_cli_command(&to_args(&["exec"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
        assert_eq!(error.message(), EXEC_USAGE);
    }

    #[test]
    fn exec_usage_error_when_missing_message_implicit() {
        let error = parse_cli_command(&to_args(&["exec", "claude"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
    }

    #[test]
    fn exec_usage_error_when_invalid_engine_implicit() {
        let error = parse_cli_command(&to_args(&["exec", "invalid", "hello"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
    }

    #[test]
    fn exec_usage_error_when_missing_engine_and_message_explicit() {
        let error = parse_cli_command(&to_args(&["exec", "--role", "my-role"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
    }

    #[test]
    fn exec_usage_error_when_missing_message_explicit() {
        let error =
            parse_cli_command(&to_args(&["exec", "--role", "my-role", "claude"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
    }

    #[test]
    fn exec_usage_error_when_invalid_engine_explicit() {
        let error =
            parse_cli_command(&to_args(&["exec", "--role", "my-role", "invalid", "hello"]))
                .unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
    }

    #[test]
    fn exec_explicit_role_joins_remaining_args_into_message() {
        assert_eq!(
            parse_cli_command(&to_args(&[
                "exec", "--role", "my-role", "claude", "please", "fix", "all", "the", "bugs"
            ])),
            Ok(CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Claude,
                message: "please fix all the bugs".to_string(),
            })
        );
    }

    #[test]
    fn exec_parser_passes_through_invalid_role_names() {
        // The parser does not validate role names; validation is deferred to execution.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let result = parse_cli_command(&to_args(&[
                "exec", "--role", bad_name, "claude", "hello",
            ]));
            assert_eq!(
                result,
                Ok(CliCommand::Exec {
                    role_name: Some(bad_name.to_string()),
                    engine: Engine::Claude,
                    message: "hello".to_string(),
                }),
                "parser should accept any string as role_name without validation: {bad_name}"
            );
        }
    }

    #[test]
    fn exec_bad_role_name_rejected_by_validation() {
        // Role name validation rejects names that the parser passes through.
        for bad_name in ["Invalid_Name", "../traversal", "", "-leading", "UPPER"] {
            let err = role_name::validate_role_name(bad_name)
                .expect_err(&format!("role name '{bad_name}' should be rejected"));
            assert!(
                err.contains("Invalid role name"),
                "validation error for '{bad_name}' should contain 'Invalid role name': {err}"
            );
        }
    }

    #[test]
    fn exec_engine_parsing_is_case_sensitive() {
        // Only lowercase "claude" and "codex" are valid engines.
        for invalid_engine in ["Claude", "CLAUDE", "Codex", "CODEX", "Claude3", "gpt4"] {
            let error_implicit =
                parse_cli_command(&to_args(&["exec", invalid_engine, "hello"])).unwrap_err();
            assert_eq!(
                error_implicit, CliError::ExecUsage,
                "implicit form should reject engine '{invalid_engine}'"
            );

            let error_explicit = parse_cli_command(&to_args(&[
                "exec",
                "--role",
                "my-role",
                invalid_engine,
                "hello",
            ]))
            .unwrap_err();
            assert_eq!(
                error_explicit, CliError::ExecUsage,
                "explicit form should reject engine '{invalid_engine}'"
            );
        }
    }

    #[test]
    fn exec_usage_error_message_matches_exec_usage_constant() {
        let error = parse_cli_command(&to_args(&["exec"])).unwrap_err();
        assert_eq!(error.message(), EXEC_USAGE);

        let error = parse_cli_command(&to_args(&["exec", "claude"])).unwrap_err();
        assert_eq!(error.message(), EXEC_USAGE);

        let error =
            parse_cli_command(&to_args(&["exec", "--role", "my-role"])).unwrap_err();
        assert_eq!(error.message(), EXEC_USAGE);
    }

    #[test]
    fn exec_message_preserves_whitespace_between_args() {
        // Each arg is joined by a single space; internal whitespace within args is preserved.
        assert_eq!(
            parse_cli_command(&to_args(&["exec", "codex", "word1", "word2", "word3"])),
            Ok(CliCommand::Exec {
                role_name: None,
                engine: Engine::Codex,
                message: "word1 word2 word3".to_string(),
            })
        );
    }

    #[test]
    fn exec_usage_error_when_role_flag_present_but_missing_role_name_and_engine() {
        let error = parse_cli_command(&to_args(&["exec", "--role"])).unwrap_err();
        assert_eq!(error, CliError::ExecUsage);
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
    fn usage_error_when_explicit_role_launch_is_missing_engine() {
        let error = parse_cli_command(&to_args(&["--role", "director-of-engineering"])).unwrap_err();
        assert_eq!(error, CliError::Usage);
        assert_eq!(error.message(), GENERAL_USAGE);
    }

    #[test]
    fn prepare_launch_prompt_fails_when_explicit_role_is_missing() {
        let temp = TestDir::new("launch-missing-role");

        let err = prepare_launch_prompt(temp.path(), Some("missing-role"))
            .expect_err("missing role should fail");

        assert_eq!(
            err,
            "Role not found: missing-role. Run: juliet init --role missing-role"
        );
    }

    #[test]
    fn prepare_launch_prompt_rejects_explicit_role_traversal_name() {
        let temp = TestDir::new("launch-explicit-invalid-role-name");
        let escaped_role_name = "../escaped-role";
        let escaped_role_dir = temp.path().join("escaped-role");

        fs::create_dir_all(temp.path().join(".juliet"))
            .expect("state root should exist for traversal regression test");
        fs::create_dir_all(&escaped_role_dir)
            .expect("escaped role directory should exist outside .juliet");
        fs::write(escaped_role_dir.join("prompt.md"), "# escaped prompt")
            .expect("escaped prompt file should exist outside .juliet");

        let err = prepare_launch_prompt(temp.path(), Some(escaped_role_name))
            .expect_err("invalid explicit role name should fail before path traversal");

        assert_eq!(
            err,
            "Invalid role name: ../escaped-role. Use lowercase letters, numbers, and hyphens."
        );
        assert!(
            !escaped_role_dir.join("juliet-prompt.md").exists(),
            "runtime prompt should not be written outside .juliet/<role>/"
        );
    }

    #[test]
    fn prepare_launch_prompt_reads_and_stages_explicit_role_prompt() {
        let temp = TestDir::new("launch-explicit-role");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Explicit prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let prompt = prepare_launch_prompt(temp.path(), Some(role_name))
            .expect("explicit role prompt should be loaded");
        assert_eq!(prompt, "# Explicit prompt\n\nDo role work.");

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, prompt);
    }

    #[test]
    fn run_launch_command_in_dir_returns_engine_exit_code_for_explicit_role() {
        let temp = TestDir::new("launch-explicit-engine-exit");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Explicit prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_launch_command_in_dir(
            temp.path(),
            Some(role_name),
            Engine::Codex,
            None,
            |engine, prompt, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(cwd, temp.path());
                Ok(5)
            },
        );

        assert_eq!(exit_code, 5);
        assert_eq!(captured_engine, Some(Engine::Codex));
        assert_eq!(captured_prompt, "# Explicit prompt\n\nDo role work.");
    }

    #[test]
    fn prepare_launch_prompt_fails_when_implicit_launch_has_no_roles() {
        let temp = TestDir::new("launch-implicit-no-roles");
        let prompts_dir = temp.path().join("prompts");
        fs::create_dir_all(&prompts_dir).expect("prompts directory should be created");
        fs::write(prompts_dir.join("juliet.md"), "# legacy prompt")
            .expect("legacy prompt should not affect role discovery");

        let err =
            prepare_launch_prompt(temp.path(), None).expect_err("missing roles should fail launch");
        assert_eq!(err, NO_ROLES_CONFIGURED_ERROR);
    }

    #[test]
    fn prepare_launch_prompt_auto_selects_single_configured_role() {
        let temp = TestDir::new("launch-implicit-single-role");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Implicit prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let prompt = prepare_launch_prompt(temp.path(), None)
            .expect("single role should be selected implicitly");
        assert_eq!(prompt, "# Implicit prompt\n\nDo role work.");

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, prompt);
    }

    #[test]
    fn prepare_launch_prompt_auto_selects_single_juliet_role() {
        let temp = TestDir::new("launch-implicit-juliet-role");
        let role_name = "juliet";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Juliet role prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let prompt = prepare_launch_prompt(temp.path(), None)
            .expect("single juliet role should be selected implicitly");
        assert_eq!(prompt, "# Juliet role prompt\n\nDo role work.");

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, prompt);
    }

    #[test]
    fn prepare_launch_prompt_auto_selects_single_artifacts_role() {
        let temp = TestDir::new("launch-implicit-artifacts-role");
        let role_name = "artifacts";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Artifacts role prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let prompt = prepare_launch_prompt(temp.path(), None)
            .expect("single artifacts role should be selected implicitly");
        assert_eq!(prompt, "# Artifacts role prompt\n\nDo role work.");

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, prompt);
    }

    #[test]
    fn run_launch_command_in_dir_returns_engine_exit_code_for_implicit_single_role_launch() {
        let temp = TestDir::new("launch-implicit-engine-exit");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Implicit prompt\n\nDo role work.")
            .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_launch_command_in_dir(
            temp.path(),
            None,
            Engine::Claude,
            None,
            |engine, prompt, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(cwd, temp.path());
                Ok(5)
            },
        );

        assert_eq!(exit_code, 5);
        assert_eq!(captured_engine, Some(Engine::Claude));
        assert_eq!(captured_prompt, "# Implicit prompt\n\nDo role work.");
    }

    #[test]
    fn prepare_launch_prompt_fails_when_multiple_roles_are_configured() {
        let temp = TestDir::new("launch-implicit-multiple-roles");
        role_state::create_role_state(temp.path(), "zeta-team")
            .expect("zeta team role state should exist");
        role_state::create_role_state(temp.path(), "alpha-team")
            .expect("alpha team role state should exist");

        let err = prepare_launch_prompt(temp.path(), None)
            .expect_err("multiple configured roles should require explicit selection");
        assert_eq!(
            err,
            "Multiple roles found. Specify one with --role <name>:\nalpha-team\nzeta-team"
        );
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
        fs::create_dir_all(prompt_path.parent().expect("prompt parent dir should exist"))
            .expect("prompt parent directory should be created");
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
    fn initialize_role_scaffolds_missing_state_files_when_prompt_and_legacy_dir_exist() {
        let temp = TestDir::new("artifacts-prompt-and-legacy-dir");
        let role_name = "artifacts";
        let role_dir = role_state::role_state_dir(temp.path(), role_name);
        fs::create_dir_all(&role_dir).expect("legacy artifacts directory should be created");
        fs::write(role_dir.join("legacy-note.txt"), "legacy artifact")
            .expect("legacy artifacts file should be created");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# legacy artifacts prompt").expect("prompt should be created");

        let outcome = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("init should scaffold missing state files");
        assert_eq!(outcome, InitOutcome::Initialized);
        assert_eq!(
            fs::read_to_string(&prompt_path).expect("prompt should remain unchanged"),
            "# legacy artifacts prompt"
        );

        assert!(role_state::role_state_is_scaffolded(temp.path(), role_name));
        assert!(
            role_state::discover_configured_roles(temp.path())
                .expect("role discovery should succeed")
                .iter()
                .any(|role| role.name == role_name),
            "artifacts role should be discoverable after scaffolding"
        );
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

    #[test]
    fn build_launch_prompt_appends_operator_input() {
        let base = "# Role prompt\n\nDo role work.";
        assert_eq!(build_launch_prompt(base, None), base.to_string());
        assert_eq!(
            build_launch_prompt(base, Some("please continue from yesterday")),
            "# Role prompt\n\nDo role work.\n\nUser input:\nplease continue from yesterday"
                .to_string()
        );
    }

    // reset_prompt unit tests

    #[test]
    fn reset_prompt_rejects_invalid_role_name() {
        let temp = TestDir::new("reset-prompt-invalid-name");
        let err = reset_prompt(temp.path(), "Invalid_Name", "seed prompt")
            .expect_err("invalid role name should fail");

        assert_eq!(
            err,
            "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens."
        );
    }

    #[test]
    fn reset_prompt_fails_when_role_not_initialized() {
        let temp = TestDir::new("reset-prompt-not-initialized");
        let err = reset_prompt(temp.path(), "missing-role", "seed prompt")
            .expect_err("uninitialized role should fail");

        assert_eq!(err, "Role 'missing-role' is not initialized.");
    }

    #[test]
    fn reset_prompt_overwrites_prompt_with_default_template() {
        let temp = TestDir::new("reset-prompt-overwrite");
        let role_name = "director-of-engineering";
        let seed = "## Seeded prompt content";

        initialize_role(temp.path(), role_name, seed).expect("init should succeed");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Custom modified prompt\n\nUser changes here.")
            .expect("prompt should be writable");

        reset_prompt(temp.path(), role_name, seed).expect("reset_prompt should succeed");

        let prompt_contents =
            fs::read_to_string(&prompt_path).expect("prompt should be readable after reset");
        let expected = role_prompt_template(role_name, seed);
        assert_eq!(prompt_contents, expected);
        assert!(prompt_contents.contains(&format!("# {role_name}")));
        assert!(prompt_contents.contains(OPERATOR_PLACEHOLDER));
        assert!(prompt_contents.contains(seed));
    }

    #[test]
    fn reset_prompt_preserves_state_files() {
        let temp = TestDir::new("reset-prompt-preserves-state");
        let role_name = "operations";
        let seed = "## Seed";

        initialize_role(temp.path(), role_name, seed).expect("init should succeed");

        let session_path = role_state::role_state_dir(temp.path(), role_name).join("session.md");
        fs::write(&session_path, "important session data").expect("session should be writable");

        reset_prompt(temp.path(), role_name, seed).expect("reset_prompt should succeed");

        assert_eq!(
            fs::read_to_string(&session_path).expect("session should still exist"),
            "important session data"
        );
    }

    // clear_history unit tests

    #[test]
    fn clear_history_rejects_invalid_role_name() {
        let temp = TestDir::new("clear-history-invalid-name");
        let err = clear_history(temp.path(), "Invalid_Name")
            .expect_err("invalid role name should fail");

        assert_eq!(
            err,
            "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens."
        );
    }

    #[test]
    fn clear_history_fails_when_role_not_initialized() {
        let temp = TestDir::new("clear-history-not-initialized");
        let err = clear_history(temp.path(), "missing-role")
            .expect_err("uninitialized role should fail");

        assert_eq!(err, "Role 'missing-role' is not initialized.");
    }

    #[test]
    fn clear_history_empties_state_files() {
        let temp = TestDir::new("clear-history-empties-state");
        let role_name = "director-of-engineering";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let role_dir = role_state::role_state_dir(temp.path(), role_name);
        fs::write(role_dir.join("session.md"), "session data").expect("write session");
        fs::write(
            role_dir.join("needs-from-operator.md"),
            "operator needs",
        )
        .expect("write needs");
        fs::write(role_dir.join("projects.md"), "project data").expect("write projects");
        fs::write(role_dir.join("processes.md"), "process data").expect("write processes");

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert_eq!(
            fs::read_to_string(role_dir.join("session.md")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(role_dir.join("needs-from-operator.md")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(role_dir.join("projects.md")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(role_dir.join("processes.md")).unwrap(),
            ""
        );
    }

    #[test]
    fn clear_history_deletes_juliet_prompt_md() {
        let temp = TestDir::new("clear-history-deletes-runtime-prompt");
        let role_name = "operations";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let runtime_path = role_state::runtime_prompt_path(temp.path(), role_name);
        fs::write(&runtime_path, "runtime prompt content").expect("write runtime prompt");
        assert!(runtime_path.exists());

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert!(!runtime_path.exists(), "juliet-prompt.md should be deleted");
    }

    #[test]
    fn clear_history_succeeds_when_juliet_prompt_md_absent() {
        let temp = TestDir::new("clear-history-no-runtime-prompt");
        let role_name = "qa";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let runtime_path = role_state::runtime_prompt_path(temp.path(), role_name);
        assert!(!runtime_path.exists());

        clear_history(temp.path(), role_name).expect("clear_history should succeed without runtime prompt");
    }

    #[test]
    fn clear_history_clears_artifacts_directory_contents() {
        let temp = TestDir::new("clear-history-clears-artifacts");
        let role_name = "engineering";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let artifacts_dir = role_state::role_state_dir(temp.path(), role_name).join("artifacts");
        fs::write(artifacts_dir.join("report.txt"), "report content").expect("write artifact file");
        fs::create_dir_all(artifacts_dir.join("subdir")).expect("create artifact subdir");
        fs::write(artifacts_dir.join("subdir").join("nested.md"), "nested content")
            .expect("write nested artifact");

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert!(artifacts_dir.is_dir(), "artifacts directory should be preserved");
        assert_eq!(
            fs::read_dir(&artifacts_dir).unwrap().count(),
            0,
            "artifacts directory should be empty"
        );
    }

    #[test]
    fn clear_history_preserves_prompt_md() {
        let temp = TestDir::new("clear-history-preserves-prompt");
        let role_name = "director-of-marketing";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::write(&prompt_path, "# Custom prompt\n\nKeep this intact.")
            .expect("write custom prompt");

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert_eq!(
            fs::read_to_string(&prompt_path).unwrap(),
            "# Custom prompt\n\nKeep this intact."
        );
    }

    // exec command unit tests

    #[test]
    fn exec_explicit_role_stages_prompt_and_appends_message() {
        let temp = TestDir::new("exec-explicit-role");
        let role_name = "director-of-engineering";
        let role_prompt = "# Exec prompt\n\nDo role work.";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");
        fs::write(role_state::role_prompt_path(temp.path(), role_name), role_prompt)
            .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some(role_name),
            Engine::Codex,
            "fix the bug",
            |engine, prompt, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(cwd, temp.path());
                Ok(0)
            },
        );

        assert_eq!(exit_code, 0);
        assert_eq!(captured_engine, Some(Engine::Codex));
        assert_eq!(
            captured_prompt,
            "# Exec prompt\n\nDo role work.\n\nUser input:\nfix the bug"
        );

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, role_prompt);
    }

    #[test]
    fn exec_implicit_single_role_stages_prompt_and_appends_message() {
        let temp = TestDir::new("exec-implicit-single-role");
        let role_name = "director-of-engineering";
        let role_prompt = "# Implicit exec prompt\n\nDo role work.";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");
        fs::write(role_state::role_prompt_path(temp.path(), role_name), role_prompt)
            .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_exec_command_in_dir(
            temp.path(),
            None,
            Engine::Claude,
            "deploy the app",
            |engine, prompt, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(cwd, temp.path());
                Ok(0)
            },
        );

        assert_eq!(exit_code, 0);
        assert_eq!(captured_engine, Some(Engine::Claude));
        assert_eq!(
            captured_prompt,
            "# Implicit exec prompt\n\nDo role work.\n\nUser input:\ndeploy the app"
        );

        let runtime_prompt =
            fs::read_to_string(role_state::runtime_prompt_path(temp.path(), role_name))
                .expect("runtime prompt should be written");
        assert_eq!(runtime_prompt, role_prompt);
    }

    #[test]
    fn exec_returns_engine_exit_code() {
        let temp = TestDir::new("exec-engine-exit-code");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");
        fs::write(
            role_state::role_prompt_path(temp.path(), role_name),
            "# prompt",
        )
        .expect("role prompt should be written");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some(role_name),
            Engine::Claude,
            "hello",
            |_engine, _prompt, _cwd| Ok(42),
        );

        assert_eq!(exit_code, 42);
    }

    #[test]
    fn exec_fails_when_explicit_role_is_missing() {
        let temp = TestDir::new("exec-missing-role");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some("missing-role"),
            Engine::Codex,
            "hello",
            |_engine, _prompt, _cwd| Ok(0),
        );

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn exec_fails_when_no_roles_configured_for_implicit_discovery() {
        let temp = TestDir::new("exec-no-roles");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            None,
            Engine::Claude,
            "hello",
            |_engine, _prompt, _cwd| Ok(0),
        );

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn exec_fails_when_multiple_roles_configured_for_implicit_discovery() {
        let temp = TestDir::new("exec-multiple-roles");
        role_state::create_role_state(temp.path(), "alpha-team")
            .expect("alpha role state should exist");
        role_state::create_role_state(temp.path(), "zeta-team")
            .expect("zeta role state should exist");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            None,
            Engine::Codex,
            "hello",
            |_engine, _prompt, _cwd| Ok(0),
        );

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn exec_fails_when_engine_runner_returns_error() {
        let temp = TestDir::new("exec-engine-error");
        let role_name = "director-of-engineering";
        role_state::create_role_state(temp.path(), role_name).expect("role state should exist");
        fs::write(
            role_state::role_prompt_path(temp.path(), role_name),
            "# prompt",
        )
        .expect("role prompt should be written");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some(role_name),
            Engine::Claude,
            "hello",
            |_engine, _prompt, _cwd| {
                Err(io::Error::new(io::ErrorKind::NotFound, "engine not found"))
            },
        );

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn exec_rejects_invalid_explicit_role_name() {
        let temp = TestDir::new("exec-invalid-role-name");

        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some("../escaped-role"),
            Engine::Codex,
            "hello",
            |_engine, _prompt, _cwd| Ok(0),
        );

        assert_eq!(exit_code, 1);
    }

    #[cfg(unix)]
    mod cli_integration_tests {
        use super::*;
        use std::env;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::{Path, PathBuf};
        use std::process::Command;
        use std::sync::OnceLock;
        use std::time::{SystemTime, UNIX_EPOCH};

        struct CliOutput {
            exit_code: i32,
            stdout: String,
            stderr: String,
        }

        struct MockCodex {
            bin_dir: PathBuf,
            args_file: PathBuf,
            exit_code: i32,
        }

        impl MockCodex {
            fn new(root: &Path, exit_code: i32) -> Self {
                let bin_dir = root.join("mock-bin");
                fs::create_dir_all(&bin_dir).expect("mock bin directory should exist");

                let args_file = root.join("mock-codex-args.txt");
                let codex_path = bin_dir.join("codex");

                fs::write(
                    &codex_path,
                    r#"#!/usr/bin/env bash
set -eu
printf '%s\0' "$@" > "${JULIET_TEST_CODEX_ARGS_FILE:?}"
exit "${JULIET_TEST_CODEX_EXIT_CODE:-0}"
"#,
                )
                .expect("mock codex script should be writable");

                let mut permissions = fs::metadata(&codex_path)
                    .expect("mock codex script metadata should be readable")
                    .permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&codex_path, permissions)
                    .expect("mock codex script should be executable");

                Self {
                    bin_dir,
                    args_file,
                    exit_code,
                }
            }

            fn configure_command(&self, command: &mut Command) {
                let existing_path = env::var("PATH").unwrap_or_default();
                let path_value = if existing_path.is_empty() {
                    self.bin_dir.display().to_string()
                } else {
                    format!("{}:{existing_path}", self.bin_dir.display())
                };

                command.env("PATH", path_value);
                command.env("JULIET_TEST_CODEX_ARGS_FILE", &self.args_file);
                command.env(
                    "JULIET_TEST_CODEX_EXIT_CODE",
                    self.exit_code.to_string(),
                );
            }

            fn recorded_args(&self) -> Vec<String> {
                fs::read_to_string(&self.args_file)
                    .expect("mock codex args capture should be readable")
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }

            fn recorded_prompt(&self) -> String {
                let args = self.recorded_args();
                args.last()
                    .expect("mock codex should have received at least one argument")
                    .clone()
            }
        }

        fn cli_binary_path() -> &'static PathBuf {
            static CLI_BINARY: OnceLock<PathBuf> = OnceLock::new();
            CLI_BINARY.get_or_init(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time drift should not occur in tests")
                    .as_nanos();
                let build_root = env::temp_dir().join(format!(
                    "juliet-cli-integration-bin-{}-{timestamp}",
                    std::process::id()
                ));
                fs::create_dir_all(&build_root)
                    .expect("cli integration build directory should be created");

                let source_path = env::current_dir()
                    .expect("test process should have a current directory")
                    .join("juliet.rs");
                assert!(
                    source_path.is_file(),
                    "expected juliet.rs at {}",
                    source_path.display()
                );

                let binary_path = build_root.join("juliet-cli");
                let output = Command::new("rustc")
                    .arg(&source_path)
                    .arg("-o")
                    .arg(&binary_path)
                    .output()
                    .expect("rustc should be invokable for cli integration tests");

                if !output.status.success() {
                    panic!(
                        "failed to compile CLI binary\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                binary_path
            })
        }

        fn create_project_root(temp: &TestDir) -> PathBuf {
            let project_root = temp.path().join("project");
            fs::create_dir_all(&project_root).expect("project root should be created");
            // Canonicalize to resolve symlinks (e.g. macOS /var -> /private/var)
            // so path assertions match what the binary sees via current_dir().
            project_root
                .canonicalize()
                .expect("project root should be canonicalizable")
        }

        fn run_cli(
            project_root: &Path,
            args: &[&str],
            mock_codex: Option<&MockCodex>,
        ) -> CliOutput {
            let mut command = Command::new(cli_binary_path());
            command.args(args).current_dir(project_root);
            if let Some(mock) = mock_codex {
                mock.configure_command(&mut command);
            }

            let output = command.output().expect("CLI command should execute");
            let exit_code = output
                .status
                .code()
                .expect("CLI process should exit with an exit code");
            CliOutput {
                exit_code,
                stdout: String::from_utf8(output.stdout)
                    .expect("stdout should be valid UTF-8 in tests"),
                stderr: String::from_utf8(output.stderr)
                    .expect("stderr should be valid UTF-8 in tests"),
            }
        }

        #[test]
        fn cli_no_args_prints_general_usage_and_exits_with_code_one() {
            let temp = TestDir::new("integration-no-args");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &[], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, format!("{GENERAL_USAGE}\n"));
            assert_eq!(output.stderr, "");
        }

        #[test]
        fn cli_init_without_role_prints_init_usage_and_exits_with_code_one() {
            let temp = TestDir::new("integration-init-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["init"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, format!("{INIT_USAGE}\n"));
            assert_eq!(output.stderr, "");
        }

        #[test]
        fn cli_init_with_empty_role_name_prints_invalid_name_and_exits_with_code_one() {
            let temp = TestDir::new("integration-init-empty-role");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["init", "--role", ""], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Invalid role name: . Use lowercase letters, numbers, and hyphens.\n"
            );
        }

        #[test]
        fn cli_init_is_idempotent_with_exact_messages_and_exit_codes() {
            let temp = TestDir::new("integration-init-idempotent");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let first = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(first.exit_code, 0);
            assert_eq!(first.stdout, format!("Initialized role: {role_name}\n"));
            assert_eq!(first.stderr, "");

            let second = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(second.exit_code, 0);
            assert_eq!(second.stdout, format!("Role already exists: {role_name}\n"));
            assert_eq!(second.stderr, "");
        }

        #[test]
        fn cli_init_artifacts_scaffolds_legacy_directory_and_allows_implicit_launch() {
            let temp = TestDir::new("integration-init-artifacts-legacy-dir");
            let project_root = create_project_root(&temp);
            let role_name = "artifacts";
            let role_prompt = "# Legacy artifacts prompt\n\nUse legacy artifacts role.";

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::create_dir_all(prompt_path.parent().expect("prompts dir should exist"))
                .expect("prompts directory should be created");
            fs::write(&prompt_path, role_prompt).expect("legacy prompt should be writable");

            let legacy_role_dir = role_state::role_state_dir(&project_root, role_name);
            fs::create_dir_all(&legacy_role_dir).expect("legacy artifacts directory should exist");
            fs::write(legacy_role_dir.join("existing-artifact.md"), "legacy artifact")
                .expect("legacy artifact file should be writable");

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);
            assert_eq!(init.stdout, format!("Initialized role: {role_name}\n"));
            assert_eq!(init.stderr, "");
            assert!(role_state::role_state_is_scaffolded(&project_root, role_name));

            let mock_codex = MockCodex::new(temp.path(), 0);
            let launch = run_cli(&project_root, &["codex"], Some(&mock_codex));

            assert_eq!(launch.exit_code, 0);
            assert_eq!(launch.stdout, "");
            assert_eq!(launch.stderr, "");
            assert_eq!(mock_codex.recorded_prompt(), role_prompt);
            assert_eq!(
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable"),
                role_prompt
            );
        }

        #[test]
        fn cli_explicit_role_launch_stages_prompt_and_uses_engine_exit_code() {
            let temp = TestDir::new("integration-explicit-launch");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-marketing";
            let role_prompt = "# Explicit role prompt\n\nRun the explicit role workflow.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let role_prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&role_prompt_path, role_prompt).expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let launch = run_cli(
                &project_root,
                &["--role", role_name, "codex"],
                Some(&mock_codex),
            );

            assert_eq!(launch.exit_code, 0);
            assert_eq!(launch.stdout, "");
            assert_eq!(launch.stderr, "");
            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    role_prompt.to_string(),
                ]
            );
            assert_eq!(
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable"),
                role_prompt
            );
        }

        #[test]
        fn cli_explicit_role_launch_with_missing_role_prints_not_found_and_exits_with_code_one() {
            let temp = TestDir::new("integration-explicit-missing");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["--role", "missing-role", "codex"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Role not found: missing-role. Run: juliet init --role missing-role\n"
            );
        }

        #[test]
        fn cli_implicit_launch_with_no_roles_prints_no_roles_message_and_exits_with_code_one() {
            let temp = TestDir::new("integration-implicit-no-roles");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["codex"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(output.stderr, format!("{NO_ROLES_CONFIGURED_ERROR}\n"));
        }

        #[test]
        fn cli_implicit_launch_with_multiple_roles_prints_sorted_list_and_exits_with_code_one() {
            let temp = TestDir::new("integration-implicit-multi-role");
            let project_root = create_project_root(&temp);
            let first_role = "zeta-team";
            let second_role = "alpha-team";

            let first_init = run_cli(&project_root, &["init", "--role", first_role], None);
            assert_eq!(first_init.exit_code, 0);
            let second_init = run_cli(&project_root, &["init", "--role", second_role], None);
            assert_eq!(second_init.exit_code, 0);

            let output = run_cli(&project_root, &["codex"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Multiple roles found. Specify one with --role <name>:\nalpha-team\nzeta-team\n"
            );
        }

        #[test]
        fn cli_implicit_single_role_launch_auto_selects_role_and_exits_with_engine_code() {
            let temp = TestDir::new("integration-implicit-single-role");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Implicit role prompt\n\nRun the implicit role workflow.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);
            fs::write(role_state::role_prompt_path(&project_root, role_name), role_prompt)
                .expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let launch = run_cli(&project_root, &["codex"], Some(&mock_codex));

            assert_eq!(launch.exit_code, 0);
            assert_eq!(launch.stdout, "");
            assert_eq!(launch.stderr, "");
            assert_eq!(mock_codex.recorded_prompt(), role_prompt);
            assert_eq!(
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable"),
                role_prompt
            );
        }

        #[test]
        fn cli_implicit_single_artifacts_role_launch_auto_selects_role_and_exits_with_engine_code()
        {
            let temp = TestDir::new("integration-implicit-artifacts-role");
            let project_root = create_project_root(&temp);
            let role_name = "artifacts";
            let role_prompt = "# Artifacts role prompt\n\nRun the implicit role workflow.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);
            fs::write(role_state::role_prompt_path(&project_root, role_name), role_prompt)
                .expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let launch = run_cli(&project_root, &["codex"], Some(&mock_codex));

            assert_eq!(launch.exit_code, 0);
            assert_eq!(launch.stdout, "");
            assert_eq!(launch.stderr, "");
            assert_eq!(mock_codex.recorded_prompt(), role_prompt);
            assert_eq!(
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable"),
                role_prompt
            );
        }

        // reset-prompt integration tests

        #[test]
        fn cli_reset_prompt_overwrites_prompt_with_default_template_and_prints_success() {
            let temp = TestDir::new("integration-reset-prompt-success");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&prompt_path, "# Custom modified prompt\n\nUser edits here.")
                .expect("prompt should be writable");

            let output = run_cli(
                &project_root,
                &["reset-prompt", "--role", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("prompt reset to default for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");

            let prompt_contents = fs::read_to_string(&prompt_path)
                .expect("prompt should be readable after reset");
            assert!(
                prompt_contents.contains(&format!("# {role_name}")),
                "prompt should contain role heading"
            );
            assert!(
                prompt_contents.contains(OPERATOR_PLACEHOLDER),
                "prompt should contain operator placeholder"
            );
            assert!(
                prompt_contents.contains("## Default Prompt Seed"),
                "prompt should contain default prompt seed heading"
            );
            assert!(
                prompt_contents.contains(DEFAULT_PROMPT_SEED),
                "prompt should contain the default prompt seed content"
            );
        }

        #[test]
        fn cli_reset_prompt_fails_when_role_not_initialized() {
            let temp = TestDir::new("integration-reset-prompt-not-initialized");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["reset-prompt", "--role", "missing-role"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Role 'missing-role' is not initialized.\n"
            );
        }

        #[test]
        fn cli_reset_prompt_fails_with_invalid_role_name() {
            let temp = TestDir::new("integration-reset-prompt-invalid-name");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["reset-prompt", "--role", "Invalid_Name"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens.\n"
            );
        }

        #[test]
        fn cli_reset_prompt_without_role_prints_usage_and_exits_with_code_one() {
            let temp = TestDir::new("integration-reset-prompt-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["reset-prompt"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, format!("{RESET_PROMPT_USAGE}\n"));
            assert_eq!(output.stderr, "");
        }

        #[test]
        fn cli_reset_prompt_preserves_state_files() {
            let temp = TestDir::new("integration-reset-prompt-preserves-state");
            let project_root = create_project_root(&temp);
            let role_name = "operations";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let session_path =
                role_state::role_state_dir(&project_root, role_name).join("session.md");
            fs::write(&session_path, "important session data")
                .expect("session should be writable");

            let output = run_cli(
                &project_root,
                &["reset-prompt", "--role", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                fs::read_to_string(&session_path).expect("session should still exist"),
                "important session data"
            );
        }

        // clear-history integration tests

        #[test]
        fn cli_clear_history_empties_state_and_prints_success() {
            let temp = TestDir::new("integration-clear-history-success");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let role_dir = role_state::role_state_dir(&project_root, role_name);
            fs::write(role_dir.join("session.md"), "session data")
                .expect("session should be writable");
            fs::write(role_dir.join("needs-from-operator.md"), "operator needs")
                .expect("needs should be writable");
            fs::write(role_dir.join("projects.md"), "project data")
                .expect("projects should be writable");
            fs::write(role_dir.join("processes.md"), "process data")
                .expect("processes should be writable");

            let runtime_path = role_state::runtime_prompt_path(&project_root, role_name);
            fs::write(&runtime_path, "runtime prompt")
                .expect("runtime prompt should be writable");

            let artifacts_dir = role_dir.join("artifacts");
            fs::write(artifacts_dir.join("report.txt"), "report content")
                .expect("artifact should be writable");
            fs::create_dir_all(artifacts_dir.join("subdir"))
                .expect("artifact subdir should be created");
            fs::write(artifacts_dir.join("subdir").join("nested.md"), "nested")
                .expect("nested artifact should be writable");

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&prompt_path, "# Custom prompt\n\nKeep this.")
                .expect("prompt should be writable");

            let output = run_cli(
                &project_root,
                &["clear-history", "--role", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("history cleared for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");

            // State files should be empty
            assert_eq!(
                fs::read_to_string(role_dir.join("session.md")).unwrap(),
                ""
            );
            assert_eq!(
                fs::read_to_string(role_dir.join("needs-from-operator.md")).unwrap(),
                ""
            );
            assert_eq!(
                fs::read_to_string(role_dir.join("projects.md")).unwrap(),
                ""
            );
            assert_eq!(
                fs::read_to_string(role_dir.join("processes.md")).unwrap(),
                ""
            );

            // Runtime prompt should be deleted
            assert!(
                !runtime_path.exists(),
                "juliet-prompt.md should be deleted"
            );

            // Artifacts directory should be empty but still exist
            assert!(artifacts_dir.is_dir(), "artifacts directory should be preserved");
            assert_eq!(
                fs::read_dir(&artifacts_dir).unwrap().count(),
                0,
                "artifacts directory should be empty"
            );

            // prompt.md should be preserved
            assert_eq!(
                fs::read_to_string(&prompt_path).unwrap(),
                "# Custom prompt\n\nKeep this."
            );
        }

        #[test]
        fn cli_clear_history_fails_when_role_not_initialized() {
            let temp = TestDir::new("integration-clear-history-not-initialized");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["clear-history", "--role", "missing-role"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Role 'missing-role' is not initialized.\n"
            );
        }

        #[test]
        fn cli_clear_history_fails_with_invalid_role_name() {
            let temp = TestDir::new("integration-clear-history-invalid-name");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["clear-history", "--role", "Invalid_Name"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens.\n"
            );
        }

        #[test]
        fn cli_clear_history_without_role_prints_usage_and_exits_with_code_one() {
            let temp = TestDir::new("integration-clear-history-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["clear-history"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, format!("{CLEAR_HISTORY_USAGE}\n"));
            assert_eq!(output.stderr, "");
        }

        #[test]
        fn cli_clear_history_preserves_prompt_md() {
            let temp = TestDir::new("integration-clear-history-preserves-prompt");
            let project_root = create_project_root(&temp);
            let role_name = "operations";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&prompt_path, "# Custom operations prompt\n\nPreserve me.")
                .expect("prompt should be writable");

            let output = run_cli(
                &project_root,
                &["clear-history", "--role", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                fs::read_to_string(&prompt_path).unwrap(),
                "# Custom operations prompt\n\nPreserve me."
            );
        }

        #[test]
        fn cli_clear_history_succeeds_when_no_runtime_prompt_exists() {
            let temp = TestDir::new("integration-clear-history-no-runtime-prompt");
            let project_root = create_project_root(&temp);
            let role_name = "qa";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let runtime_path = role_state::runtime_prompt_path(&project_root, role_name);
            assert!(!runtime_path.exists());

            let output = run_cli(
                &project_root,
                &["clear-history", "--role", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("history cleared for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");
        }

        // exec integration tests

        #[test]
        fn cli_exec_explicit_role_stages_prompt_and_appends_message() {
            let temp = TestDir::new("integration-exec-explicit");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-marketing";
            let role_prompt = "# Exec role prompt\n\nRun the exec workflow.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let role_prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&role_prompt_path, role_prompt).expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &["exec", "--role", role_name, "codex", "fix", "the", "bug"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "");
            assert_eq!(output.stderr, "");

            let expected_prompt =
                format!("{role_prompt}\n\nUser input:\nfix the bug");
            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    expected_prompt.clone(),
                ]
            );

            let runtime_prompt =
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable");
            assert_eq!(runtime_prompt, role_prompt);
        }

        #[test]
        fn cli_exec_implicit_single_role_stages_prompt_and_appends_message() {
            let temp = TestDir::new("integration-exec-implicit");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Implicit exec prompt\n\nDo exec work.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);
            fs::write(role_state::role_prompt_path(&project_root, role_name), role_prompt)
                .expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &["exec", "codex", "deploy", "now"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "");
            assert_eq!(output.stderr, "");

            let expected_prompt =
                format!("{role_prompt}\n\nUser input:\ndeploy now");
            assert_eq!(mock_codex.recorded_prompt(), expected_prompt);

            let runtime_prompt =
                fs::read_to_string(role_state::runtime_prompt_path(&project_root, role_name))
                    .expect("runtime prompt should be readable");
            assert_eq!(runtime_prompt, role_prompt);
        }

        #[test]
        fn cli_exec_returns_engine_exit_code() {
            let temp = TestDir::new("integration-exec-exit-code");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let mock_codex = MockCodex::new(temp.path(), 7);
            let output = run_cli(
                &project_root,
                &["exec", "--role", role_name, "codex", "hello"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 7);
        }

        #[test]
        fn cli_exec_with_missing_role_prints_error_and_exits_with_code_one() {
            let temp = TestDir::new("integration-exec-missing-role");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["exec", "--role", "missing-role", "codex", "hello"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Role not found: missing-role. Run: juliet init --role missing-role\n"
            );
        }

        #[test]
        fn cli_exec_implicit_with_no_roles_prints_error_and_exits_with_code_one() {
            let temp = TestDir::new("integration-exec-no-roles");
            let project_root = create_project_root(&temp);

            let output = run_cli(
                &project_root,
                &["exec", "codex", "hello"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(output.stderr, format!("{NO_ROLES_CONFIGURED_ERROR}\n"));
        }

        #[test]
        fn cli_exec_implicit_with_multiple_roles_prints_error_and_exits_with_code_one() {
            let temp = TestDir::new("integration-exec-multi-roles");
            let project_root = create_project_root(&temp);

            let init1 = run_cli(&project_root, &["init", "--role", "alpha-team"], None);
            assert_eq!(init1.exit_code, 0);
            let init2 = run_cli(&project_root, &["init", "--role", "zeta-team"], None);
            assert_eq!(init2.exit_code, 0);

            let output = run_cli(
                &project_root,
                &["exec", "codex", "hello"],
                None,
            );

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Multiple roles found. Specify one with --role <name>:\nalpha-team\nzeta-team\n"
            );
        }
    }
}
