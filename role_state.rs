#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const JULIET_STATE_DIR: &str = ".juliet";
const PROMPTS_DIR: &str = "prompts";
const ARTIFACTS_DIR: &str = "artifacts";
const LEGACY_PROMPT_ROLE_NAME: &str = "juliet";
const RUNTIME_PROMPT_FILE: &str = "juliet-prompt.md";
const STATE_FILES: [&str; 4] = [
    "session.md",
    "needs-from-operator.md",
    "projects.md",
    "processes.md",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredRole {
    pub name: String,
    pub prompt_path: PathBuf,
}

pub fn role_state_dir(project_root: &Path, role_name: &str) -> PathBuf {
    project_root.join(JULIET_STATE_DIR).join(role_name)
}

pub fn role_prompt_path(project_root: &Path, role_name: &str) -> PathBuf {
    project_root
        .join(PROMPTS_DIR)
        .join(format!("{role_name}.md"))
}

pub fn runtime_prompt_path(project_root: &Path, role_name: &str) -> PathBuf {
    role_state_dir(project_root, role_name).join(RUNTIME_PROMPT_FILE)
}

pub fn role_state_exists(project_root: &Path, role_name: &str) -> bool {
    role_state_dir(project_root, role_name).is_dir()
}

pub fn create_role_state(project_root: &Path, role_name: &str) -> io::Result<()> {
    let role_dir = role_state_dir(project_root, role_name);
    fs::create_dir_all(role_dir.join(ARTIFACTS_DIR))?;

    for file in STATE_FILES {
        ensure_file(&role_dir.join(file))?;
    }

    Ok(())
}

pub fn discover_configured_roles(project_root: &Path) -> io::Result<Vec<ConfiguredRole>> {
    let state_root = project_root.join(JULIET_STATE_DIR);
    let entries = match fs::read_dir(state_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    let mut roles = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let role_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if role_name == ARTIFACTS_DIR || role_name == LEGACY_PROMPT_ROLE_NAME {
            continue;
        }

        roles.push(ConfiguredRole {
            prompt_path: role_prompt_path(project_root, &role_name),
            name: role_name,
        });
    }

    roles.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(roles)
}

pub fn write_runtime_prompt(project_root: &Path, role_name: &str, prompt: &str) -> io::Result<()> {
    let role_dir = role_state_dir(project_root, role_name);
    if !role_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("role state directory not found: {}", role_dir.display()),
        ));
    }

    fs::write(runtime_prompt_path(project_root, role_name), prompt)
}

fn ensure_file(path: &Path) -> io::Result<()> {
    if path.exists() {
        if path.is_file() {
            return Ok(());
        }

        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("expected file path, found non-file: {}", path.display()),
        ));
    }

    fs::File::create(path).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
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
            let path = env::temp_dir().join(format!(
                "juliet-role-state-{name}-{}-{timestamp}",
                process::id()
            ));
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

    #[test]
    fn create_role_state_builds_expected_layout() {
        let temp = TestDir::new("layout");
        let role_name = "director-of-engineering";

        create_role_state(temp.path(), role_name).expect("role state should be scaffolded");

        let role_dir = role_state_dir(temp.path(), role_name);
        assert!(role_dir.is_dir());
        assert!(role_state_exists(temp.path(), role_name));
        assert!(role_dir.join(ARTIFACTS_DIR).is_dir());

        for file in STATE_FILES {
            assert!(
                role_dir.join(file).is_file(),
                "missing state file: {}",
                file
            );
        }

        assert_eq!(
            runtime_prompt_path(temp.path(), role_name),
            role_dir.join(RUNTIME_PROMPT_FILE)
        );
        assert!(!runtime_prompt_path(temp.path(), role_name).exists());
    }

    #[test]
    fn create_role_state_is_idempotent_and_preserves_file_contents() {
        let temp = TestDir::new("idempotent");
        let role_name = "director-of-marketing";

        create_role_state(temp.path(), role_name).expect("initial scaffold should succeed");
        let session_path = role_state_dir(temp.path(), role_name).join("session.md");
        fs::write(&session_path, "cached session contents").expect("seed state content");

        create_role_state(temp.path(), role_name).expect("repeat scaffold should succeed");

        let contents = fs::read_to_string(session_path).expect("session.md should remain readable");
        assert_eq!(contents, "cached session contents");
    }

    #[test]
    fn role_state_exists_only_when_directory_is_present() {
        let temp = TestDir::new("exists");
        let role_name = "operations";

        assert!(!role_state_exists(temp.path(), role_name));

        let state_root = temp.path().join(JULIET_STATE_DIR);
        fs::create_dir_all(&state_root).expect("state root should exist");
        fs::write(state_root.join(role_name), "not a directory").expect("write placeholder file");

        assert!(!role_state_exists(temp.path(), role_name));
    }

    #[test]
    fn write_runtime_prompt_requires_role_state_and_overwrites_contents() {
        let temp = TestDir::new("runtime-prompt");
        let role_name = "engineering";

        let missing_role_error =
            write_runtime_prompt(temp.path(), role_name, "# prompt").expect_err("must fail");
        assert_eq!(missing_role_error.kind(), io::ErrorKind::NotFound);

        create_role_state(temp.path(), role_name).expect("state should be created");
        write_runtime_prompt(temp.path(), role_name, "# prompt one").expect("write prompt one");
        write_runtime_prompt(temp.path(), role_name, "# prompt two").expect("overwrite prompt");

        let contents = fs::read_to_string(runtime_prompt_path(temp.path(), role_name))
            .expect("runtime prompt should be readable");
        assert_eq!(contents, "# prompt two");
    }

    #[test]
    fn discover_configured_roles_returns_empty_when_state_directory_is_missing() {
        let temp = TestDir::new("discover-empty");
        let roles = discover_configured_roles(temp.path()).expect("discovery should succeed");

        assert!(roles.is_empty());
    }

    #[test]
    fn discover_configured_roles_filters_entries_and_maps_prompt_paths() {
        let temp = TestDir::new("discover-filtered");
        let state_root = temp.path().join(JULIET_STATE_DIR);
        fs::create_dir_all(&state_root).expect("state root should be created");

        fs::create_dir_all(state_root.join("director-of-engineering"))
            .expect("engineering role should be created");
        fs::create_dir_all(state_root.join("director-of-marketing"))
            .expect("marketing role should be created");
        fs::create_dir_all(state_root.join(ARTIFACTS_DIR))
            .expect("artifacts directory should be created");
        fs::create_dir_all(state_root.join(LEGACY_PROMPT_ROLE_NAME))
            .expect("legacy juliet directory should be created");
        fs::write(state_root.join("README.md"), "not a role")
            .expect("non-directory entry should be created");

        let prompts_root = temp.path().join(PROMPTS_DIR);
        fs::create_dir_all(&prompts_root).expect("prompts root should be created");
        fs::write(prompts_root.join("juliet.md"), "legacy default prompt")
            .expect("legacy prompt should exist");
        fs::write(prompts_root.join("analyst.md"), "prompt with no role state")
            .expect("orphan prompt should exist");

        let roles = discover_configured_roles(temp.path()).expect("discovery should succeed");
        assert_eq!(
            roles,
            vec![
                ConfiguredRole {
                    name: "director-of-engineering".to_string(),
                    prompt_path: role_prompt_path(temp.path(), "director-of-engineering"),
                },
                ConfiguredRole {
                    name: "director-of-marketing".to_string(),
                    prompt_path: role_prompt_path(temp.path(), "director-of-marketing"),
                },
            ]
        );
    }
}
