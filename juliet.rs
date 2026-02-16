use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

mod role_name;
mod role_state;

const DEFAULT_PROMPT_SEED: &str = include_str!("prompts/juliet.md");
const NO_ROLES_CONFIGURED_ERROR: &str = "No roles configured. Run: juliet init --project <name>";
const MULTIPLE_ROLES_FOUND_ERROR: &str = "Multiple roles found. Specify one with --project <name>:";
const OPERATOR_PLACEHOLDER: &str =
    "<!-- TODO: Replace with role-specific instructions and expected operator input. -->";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Engine {
    Claude,
    Codex,
}

impl Engine {
    fn as_str(self) -> &'static str {
        match self {
            Engine::Claude => "claude",
            Engine::Codex => "codex",
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
        continue_id: Option<String>,
        json_output: bool,
    },
}

#[derive(Debug, Eq, PartialEq)]
struct ExecResult {
    text: String,
    resume_id: String,
}

#[derive(Debug)]
struct EngineOutput {
    status_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitOutcome {
    Initialized,
    AlreadyExists,
}

#[derive(Debug, Args)]
struct ProjectArgs {
    /// Role name to target.
    #[arg(
        long = "project",
        visible_alias = "role",
        value_name = "ROLE_NAME",
        allow_hyphen_values = true
    )]
    role_name: String,
}

#[derive(Debug, Args)]
struct ExecArgs {
    /// Role name to target. If omitted, Juliet auto-selects when exactly one role exists.
    #[arg(
        long = "project",
        visible_alias = "role",
        value_name = "ROLE_NAME",
        allow_hyphen_values = true
    )]
    role_name: Option<String>,
    /// Continue a prior non-interactive thread/session id.
    #[arg(long = "continue", value_name = "RESUME_ID")]
    continue_id: Option<String>,
    /// Emit normalized JSON output for this exec turn.
    #[arg(long = "json")]
    json_output: bool,
    /// Engine to execute.
    engine: Engine,
    /// Message text appended to the prompt as user input.
    #[arg(
        required = true,
        num_args = 1..,
        value_name = "MESSAGE"
    )]
    message: Vec<String>,
}

#[derive(Debug, Parser)]
#[command(
    name = "juliet",
    about = "CLI API for project-scoped Juliet workflows",
    long_about = None,
    version
)]
struct JulietCli {
    #[command(subcommand)]
    command: Option<JulietSubcommand>,

    /// Role name to launch. If omitted, Juliet auto-selects when exactly one role exists.
    #[arg(
        long = "project",
        visible_alias = "role",
        value_name = "ROLE_NAME",
        allow_hyphen_values = true
    )]
    role_name: Option<String>,
    /// Engine to launch in interactive mode.
    engine: Option<Engine>,
    /// Optional operator input appended to the launch prompt.
    #[arg(
        num_args = 0..,
        trailing_var_arg = true,
        value_name = "OPERATOR_INPUT",
        allow_hyphen_values = true
    )]
    operator_input: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum JulietSubcommand {
    /// Initialize a new role scaffold.
    #[command(about = "Initialize a new role scaffold", long_about = None)]
    Init(ProjectArgs),
    /// Reset a role prompt to the default template.
    #[command(name = "reset-prompt")]
    #[command(about = "Reset a role prompt to the default template", long_about = None)]
    ResetPrompt(ProjectArgs),
    /// Clear role state/history while preserving prompt customization.
    #[command(name = "clear-history")]
    #[command(
        about = "Clear role state/history while preserving prompt customization",
        long_about = None
    )]
    ClearHistory(ProjectArgs),
    /// Execute a single non-interactive turn.
    #[command(about = "Execute a single non-interactive turn", long_about = None)]
    Exec(ExecArgs),
}

fn parse_with_clap<P>(args: &[String]) -> Result<P, clap::Error>
where
    P: Parser,
{
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("juliet".to_string());
    argv.extend(args.iter().cloned());
    P::try_parse_from(argv)
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, clap::Error> {
    let parsed = parse_with_clap::<JulietCli>(args)?;
    match parsed.command {
        Some(JulietSubcommand::Init(project)) => Ok(CliCommand::Init {
            role_name: project.role_name,
        }),
        Some(JulietSubcommand::ResetPrompt(project)) => Ok(CliCommand::ResetPrompt {
            role_name: project.role_name,
        }),
        Some(JulietSubcommand::ClearHistory(project)) => Ok(CliCommand::ClearHistory {
            role_name: project.role_name,
        }),
        Some(JulietSubcommand::Exec(exec)) => Ok(CliCommand::Exec {
            role_name: exec.role_name,
            engine: exec.engine,
            message: exec.message.join(" "),
            continue_id: exec.continue_id,
            json_output: exec.json_output,
        }),
        None => {
            let Some(engine) = parsed.engine else {
                return Err(JulietCli::command().error(
                    ErrorKind::MissingRequiredArgument,
                    "the following required arguments were not provided:\n  <ENGINE>",
                ));
            };
            Ok(CliCommand::Launch {
                role_name: parsed.role_name,
                engine,
                operator_input: parse_operator_input(&parsed.operator_input),
            })
        }
    }
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

fn command_output_to_engine_output(output: Output) -> EngineOutput {
    EngineOutput {
        status_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run_codex_exec_json(
    prompt: &str,
    continue_id: Option<&str>,
    cwd: &Path,
) -> io::Result<EngineOutput> {
    let mut command = Command::new("codex");
    command
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("exec");

    if let Some(resume_id) = continue_id {
        command.arg("resume").arg(resume_id);
    }

    let output = command
        .arg(prompt)
        .arg("--json")
        .current_dir(cwd)
        .output()?;
    Ok(command_output_to_engine_output(output))
}

fn run_claude_exec_json(
    prompt: &str,
    continue_id: Option<&str>,
    cwd: &Path,
) -> io::Result<EngineOutput> {
    let mut command = Command::new("claude");
    command.arg("--dangerously-skip-permissions");

    if let Some(resume_id) = continue_id {
        command.arg("--resume").arg(resume_id);
    }

    let output = command
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("json")
        .env("IS_SANDBOX", "1")
        .current_dir(cwd)
        .output()?;
    Ok(command_output_to_engine_output(output))
}

fn run_exec_engine(
    engine: Engine,
    prompt: &str,
    continue_id: Option<&str>,
    cwd: &Path,
) -> io::Result<EngineOutput> {
    match engine {
        Engine::Claude => run_claude_exec_json(prompt, continue_id, cwd),
        Engine::Codex => run_codex_exec_json(prompt, continue_id, cwd),
    }
}

fn parse_json_values(raw: &str) -> Vec<Value> {
    let mut values = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            values.push(value);
        }
    }

    if values.is_empty() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                values.push(value);
            }
        }
    }

    values
}

fn extract_text_candidate(value: &Value) -> Option<String> {
    for pointer in [
        "/item/text",
        "/text",
        "/result",
        "/output_text",
        "/message/text",
        "/content/0/text",
        "/message/content/0/text",
    ] {
        if let Some(text) = value.pointer(pointer).and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }

    if let Some(content) = value.get("content").and_then(Value::as_array) {
        for item in content {
            if let Some(text) = item.as_str() {
                return Some(text.to_string());
            }
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn parse_codex_exec_result(raw_stdout: &str) -> Result<ExecResult, String> {
    let values = parse_json_values(raw_stdout);
    if values.is_empty() {
        return Err("codex returned no parseable JSON output".to_string());
    }

    let mut resume_id = None;
    let mut text = None;
    for value in &values {
        if resume_id.is_none() {
            resume_id = value
                .get("thread_id")
                .and_then(Value::as_str)
                .map(|id| id.to_string());
        }

        if value.get("type").and_then(Value::as_str) == Some("item.completed") {
            if let Some(item_text) = value.pointer("/item/text").and_then(Value::as_str) {
                text = Some(item_text.to_string());
            }
        }
    }

    if text.is_none() {
        for value in &values {
            if let Some(candidate) = extract_text_candidate(value) {
                text = Some(candidate);
            }
        }
    }

    let resume_id =
        resume_id.ok_or_else(|| "codex JSON output did not include thread_id".to_string())?;
    Ok(ExecResult {
        text: text.unwrap_or_default(),
        resume_id,
    })
}

fn parse_claude_exec_result(raw_stdout: &str) -> Result<ExecResult, String> {
    let values = parse_json_values(raw_stdout);
    if values.is_empty() {
        return Err("claude returned no parseable JSON output".to_string());
    }

    let mut resume_id = None;
    let mut text = None;
    for value in &values {
        if resume_id.is_none() {
            resume_id = value
                .get("session_id")
                .and_then(Value::as_str)
                .map(|id| id.to_string());
        }
        if text.is_none() {
            text = extract_text_candidate(value);
        } else if let Some(candidate) = extract_text_candidate(value) {
            text = Some(candidate);
        }
    }

    let resume_id =
        resume_id.ok_or_else(|| "claude JSON output did not include session_id".to_string())?;
    Ok(ExecResult {
        text: text.unwrap_or_default(),
        resume_id,
    })
}

fn parse_exec_result(engine: Engine, raw_stdout: &str) -> Result<ExecResult, String> {
    match engine {
        Engine::Claude => parse_claude_exec_result(raw_stdout),
        Engine::Codex => parse_codex_exec_result(raw_stdout),
    }
}

fn format_exec_result_json(engine: Engine, result: &ExecResult) -> String {
    json!({
        "text": result.text,
        "resume_id": result.resume_id,
        "engine": engine.as_str(),
    })
    .to_string()
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
    let state_gitignore_path = role_state::state_gitignore_path(project_root);
    role_state::ensure_state_gitignore(project_root).map_err(|err| {
        format!(
            "failed to initialize state gitignore at {}: {err}",
            state_gitignore_path.display()
        )
    })?;
    let shared_learnings_path = role_state::shared_learnings_path(project_root);
    role_state::ensure_shared_learnings(project_root).map_err(|err| {
        format!(
            "failed to initialize shared learnings at {}: {err}",
            shared_learnings_path.display()
        )
    })?;

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

    role_state::clear_role_history(project_root, role_name)
        .map_err(|err| format!("failed to clear history for role {role_name}: {err}"))?;

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
            "Role not found: {role_name}. Run: juliet init --project {role_name}"
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

fn run_launch_command(
    role_name: Option<&str>,
    engine: Engine,
    operator_input: Option<&str>,
) -> i32 {
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
    continue_id: Option<&str>,
    json_output: bool,
    engine_runner: F,
) -> i32
where
    F: FnOnce(Engine, &str, Option<&str>, &Path) -> io::Result<EngineOutput>,
{
    let base_prompt = match prepare_launch_prompt(project_root, role_name) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let prompt = build_launch_prompt(&base_prompt, Some(message));

    match engine_runner(engine, &prompt, continue_id, project_root) {
        Ok(engine_output) => {
            if engine_output.status_code != 0 {
                if !engine_output.stderr.is_empty() {
                    eprint!("{}", engine_output.stderr);
                } else if !engine_output.stdout.is_empty() {
                    eprint!("{}", engine_output.stdout);
                }
                return engine_output.status_code;
            }

            let exec_result = match parse_exec_result(engine, &engine_output.stdout) {
                Ok(parsed) => parsed,
                Err(err) => {
                    eprintln!("failed to parse {} exec output: {err}", engine.as_str());
                    return 1;
                }
            };

            if json_output {
                println!("{}", format_exec_result_json(engine, &exec_result));
            } else if !exec_result.text.is_empty() {
                println!("{}", exec_result.text);
            }

            engine_output.status_code
        }
        Err(err) => {
            eprintln!("failed to run engine: {err}");
            1
        }
    }
}

fn run_exec_command(
    role_name: Option<&str>,
    engine: Engine,
    message: &str,
    continue_id: Option<&str>,
    json_output: bool,
) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    run_exec_command_in_dir(
        &cwd,
        role_name,
        engine,
        message,
        continue_id,
        json_output,
        run_exec_engine,
    )
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match parse_cli_command(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            let exit_code = err.exit_code();
            let _ = err.print();
            std::process::exit(exit_code);
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
            continue_id,
            json_output,
        } => run_exec_command(
            role_name.as_deref(),
            engine,
            &message,
            continue_id.as_deref(),
            json_output,
        ),
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
    fn parses_init_with_project() {
        let parsed = parse_cli_command(&to_args(&["init", "--project", "director-of-engineering"]))
            .expect("init parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::Init {
                role_name: "director-of-engineering".to_string()
            }
        );
    }

    #[test]
    fn parses_init_with_role_alias() {
        let parsed = parse_cli_command(&to_args(&["init", "--role", "director-of-engineering"]))
            .expect("init alias parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::Init {
                role_name: "director-of-engineering".to_string()
            }
        );
    }

    #[test]
    fn parses_explicit_role_launch_with_alias_and_operator_input() {
        let parsed = parse_cli_command(&to_args(&[
            "--role",
            "director-of-engineering",
            "codex",
            "continue",
            "project",
            "alpha",
        ]))
        .expect("explicit launch parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::Launch {
                role_name: Some("director-of-engineering".to_string()),
                engine: Engine::Codex,
                operator_input: Some("continue project alpha".to_string()),
            }
        );
    }

    #[test]
    fn parses_implicit_role_launch() {
        let parsed =
            parse_cli_command(&to_args(&["claude"])).expect("implicit launch parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::Launch {
                role_name: None,
                engine: Engine::Claude,
                operator_input: None,
            }
        );
    }

    #[test]
    fn parses_reset_prompt_with_alias() {
        let parsed = parse_cli_command(&to_args(&["reset-prompt", "--role", "ops"]))
            .expect("reset-prompt parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::ResetPrompt {
                role_name: "ops".to_string()
            }
        );
    }

    #[test]
    fn parses_clear_history_with_project() {
        let parsed = parse_cli_command(&to_args(&["clear-history", "--project", "qa-team"]))
            .expect("clear-history parse should succeed");
        assert_eq!(
            parsed,
            CliCommand::ClearHistory {
                role_name: "qa-team".to_string()
            }
        );
    }

    #[test]
    fn parses_exec_implicit_and_explicit() {
        let implicit = parse_cli_command(&to_args(&["exec", "claude", "do", "the", "thing"]))
            .expect("implicit exec parse should succeed");
        assert_eq!(
            implicit,
            CliCommand::Exec {
                role_name: None,
                engine: Engine::Claude,
                message: "do the thing".to_string(),
                continue_id: None,
                json_output: false,
            }
        );

        let explicit = parse_cli_command(&to_args(&[
            "exec", "--role", "my-role", "codex", "fix", "the", "bug",
        ]))
        .expect("explicit exec parse should succeed");
        assert_eq!(
            explicit,
            CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Codex,
                message: "fix the bug".to_string(),
                continue_id: None,
                json_output: false,
            }
        );
    }

    #[test]
    fn parses_exec_with_hyphen_prefixed_role_value() {
        let parsed = parse_cli_command(&to_args(&[
            "exec",
            "--project",
            "-leading",
            "claude",
            "hello",
        ]))
        .expect("exec parse should allow hyphen-prefixed role values");
        assert_eq!(
            parsed,
            CliCommand::Exec {
                role_name: Some("-leading".to_string()),
                engine: Engine::Claude,
                message: "hello".to_string(),
                continue_id: None,
                json_output: false,
            }
        );
    }

    #[test]
    fn parses_exec_continue_and_json_options() {
        let parsed = parse_cli_command(&to_args(&[
            "exec",
            "--project",
            "my-role",
            "--continue",
            "session-123",
            "--json",
            "codex",
            "ship",
            "it",
        ]))
        .expect("exec parse with continue/json options should succeed");
        assert_eq!(
            parsed,
            CliCommand::Exec {
                role_name: Some("my-role".to_string()),
                engine: Engine::Codex,
                message: "ship it".to_string(),
                continue_id: Some("session-123".to_string()),
                json_output: true,
            }
        );
    }

    #[test]
    fn parses_exec_json_option_after_message() {
        let parsed = parse_cli_command(&to_args(&["exec", "codex", "hello", "--json"]))
            .expect("exec parse should allow --json after message");
        assert_eq!(
            parsed,
            CliCommand::Exec {
                role_name: None,
                engine: Engine::Codex,
                message: "hello".to_string(),
                continue_id: None,
                json_output: true,
            }
        );
    }

    #[test]
    fn parser_errors_are_clap_native_for_invalid_shapes() {
        for args in [
            vec!["reset-prompt"],
            vec!["clear-history", "--role"],
            vec!["exec", "claude"],
            vec!["exec", "--role", "my-role", "claude"],
            vec!["exec", "--role"],
            vec!["exec", "--continue", "claude", "hello"],
            vec!["exec", "--continue"],
        ] {
            assert!(
                parse_cli_command(&to_args(&args)).is_err(),
                "expected parse error for args: {:?}",
                args
            );
        }
    }

    #[test]
    fn parser_requires_engine_for_launch_mode() {
        let error = parse_cli_command(&to_args(&[])).expect_err("no args should fail");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );

        let explicit_missing_engine = parse_cli_command(&to_args(&["--role", "director"]))
            .expect_err("explicit launch without engine should fail");
        assert_eq!(
            explicit_missing_engine.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn parser_rejects_invalid_exec_engine_values() {
        for invalid_engine in ["Claude", "CLAUDE", "Codex", "CODEX", "Claude3", "gpt4"] {
            let error = parse_cli_command(&to_args(&["exec", invalid_engine, "hello"]))
                .expect_err("invalid engine should fail");
            assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
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
    fn prepare_launch_prompt_fails_when_explicit_role_is_missing() {
        let temp = TestDir::new("launch-missing-role");

        let err = prepare_launch_prompt(temp.path(), Some("missing-role"))
            .expect_err("missing role should fail");

        assert_eq!(
            err,
            "Role not found: missing-role. Run: juliet init --project missing-role"
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
            "Multiple roles found. Specify one with --project <name>:\nalpha-team\nzeta-team"
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
        assert!(role_state::shared_learnings_path(temp.path()).is_file());
        let state_gitignore_path = role_state::state_gitignore_path(temp.path());
        let state_gitignore =
            fs::read_to_string(state_gitignore_path).expect("state gitignore should be readable");
        assert!(
            state_gitignore.contains("!*/prompt.md"),
            "state gitignore should keep role prompt files tracked"
        );
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
    fn initialize_role_repairs_missing_state_gitignore_even_when_role_already_exists() {
        let temp = TestDir::new("repair-state-gitignore");
        let role_name = "director-of-operations";

        let first = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("first init should succeed");
        assert_eq!(first, InitOutcome::Initialized);

        let state_gitignore_path = role_state::state_gitignore_path(temp.path());
        fs::remove_file(&state_gitignore_path).expect("state gitignore should be removable");

        let second = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("second init should still succeed");
        assert_eq!(second, InitOutcome::AlreadyExists);
        let state_gitignore =
            fs::read_to_string(state_gitignore_path).expect("state gitignore should be recreated");
        assert!(
            state_gitignore.contains("!*/prompt.md"),
            "state gitignore should keep role prompt files tracked"
        );
    }

    #[test]
    fn initialize_role_repairs_missing_shared_learnings_even_when_role_already_exists() {
        let temp = TestDir::new("repair-shared-learnings");
        let role_name = "director-of-operations";

        let first = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("first init should succeed");
        assert_eq!(first, InitOutcome::Initialized);

        let shared_learnings_path = role_state::shared_learnings_path(temp.path());
        fs::remove_file(&shared_learnings_path).expect("shared learnings should be removable");

        let second = initialize_role(temp.path(), role_name, "seed prompt")
            .expect("second init should still succeed");
        assert_eq!(second, InitOutcome::AlreadyExists);
        assert!(
            shared_learnings_path.is_file(),
            "shared learnings file should be recreated on repeated init"
        );
    }

    #[test]
    fn initialize_role_creates_missing_state_when_prompt_already_exists() {
        let temp = TestDir::new("prompt-only");
        let role_name = "operations";
        let prompt_path = role_state::role_prompt_path(temp.path(), role_name);
        fs::create_dir_all(
            prompt_path
                .parent()
                .expect("prompt parent dir should exist"),
        )
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
        assert!(role_state::shared_learnings_path(temp.path()).is_file());
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
        fs::write(
            &prompt_path,
            "# Custom modified prompt\n\nUser changes here.",
        )
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
        let err =
            clear_history(temp.path(), "Invalid_Name").expect_err("invalid role name should fail");

        assert_eq!(
            err,
            "Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens."
        );
    }

    #[test]
    fn clear_history_fails_when_role_not_initialized() {
        let temp = TestDir::new("clear-history-not-initialized");
        let err =
            clear_history(temp.path(), "missing-role").expect_err("uninitialized role should fail");

        assert_eq!(err, "Role 'missing-role' is not initialized.");
    }

    #[test]
    fn clear_history_empties_state_files() {
        let temp = TestDir::new("clear-history-empties-state");
        let role_name = "director-of-engineering";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let role_dir = role_state::role_state_dir(temp.path(), role_name);
        fs::write(role_dir.join("session.md"), "session data").expect("write session");
        fs::write(role_dir.join("needs-from-operator.md"), "operator needs").expect("write needs");
        fs::write(role_dir.join("projects.md"), "project data").expect("write projects");
        fs::write(role_dir.join("processes.md"), "process data").expect("write processes");
        let shared_learnings_path = role_state::shared_learnings_path(temp.path());
        fs::write(&shared_learnings_path, "learning data").expect("write shared learnings");

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert_eq!(fs::read_to_string(role_dir.join("session.md")).unwrap(), "");
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
        assert_eq!(
            fs::read_to_string(shared_learnings_path).unwrap(),
            "learning data"
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

        clear_history(temp.path(), role_name)
            .expect("clear_history should succeed without runtime prompt");
    }

    #[test]
    fn clear_history_clears_artifacts_directory_contents() {
        let temp = TestDir::new("clear-history-clears-artifacts");
        let role_name = "engineering";

        initialize_role(temp.path(), role_name, "seed").expect("init should succeed");

        let artifacts_dir = role_state::role_state_dir(temp.path(), role_name).join("artifacts");
        fs::write(artifacts_dir.join("report.txt"), "report content").expect("write artifact file");
        fs::create_dir_all(artifacts_dir.join("subdir")).expect("create artifact subdir");
        fs::write(
            artifacts_dir.join("subdir").join("nested.md"),
            "nested content",
        )
        .expect("write nested artifact");

        clear_history(temp.path(), role_name).expect("clear_history should succeed");

        assert!(
            artifacts_dir.is_dir(),
            "artifacts directory should be preserved"
        );
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
        fs::write(
            role_state::role_prompt_path(temp.path(), role_name),
            role_prompt,
        )
        .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_exec_command_in_dir(
            temp.path(),
            Some(role_name),
            Engine::Codex,
            "fix the bug",
            None,
            false,
            |engine, prompt, continue_id, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(continue_id, None);
                assert_eq!(cwd, temp.path());
                Ok(EngineOutput {
                    status_code: 0,
                    stdout:
                        "{\"thread_id\":\"thread-1\"}\n{\"type\":\"item.completed\",\"item\":{\"text\":\"done\"}}\n"
                            .to_string(),
                    stderr: String::new(),
                })
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
        fs::write(
            role_state::role_prompt_path(temp.path(), role_name),
            role_prompt,
        )
        .expect("role prompt should be written");

        let mut captured_engine = None;
        let mut captured_prompt = String::new();
        let exit_code = run_exec_command_in_dir(
            temp.path(),
            None,
            Engine::Claude,
            "deploy the app",
            None,
            false,
            |engine, prompt, continue_id, cwd| {
                captured_engine = Some(engine);
                captured_prompt = prompt.to_string();
                assert_eq!(continue_id, None);
                assert_eq!(cwd, temp.path());
                Ok(EngineOutput {
                    status_code: 0,
                    stdout: "{\"session_id\":\"session-1\",\"result\":\"done\"}\n".to_string(),
                    stderr: String::new(),
                })
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| {
                Ok(EngineOutput {
                    status_code: 42,
                    stdout: String::new(),
                    stderr: String::new(),
                })
            },
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| unreachable!("runner should not be called"),
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| unreachable!("runner should not be called"),
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| unreachable!("runner should not be called"),
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| {
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
            None,
            false,
            |_engine, _prompt, _continue_id, _cwd| unreachable!("runner should not be called"),
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
if [ "${2:-}" = "exec" ]; then
  if [ "${JULIET_TEST_CODEX_STDOUT:-}" != "" ]; then
    printf '%s' "${JULIET_TEST_CODEX_STDOUT}"
  else
    printf '%s\n' '{"thread_id":"codex-thread-id"}'
    printf '%s\n' '{"type":"item.completed","item":{"text":"codex mock response"}}'
  fi
fi
if [ "${JULIET_TEST_CODEX_STDERR:-}" != "" ]; then
  printf '%s' "${JULIET_TEST_CODEX_STDERR}" >&2
fi
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

        struct MockClaude {
            bin_dir: PathBuf,
            args_file: PathBuf,
            env_file: PathBuf,
            exit_code: i32,
        }

        impl MockClaude {
            fn new(root: &Path, exit_code: i32) -> Self {
                let bin_dir = root.join("mock-bin");
                fs::create_dir_all(&bin_dir).expect("mock bin directory should exist");

                let args_file = root.join("mock-claude-args.txt");
                let env_file = root.join("mock-claude-env.txt");
                let claude_path = bin_dir.join("claude");

                fs::write(
                    &claude_path,
                    r#"#!/usr/bin/env bash
set -eu
printf '%s\0' "$@" > "${JULIET_TEST_CLAUDE_ARGS_FILE:?}"
printf 'IS_SANDBOX=%s\n' "${IS_SANDBOX:-}" > "${JULIET_TEST_CLAUDE_ENV_FILE:?}"
emit_json="0"
for arg in "$@"; do
  if [ "$arg" = "--output-format" ]; then
    emit_json="1"
    break
  fi
done
if [ "$emit_json" = "1" ]; then
  if [ "${JULIET_TEST_CLAUDE_STDOUT:-}" != "" ]; then
    printf '%s' "${JULIET_TEST_CLAUDE_STDOUT}"
  else
    printf '%s\n' '{"session_id":"claude-session-id","result":"claude mock response"}'
  fi
fi
if [ "${JULIET_TEST_CLAUDE_STDERR:-}" != "" ]; then
  printf '%s' "${JULIET_TEST_CLAUDE_STDERR}" >&2
fi
exit "${JULIET_TEST_CLAUDE_EXIT_CODE:-0}"
"#,
                )
                .expect("mock claude script should be writable");

                let mut permissions = fs::metadata(&claude_path)
                    .expect("mock claude script metadata should be readable")
                    .permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&claude_path, permissions)
                    .expect("mock claude script should be executable");

                Self {
                    bin_dir,
                    args_file,
                    env_file,
                    exit_code,
                }
            }

            fn recorded_args(&self) -> Vec<String> {
                fs::read_to_string(&self.args_file)
                    .expect("mock claude args capture should be readable")
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }

            fn recorded_env(&self) -> String {
                fs::read_to_string(&self.env_file)
                    .expect("mock claude env capture should be readable")
            }
        }

        fn cli_binary_path() -> &'static PathBuf {
            static CLI_BINARY: OnceLock<PathBuf> = OnceLock::new();
            CLI_BINARY.get_or_init(|| {
                let manifest_dir =
                    env::current_dir().expect("test process should have a current directory");
                let output = Command::new("cargo")
                    .arg("build")
                    .arg("--bin")
                    .arg("juliet")
                    .arg("--quiet")
                    .current_dir(&manifest_dir)
                    .output()
                    .expect("cargo should be invokable for cli integration tests");

                if !output.status.success() {
                    panic!(
                        "failed to build CLI binary\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                let target_dir = env::var_os("CARGO_TARGET_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| manifest_dir.join("target"));
                let binary_path = target_dir.join("debug").join("juliet");
                assert!(
                    binary_path.is_file(),
                    "expected juliet binary at {}",
                    binary_path.display()
                );
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
            run_cli_with_engines(project_root, args, mock_codex, None)
        }

        fn run_cli_with_engines(
            project_root: &Path,
            args: &[&str],
            mock_codex: Option<&MockCodex>,
            mock_claude: Option<&MockClaude>,
        ) -> CliOutput {
            let mut command = Command::new(cli_binary_path());
            command.args(args).current_dir(project_root);

            // Collect PATH components from mocks
            let existing_path = env::var("PATH").unwrap_or_default();
            let mut path_dirs: Vec<String> = Vec::new();

            if let Some(mock) = mock_codex {
                path_dirs.push(mock.bin_dir.display().to_string());
                command.env("JULIET_TEST_CODEX_ARGS_FILE", &mock.args_file);
                command.env("JULIET_TEST_CODEX_EXIT_CODE", mock.exit_code.to_string());
            }

            if let Some(mock) = mock_claude {
                if !path_dirs
                    .iter()
                    .any(|d| d == &mock.bin_dir.display().to_string())
                {
                    path_dirs.push(mock.bin_dir.display().to_string());
                }
                command.env("JULIET_TEST_CLAUDE_ARGS_FILE", &mock.args_file);
                command.env("JULIET_TEST_CLAUDE_ENV_FILE", &mock.env_file);
                command.env("JULIET_TEST_CLAUDE_EXIT_CODE", mock.exit_code.to_string());
            }

            if !path_dirs.is_empty() {
                if !existing_path.is_empty() {
                    path_dirs.push(existing_path);
                }
                command.env("PATH", path_dirs.join(":"));
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
        fn cli_help_uses_standard_clap_format() {
            let temp = TestDir::new("integration-help");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["--help"], None);

            assert_eq!(output.exit_code, 0);
            assert!(output.stdout.contains("Usage: juliet"));
            assert!(output.stdout.contains("Commands:"));
            assert!(output.stdout.contains("init"));
            assert!(output.stdout.contains("Initialize a new role scaffold"));
            assert!(output
                .stdout
                .contains("Reset a role prompt to the default template"));
            assert!(output.stdout.contains("Clear role state/history"));
            assert!(output.stdout.contains("exec"));
            assert!(output
                .stdout
                .contains("Execute a single non-interactive turn"));
            assert!(output.stdout.contains("-h, --help"));
            assert!(output.stdout.contains("-V, --version"));
            assert!(output.stdout.contains("--project <ROLE_NAME>"));
            assert_eq!(output.stderr, "");
        }

        #[test]
        fn cli_subcommand_help_includes_argument_annotations() {
            let temp = TestDir::new("integration-subcommand-help");
            let project_root = create_project_root(&temp);

            let exec_help = run_cli(&project_root, &["exec", "--help"], None);
            assert_eq!(exec_help.exit_code, 0);
            assert!(exec_help
                .stdout
                .contains("Execute a single non-interactive turn"));
            assert!(exec_help.stdout.contains("--project <ROLE_NAME>"));
            assert!(exec_help
                .stdout
                .contains("Message text appended to the prompt"));

            let launch_help = run_cli(&project_root, &["--help"], None);
            assert!(launch_help
                .stdout
                .contains("Engine to launch in interactive mode"));
            assert!(launch_help
                .stdout
                .contains("Optional operator input appended to the launch prompt"));
        }

        #[test]
        fn cli_no_args_prints_clap_error_and_usage() {
            let temp = TestDir::new("integration-no-args");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &[], None);

            assert_eq!(output.exit_code, 2);
            assert_eq!(output.stdout, "");
            assert!(output.stderr.contains("error:"));
            assert!(output
                .stderr
                .contains("required arguments were not provided"));
            assert!(output.stderr.contains("<ENGINE>"));
            assert!(output.stderr.contains("Usage: juliet"));
        }

        #[test]
        fn cli_init_without_role_prints_clap_usage_and_exits_with_code_two() {
            let temp = TestDir::new("integration-init-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["init"], None);

            assert_eq!(output.exit_code, 2);
            assert_eq!(output.stdout, "");
            assert!(output.stderr.contains("error:"));
            assert!(output
                .stderr
                .contains("Usage: juliet init --project <ROLE_NAME>"));
        }

        #[test]
        fn cli_init_with_empty_role_name_prints_invalid_name_and_exits_with_code_one() {
            let temp = TestDir::new("integration-init-empty-role");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["init", "--project", ""], None);

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

            let first = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(first.exit_code, 0);
            assert_eq!(first.stdout, format!("Initialized role: {role_name}\n"));
            assert_eq!(first.stderr, "");
            let state_gitignore_path = role_state::state_gitignore_path(&project_root);
            let first_state_gitignore = fs::read_to_string(&state_gitignore_path)
                .expect("state gitignore should exist after init");
            assert!(
                first_state_gitignore.contains("!*/prompt.md"),
                "state gitignore should keep role prompt files tracked"
            );

            fs::remove_file(&state_gitignore_path)
                .expect("state gitignore should be removable for repair test");

            let second = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(second.exit_code, 0);
            assert_eq!(second.stdout, format!("Role already exists: {role_name}\n"));
            assert_eq!(second.stderr, "");
            let second_state_gitignore = fs::read_to_string(state_gitignore_path)
                .expect("state gitignore should be recreated on repeated init");
            assert!(
                second_state_gitignore.contains("!*/prompt.md"),
                "state gitignore should keep role prompt files tracked"
            );
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
            fs::write(
                legacy_role_dir.join("existing-artifact.md"),
                "legacy artifact",
            )
            .expect("legacy artifact file should be writable");

            let init = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(init.exit_code, 0);
            assert_eq!(init.stdout, format!("Initialized role: {role_name}\n"));
            assert_eq!(init.stderr, "");
            assert!(role_state::role_state_is_scaffolded(
                &project_root,
                role_name
            ));

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
                &["--project", role_name, "codex"],
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
                "Role not found: missing-role. Run: juliet init --project missing-role\n"
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
                "Multiple roles found. Specify one with --project <name>:\nalpha-team\nzeta-team\n"
            );
        }

        #[test]
        fn cli_implicit_single_role_launch_auto_selects_role_and_exits_with_engine_code() {
            let temp = TestDir::new("integration-implicit-single-role");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Implicit role prompt\n\nRun the implicit role workflow.";

            let init = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(init.exit_code, 0);
            fs::write(
                role_state::role_prompt_path(&project_root, role_name),
                role_prompt,
            )
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
            fs::write(
                role_state::role_prompt_path(&project_root, role_name),
                role_prompt,
            )
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
                &["reset-prompt", "--project", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("prompt reset to default for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");

            let prompt_contents =
                fs::read_to_string(&prompt_path).expect("prompt should be readable after reset");
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
            assert_eq!(output.stderr, "Role 'missing-role' is not initialized.\n");
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
        fn cli_reset_prompt_without_role_prints_clap_usage_and_exits_with_code_two() {
            let temp = TestDir::new("integration-reset-prompt-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["reset-prompt"], None);

            assert_eq!(output.exit_code, 2);
            assert_eq!(output.stdout, "");
            assert!(output.stderr.contains("error:"));
            assert!(output
                .stderr
                .contains("Usage: juliet reset-prompt --project <ROLE_NAME>"));
        }

        #[test]
        fn cli_reset_prompt_preserves_state_files() {
            let temp = TestDir::new("integration-reset-prompt-preserves-state");
            let project_root = create_project_root(&temp);
            let role_name = "operations";

            let init = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(init.exit_code, 0);

            let session_path =
                role_state::role_state_dir(&project_root, role_name).join("session.md");
            fs::write(&session_path, "important session data").expect("session should be writable");

            let output = run_cli(&project_root, &["reset-prompt", "--role", role_name], None);

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                fs::read_to_string(&session_path).expect("session should still exist"),
                "important session data"
            );
        }

        #[test]
        fn cli_reset_prompt_restores_exact_init_template() {
            let temp = TestDir::new("integration-reset-prompt-exact-template");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            let original_prompt =
                fs::read_to_string(&prompt_path).expect("init should create prompt.md");

            fs::write(
                &prompt_path,
                "# Completely replaced prompt\n\nNothing from before.",
            )
            .expect("prompt should be writable");

            let output = run_cli(&project_root, &["reset-prompt", "--role", role_name], None);

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("prompt reset to default for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");

            let reset_prompt =
                fs::read_to_string(&prompt_path).expect("prompt should be readable after reset");
            assert_eq!(
                reset_prompt, original_prompt,
                "reset-prompt should restore the exact same content as init"
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
            let shared_learnings_path = role_state::shared_learnings_path(&project_root);
            fs::write(&shared_learnings_path, "learning data")
                .expect("shared learnings should be writable");

            let runtime_path = role_state::runtime_prompt_path(&project_root, role_name);
            fs::write(&runtime_path, "runtime prompt").expect("runtime prompt should be writable");

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
                &["clear-history", "--project", role_name],
                None,
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(
                output.stdout,
                format!("history cleared for role '{role_name}'\n")
            );
            assert_eq!(output.stderr, "");

            // State files should be empty
            assert_eq!(fs::read_to_string(role_dir.join("session.md")).unwrap(), "");
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
            assert_eq!(
                fs::read_to_string(shared_learnings_path).unwrap(),
                "learning data"
            );

            // Runtime prompt should be deleted
            assert!(!runtime_path.exists(), "juliet-prompt.md should be deleted");

            // Artifacts directory should be empty but still exist
            assert!(
                artifacts_dir.is_dir(),
                "artifacts directory should be preserved"
            );
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
            assert_eq!(output.stderr, "Role 'missing-role' is not initialized.\n");
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
        fn cli_clear_history_without_role_prints_clap_usage_and_exits_with_code_two() {
            let temp = TestDir::new("integration-clear-history-usage");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["clear-history"], None);

            assert_eq!(output.exit_code, 2);
            assert_eq!(output.stdout, "");
            assert!(output.stderr.contains("error:"));
            assert!(output
                .stderr
                .contains("Usage: juliet clear-history --project <ROLE_NAME>"));
        }

        #[test]
        fn cli_clear_history_preserves_prompt_md() {
            let temp = TestDir::new("integration-clear-history-preserves-prompt");
            let project_root = create_project_root(&temp);
            let role_name = "operations";

            let init = run_cli(&project_root, &["init", "--project", role_name], None);
            assert_eq!(init.exit_code, 0);

            let prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&prompt_path, "# Custom operations prompt\n\nPreserve me.")
                .expect("prompt should be writable");

            let output = run_cli(&project_root, &["clear-history", "--role", role_name], None);

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

            let output = run_cli(&project_root, &["clear-history", "--role", role_name], None);

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
                &["exec", "--project", role_name, "codex", "fix", "the", "bug"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "codex mock response\n");
            assert_eq!(output.stderr, "");

            let expected_prompt = format!("{role_prompt}\n\nUser input:\nfix the bug");
            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    "exec".to_string(),
                    expected_prompt,
                    "--json".to_string(),
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
            fs::write(
                role_state::role_prompt_path(&project_root, role_name),
                role_prompt,
            )
            .expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &["exec", "codex", "deploy", "now"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "codex mock response\n");
            assert_eq!(output.stderr, "");

            let expected_prompt = format!("{role_prompt}\n\nUser input:\ndeploy now");
            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    "exec".to_string(),
                    expected_prompt,
                    "--json".to_string(),
                ]
            );

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
                "Role not found: missing-role. Run: juliet init --project missing-role\n"
            );
        }

        #[test]
        fn cli_exec_implicit_with_no_roles_prints_error_and_exits_with_code_one() {
            let temp = TestDir::new("integration-exec-no-roles");
            let project_root = create_project_root(&temp);

            let output = run_cli(&project_root, &["exec", "codex", "hello"], None);

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

            let output = run_cli(&project_root, &["exec", "codex", "hello"], None);

            assert_eq!(output.exit_code, 1);
            assert_eq!(output.stdout, "");
            assert_eq!(
                output.stderr,
                "Multiple roles found. Specify one with --project <name>:\nalpha-team\nzeta-team\n"
            );
        }

        #[test]
        fn cli_exec_claude_uses_print_flag_and_sandbox_env() {
            let temp = TestDir::new("integration-exec-claude-print");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Claude exec prompt\n\nRun claude exec.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let role_prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&role_prompt_path, role_prompt).expect("role prompt should be writable");

            let mock_claude = MockClaude::new(temp.path(), 0);
            let output = run_cli_with_engines(
                &project_root,
                &["exec", "--role", role_name, "claude", "do", "the", "thing"],
                None,
                Some(&mock_claude),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "claude mock response\n");
            assert_eq!(output.stderr, "");

            let expected_prompt = format!("{role_prompt}\n\nUser input:\ndo the thing");
            assert_eq!(
                mock_claude.recorded_args(),
                vec![
                    "--dangerously-skip-permissions".to_string(),
                    "-p".to_string(),
                    expected_prompt,
                    "--output-format".to_string(),
                    "json".to_string(),
                ]
            );

            let env_output = mock_claude.recorded_env();
            assert!(
                env_output.contains("IS_SANDBOX=1"),
                "IS_SANDBOX env var should be set to 1, got: {env_output}"
            );
        }

        #[test]
        fn cli_exec_claude_returns_engine_exit_code() {
            let temp = TestDir::new("integration-exec-claude-exit-code");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let mock_claude = MockClaude::new(temp.path(), 3);
            let output = run_cli_with_engines(
                &project_root,
                &["exec", "--role", role_name, "claude", "hello"],
                None,
                Some(&mock_claude),
            );

            assert_eq!(output.exit_code, 3);
        }

        #[test]
        fn cli_exec_codex_uses_exec_json_flag() {
            let temp = TestDir::new("integration-exec-codex-json");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Codex exec prompt\n\nRun codex exec.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let role_prompt_path = role_state::role_prompt_path(&project_root, role_name);
            fs::write(&role_prompt_path, role_prompt).expect("role prompt should be writable");

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &["exec", "--role", role_name, "codex", "fix", "bug"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "codex mock response\n");
            assert_eq!(output.stderr, "");

            let expected_prompt = format!("{role_prompt}\n\nUser input:\nfix bug");
            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    "exec".to_string(),
                    expected_prompt,
                    "--json".to_string(),
                ]
            );
        }

        #[test]
        fn cli_exec_claude_implicit_role_uses_print_flag() {
            let temp = TestDir::new("integration-exec-claude-implicit");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";
            let role_prompt = "# Claude implicit exec\n\nDo work.";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);
            fs::write(
                role_state::role_prompt_path(&project_root, role_name),
                role_prompt,
            )
            .expect("role prompt should be writable");

            let mock_claude = MockClaude::new(temp.path(), 0);
            let output = run_cli_with_engines(
                &project_root,
                &["exec", "claude", "deploy", "now"],
                None,
                Some(&mock_claude),
            );

            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "claude mock response\n");
            assert_eq!(output.stderr, "");

            let expected_prompt = format!("{role_prompt}\n\nUser input:\ndeploy now");
            assert_eq!(
                mock_claude.recorded_args(),
                vec![
                    "--dangerously-skip-permissions".to_string(),
                    "-p".to_string(),
                    expected_prompt,
                    "--output-format".to_string(),
                    "json".to_string(),
                ]
            );

            let env_output = mock_claude.recorded_env();
            assert!(
                env_output.contains("IS_SANDBOX=1"),
                "IS_SANDBOX env var should be set to 1, got: {env_output}"
            );
        }

        #[test]
        fn cli_exec_continue_uses_codex_resume_syntax() {
            let temp = TestDir::new("integration-exec-codex-continue");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &[
                    "exec",
                    "--role",
                    role_name,
                    "--continue",
                    "thread-123",
                    "codex",
                    "hello",
                ],
                Some(&mock_codex),
            );
            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "codex mock response\n");
            assert_eq!(output.stderr, "");

            assert_eq!(
                mock_codex.recorded_args(),
                vec![
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    "exec".to_string(),
                    "resume".to_string(),
                    "thread-123".to_string(),
                    format!(
                        "{}\n\nUser input:\nhello",
                        fs::read_to_string(role_state::role_prompt_path(&project_root, role_name))
                            .expect("role prompt should be readable")
                    ),
                    "--json".to_string(),
                ]
            );
        }

        #[test]
        fn cli_exec_continue_uses_claude_resume_syntax() {
            let temp = TestDir::new("integration-exec-claude-continue");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let mock_claude = MockClaude::new(temp.path(), 0);
            let output = run_cli_with_engines(
                &project_root,
                &[
                    "exec",
                    "--role",
                    role_name,
                    "--continue",
                    "session-456",
                    "claude",
                    "hello",
                ],
                None,
                Some(&mock_claude),
            );
            assert_eq!(output.exit_code, 0);
            assert_eq!(output.stdout, "claude mock response\n");
            assert_eq!(output.stderr, "");

            assert_eq!(
                mock_claude.recorded_args(),
                vec![
                    "--dangerously-skip-permissions".to_string(),
                    "--resume".to_string(),
                    "session-456".to_string(),
                    "-p".to_string(),
                    format!(
                        "{}\n\nUser input:\nhello",
                        fs::read_to_string(role_state::role_prompt_path(&project_root, role_name))
                            .expect("role prompt should be readable")
                    ),
                    "--output-format".to_string(),
                    "json".to_string(),
                ]
            );
        }

        #[test]
        fn cli_exec_json_outputs_normalized_envelope() {
            let temp = TestDir::new("integration-exec-json-envelope");
            let project_root = create_project_root(&temp);
            let role_name = "director-of-engineering";

            let init = run_cli(&project_root, &["init", "--role", role_name], None);
            assert_eq!(init.exit_code, 0);

            let mock_codex = MockCodex::new(temp.path(), 0);
            let output = run_cli(
                &project_root,
                &["exec", "--json", "--role", role_name, "codex", "hello"],
                Some(&mock_codex),
            );

            assert_eq!(output.exit_code, 0);
            let payload: Value =
                serde_json::from_str(output.stdout.trim()).expect("stdout should be valid JSON");
            assert_eq!(payload["text"], "codex mock response");
            assert_eq!(payload["resume_id"], "codex-thread-id");
            assert_eq!(payload["engine"], "codex");
            assert_eq!(output.stderr, "");
        }
    }
}
