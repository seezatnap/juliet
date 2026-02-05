#![allow(dead_code)]

const INVALID_ROLE_NAME_RULES: &str = "Use lowercase letters, numbers, and hyphens.";

fn invalid_role_name_error(name: &str) -> String {
    format!("Invalid role name: {name}. {INVALID_ROLE_NAME_RULES}")
}

pub fn is_valid_role_name(name: &str) -> bool {
    if name.is_empty() || name.starts_with('-') || name.ends_with('-') {
        return false;
    }

    name.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    })
}

pub fn validate_role_name(name: &str) -> Result<(), String> {
    if is_valid_role_name(name) {
        Ok(())
    } else {
        Err(invalid_role_name_error(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_alphanumeric_and_hyphens() {
        for valid_name in ["a", "role-1", "director-of-engineering", "a1-b2-c3"] {
            assert!(is_valid_role_name(valid_name));
            assert_eq!(validate_role_name(valid_name), Ok(()));
        }
    }

    #[test]
    fn rejects_empty_and_hyphen_edges() {
        assert!(!is_valid_role_name(""));
        assert_eq!(
            validate_role_name(""),
            Err("Invalid role name: . Use lowercase letters, numbers, and hyphens.".to_string())
        );

        for invalid_name in ["-role", "role-", "-"] {
            assert!(!is_valid_role_name(invalid_name));
            assert_eq!(
                validate_role_name(invalid_name),
                Err(format!(
                    "Invalid role name: {invalid_name}. Use lowercase letters, numbers, and hyphens."
                ))
            );
        }
    }

    #[test]
    fn rejects_uppercase_and_non_alphanumeric_characters() {
        for invalid_name in ["Role", "my_role", "qa role", "ümlaut"] {
            assert!(!is_valid_role_name(invalid_name));
            assert_eq!(
                validate_role_name(invalid_name),
                Err(format!(
                    "Invalid role name: {invalid_name}. Use lowercase letters, numbers, and hyphens."
                ))
            );
        }
    }

    #[test]
    fn allows_consecutive_hyphens_when_not_on_edges() {
        for valid_name in ["0", "123", "eng--ops", "team-01--alpha"] {
            assert!(is_valid_role_name(valid_name));
            assert_eq!(validate_role_name(valid_name), Ok(()));
        }
    }

    #[test]
    fn rejects_whitespace_and_path_like_names_without_trimming() {
        for invalid_name in [" role", "role ", "role/name", "role.name"] {
            assert!(!is_valid_role_name(invalid_name));
            assert_eq!(
                validate_role_name(invalid_name),
                Err(format!(
                    "Invalid role name: {invalid_name}. Use lowercase letters, numbers, and hyphens."
                ))
            );
        }
    }
}
