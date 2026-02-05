use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

mod role_name;
mod role_state;

const GENERAL_USAGE: &str = "Usage: juliet <command> [options]\nCommands:\n  juliet init --role <name>\n  juliet --role <name> <claude|codex>\n  juliet <claude|codex>";
const INIT_USAGE: &str = "Usage: juliet init --role <name>";
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

fn run_launch_command(role_name: Option<&str>, engine: Engine, operator_input: Option<&str>) -> i32 {
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("failed to get current directory: {err}");
            return 1;
        }
    };

    let prompt = match prepare_launch_prompt(&cwd, role_name) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let prompt = build_launch_prompt(&prompt, operator_input);

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
        CliCommand::Launch {
            role_name,
            engine,
            operator_input,
        } => run_launch_command(role_name.as_deref(), engine, operator_input.as_deref()),
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
        fs::create_dir_all(temp.path().join("prompts"))
            .expect("prompts root should exist for traversal regression test");
        fs::create_dir_all(&escaped_role_dir)
            .expect("escaped role directory should exist outside .juliet");
        fs::write(temp.path().join("escaped-role.md"), "# escaped prompt")
            .expect("escaped prompt file should exist outside prompts");

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
        fs::create_dir_all(prompt_path.parent().expect("prompts dir should exist"))
            .expect("prompts directory should be created");
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
        fs::create_dir_all(prompt_path.parent().expect("prompts dir should exist"))
            .expect("prompts directory should be created");
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
}
