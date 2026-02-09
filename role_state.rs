#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const JULIET_STATE_DIR: &str = ".juliet";
const ARTIFACTS_DIR: &str = "artifacts";
const STATE_GITIGNORE_FILE: &str = ".gitignore";
const STATE_GITIGNORE_CONTENTS: &str = "# Managed by juliet: keep role prompt customizations, ignore runtime state.\n*\n!.gitignore\n!*/\n!*/prompt.md\n";
const PROMPT_FILE: &str = "prompt.md";
const RUNTIME_PROMPT_FILE: &str = "juliet-prompt.md";
const STATE_FILES: [&str; 5] = [
    "session.md",
    "needs-from-operator.md",
    "projects.md",
    "processes.md",
    "learnings.md",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredRole {
    pub name: String,
    pub prompt_path: PathBuf,
}

pub fn role_state_dir(project_root: &Path, role_name: &str) -> PathBuf {
    project_root.join(JULIET_STATE_DIR).join(role_name)
}

pub fn state_gitignore_path(project_root: &Path) -> PathBuf {
    project_root.join(JULIET_STATE_DIR).join(STATE_GITIGNORE_FILE)
}

pub fn role_prompt_path(project_root: &Path, role_name: &str) -> PathBuf {
    role_state_dir(project_root, role_name).join(PROMPT_FILE)
}

pub fn runtime_prompt_path(project_root: &Path, role_name: &str) -> PathBuf {
    role_state_dir(project_root, role_name).join(RUNTIME_PROMPT_FILE)
}

pub fn role_state_exists(project_root: &Path, role_name: &str) -> bool {
    role_state_dir(project_root, role_name).is_dir()
}

pub fn role_state_is_scaffolded(project_root: &Path, role_name: &str) -> bool {
    has_role_state_layout(&role_state_dir(project_root, role_name))
}

pub fn create_role_state(project_root: &Path, role_name: &str) -> io::Result<()> {
    ensure_state_gitignore(project_root)?;

    let role_dir = role_state_dir(project_root, role_name);
    fs::create_dir_all(role_dir.join(ARTIFACTS_DIR))?;

    for file in STATE_FILES {
        ensure_file(&role_dir.join(file))?;
    }

    Ok(())
}

pub fn ensure_state_gitignore(project_root: &Path) -> io::Result<()> {
    let state_root = project_root.join(JULIET_STATE_DIR);
    fs::create_dir_all(&state_root)?;
    fs::write(state_gitignore_path(project_root), STATE_GITIGNORE_CONTENTS)
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

        let role_dir = entry.path();
        if role_name == ARTIFACTS_DIR && !has_role_state_layout(&role_dir) {
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

fn has_role_state_layout(role_dir: &Path) -> bool {
    STATE_FILES.iter().all(|file| role_dir.join(file).is_file())
        && role_dir.join(ARTIFACTS_DIR).is_dir()
}

pub fn clear_role_history(project_root: &Path, role_name: &str) -> io::Result<()> {
    let role_dir = role_state_dir(project_root, role_name);

    // Empty state files
    for file in STATE_FILES {
        fs::write(role_dir.join(file), "")?;
    }

    // Delete runtime prompt if present
    let runtime_path = runtime_prompt_path(project_root, role_name);
    if runtime_path.exists() {
        fs::remove_file(&runtime_path)?;
    }

    // Clear artifacts directory contents while preserving the directory
    let artifacts_dir = role_dir.join(ARTIFACTS_DIR);
    if artifacts_dir.is_dir() {
        for entry in fs::read_dir(&artifacts_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
    }

    Ok(())
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
    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
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
            fs::read_to_string(state_gitignore_path(temp.path()))
                .expect("state gitignore should be readable"),
            STATE_GITIGNORE_CONTENTS
        );

        assert_eq!(
            runtime_prompt_path(temp.path(), role_name),
            role_dir.join(RUNTIME_PROMPT_FILE)
        );
        assert!(!runtime_prompt_path(temp.path(), role_name).exists());
    }

    #[test]
    fn state_path_helpers_are_scoped_under_project_root() {
        let temp = TestDir::new("path-helpers");
        let role_name = "operations";

        assert_eq!(
            role_state_dir(temp.path(), role_name),
            temp.path().join(JULIET_STATE_DIR).join(role_name)
        );
        assert_eq!(
            role_prompt_path(temp.path(), role_name),
            temp.path()
                .join(JULIET_STATE_DIR)
                .join(role_name)
                .join(PROMPT_FILE)
        );
        assert_eq!(
            runtime_prompt_path(temp.path(), role_name),
            temp.path()
                .join(JULIET_STATE_DIR)
                .join(role_name)
                .join(RUNTIME_PROMPT_FILE)
        );
        assert_eq!(
            state_gitignore_path(temp.path()),
            temp.path().join(JULIET_STATE_DIR).join(STATE_GITIGNORE_FILE)
        );
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
    fn ensure_state_gitignore_rewrites_customized_contents_to_expected_template() {
        let temp = TestDir::new("state-gitignore");
        let role_name = "director-of-product";
        create_role_state(temp.path(), role_name).expect("initial scaffold should succeed");

        let gitignore_path = state_gitignore_path(temp.path());
        fs::write(&gitignore_path, "custom\n").expect("customized gitignore should be writable");

        ensure_state_gitignore(temp.path()).expect("gitignore should be restored");
        assert_eq!(
            fs::read_to_string(gitignore_path).expect("state gitignore should be readable"),
            STATE_GITIGNORE_CONTENTS
        );
    }

    #[test]
    fn create_role_state_errors_when_expected_state_file_path_is_a_directory() {
        let temp = TestDir::new("state-file-collision");
        let role_name = "engineering";
        let role_dir = role_state_dir(temp.path(), role_name);
        fs::create_dir_all(role_dir.join("session.md"))
            .expect("directory should occupy an expected state file path");

        let err = create_role_state(temp.path(), role_name)
            .expect_err("scaffold should fail when file path is not a file");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            err.to_string()
                .contains("expected file path, found non-file:"),
            "unexpected error message: {}",
            err
        );
    }

    #[test]
    fn role_state_exists_only_when_directory_is_present() {
        let temp = TestDir::new("exists");
        let role_name = "operations";

        assert!(!role_state_exists(temp.path(), role_name));
        assert!(!role_state_is_scaffolded(temp.path(), role_name));

        let state_root = temp.path().join(JULIET_STATE_DIR);
        fs::create_dir_all(&state_root).expect("state root should exist");
        fs::write(state_root.join(role_name), "not a directory").expect("write placeholder file");

        assert!(!role_state_exists(temp.path(), role_name));
        assert!(!role_state_is_scaffolded(temp.path(), role_name));
    }

    #[test]
    fn role_state_is_scaffolded_requires_full_layout() {
        let temp = TestDir::new("scaffolded-layout");
        let role_name = "artifacts";
        let role_dir = role_state_dir(temp.path(), role_name);

        fs::create_dir_all(&role_dir).expect("legacy role directory should be created");
        assert!(role_state_exists(temp.path(), role_name));
        assert!(
            !role_state_is_scaffolded(temp.path(), role_name),
            "role directory without state files should not count as scaffolded"
        );

        create_role_state(temp.path(), role_name).expect("missing role state should be scaffolded");
        assert!(role_state_exists(temp.path(), role_name));
        assert!(
            role_state_is_scaffolded(temp.path(), role_name),
            "role state should be scaffolded once required files and artifacts directory exist"
        );
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
        fs::create_dir_all(state_root.join("juliet")).expect("juliet role should be created");
        fs::write(state_root.join("README.md"), "not a role")
            .expect("non-directory entry should be created");

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
                ConfiguredRole {
                    name: "juliet".to_string(),
                    prompt_path: role_prompt_path(temp.path(), "juliet"),
                },
            ]
        );
    }

    #[test]
    fn discover_configured_roles_includes_artifacts_when_it_has_role_state_layout() {
        let temp = TestDir::new("discover-artifacts-role");
        create_role_state(temp.path(), ARTIFACTS_DIR)
            .expect("artifacts role state should be created");

        let roles = discover_configured_roles(temp.path()).expect("discovery should succeed");
        assert_eq!(
            roles,
            vec![ConfiguredRole {
                name: ARTIFACTS_DIR.to_string(),
                prompt_path: role_prompt_path(temp.path(), ARTIFACTS_DIR),
            }]
        );
    }

    #[test]
    fn discover_configured_roles_returns_names_in_sorted_order() {
        let temp = TestDir::new("discover-sorted");
        let state_root = temp.path().join(JULIET_STATE_DIR);
        fs::create_dir_all(state_root.join("zeta-team")).expect("zeta role should be created");
        fs::create_dir_all(state_root.join("alpha-team")).expect("alpha role should be created");

        let roles = discover_configured_roles(temp.path()).expect("discovery should succeed");
        assert_eq!(
            roles,
            vec![
                ConfiguredRole {
                    name: "alpha-team".to_string(),
                    prompt_path: role_prompt_path(temp.path(), "alpha-team"),
                },
                ConfiguredRole {
                    name: "zeta-team".to_string(),
                    prompt_path: role_prompt_path(temp.path(), "zeta-team"),
                },
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn discover_configured_roles_ignores_non_utf8_directory_names() {
        let temp = TestDir::new("discover-non-utf8");
        let state_root = temp.path().join(JULIET_STATE_DIR);
        fs::create_dir_all(state_root.join("qa")).expect("qa role should be created");

        let invalid_name = OsString::from_vec(vec![b'n', b'a', b'm', b'e', 0xFF]);
        match fs::create_dir_all(state_root.join(PathBuf::from(invalid_name))) {
            Ok(_) => {}
            Err(err) if err.raw_os_error() == Some(92) => {
                // macOS APFS does not support non-UTF-8 filenames (EILSEQ).
                // The behavior under test only matters on filesystems that allow them.
                return;
            }
            Err(err) => panic!("unexpected error creating non-utf8 directory: {err}"),
        }

        let roles = discover_configured_roles(temp.path()).expect("discovery should succeed");
        assert_eq!(
            roles,
            vec![ConfiguredRole {
                name: "qa".to_string(),
                prompt_path: role_prompt_path(temp.path(), "qa"),
            }]
        );
    }
}
