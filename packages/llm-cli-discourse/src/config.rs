//! Configuration for the Discourse CLI tool.
//!
//! Loaded from `$XDG_CONFIG_HOME/llm-cli/config.toml`. Supports multiple
//! Discourse instances under `[discourse.<name>]` sections. Each instance
//! requires `base_url`, `op_item_id`, and `api_username`.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FileConfig {
    discourse: Option<BTreeMap<String, DiscourseSection>>,
}

#[derive(Debug, Deserialize)]
struct DiscourseSection {
    base_url: Option<String>,
    op_item_id: Option<String>,
    op_field: Option<String>,
    api_username: Option<String>,
}

/// Resolved configuration for a single Discourse instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceConfig {
    pub name: String,
    pub base_url: String,
    pub op_item_id: String,
    /// The 1Password field name to read. Defaults to "credential".
    pub op_field: String,
    pub api_username: String,
}

/// Errors that can occur when loading configuration.
#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    ParseError(String),
    NoInstances,
    InstanceNotFound(String),
    AmbiguousInstance(Vec<String>),
    MissingField {
        instance: String,
        field: &'static str,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => {
                write!(f, "Config file not found at {}", path.display())
            }
            ConfigError::ParseError(msg) => write!(f, "Failed to parse config: {msg}"),
            ConfigError::NoInstances => write!(f, "No [discourse.*] sections found in config"),
            ConfigError::InstanceNotFound(name) => {
                write!(f, "Discourse instance '{name}' not found in config")
            }
            ConfigError::AmbiguousInstance(names) => {
                write!(
                    f,
                    "Multiple Discourse instances configured ({}). Use --instance to select one.",
                    names.join(", ")
                )
            }
            ConfigError::MissingField { instance, field } => {
                write!(
                    f,
                    "Missing '{field}' in [discourse.{instance}] config section"
                )
            }
        }
    }
}

/// Parse all Discourse instance configs from a TOML string.
fn parse_all(toml_str: &str) -> Result<BTreeMap<String, DiscourseSection>, ConfigError> {
    let file_config: FileConfig =
        toml::from_str(toml_str).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    let instances = file_config.discourse.ok_or(ConfigError::NoInstances)?;
    if instances.is_empty() {
        return Err(ConfigError::NoInstances);
    }
    Ok(instances)
}

/// Resolve a single instance from parsed config, validating required fields.
fn resolve_instance(
    name: &str,
    section: &DiscourseSection,
) -> Result<InstanceConfig, ConfigError> {
    let base_url = section
        .base_url
        .clone()
        .ok_or(ConfigError::MissingField {
            instance: name.to_string(),
            field: "base_url",
        })?;
    let op_item_id = section
        .op_item_id
        .clone()
        .ok_or(ConfigError::MissingField {
            instance: name.to_string(),
            field: "op_item_id",
        })?;
    let op_field = section
        .op_field
        .clone()
        .unwrap_or_else(|| "credential".to_string());
    let api_username = section
        .api_username
        .clone()
        .ok_or(ConfigError::MissingField {
            instance: name.to_string(),
            field: "api_username",
        })?;

    Ok(InstanceConfig {
        name: name.to_string(),
        base_url,
        op_item_id,
        op_field,
        api_username,
    })
}

/// Parse config and select the appropriate instance.
///
/// If `instance_name` is `Some`, use that specific instance.
/// If `None` and only one instance exists, use it automatically.
/// If `None` and multiple exist, return an error listing available instances.
pub fn select_instance(
    toml_str: &str,
    instance_name: Option<&str>,
) -> Result<InstanceConfig, ConfigError> {
    let instances = parse_all(toml_str)?;

    match instance_name {
        Some(name) => {
            let section = instances
                .get(name)
                .ok_or_else(|| ConfigError::InstanceNotFound(name.to_string()))?;
            resolve_instance(name, section)
        }
        None => {
            if instances.len() == 1 {
                let (name, section) = instances.iter().next().unwrap();
                resolve_instance(name, section)
            } else {
                let names: Vec<String> = instances.keys().cloned().collect();
                Err(ConfigError::AmbiguousInstance(names))
            }
        }
    }
}

/// Return the expected config file path.
///
/// Uses `$XDG_CONFIG_HOME/llm-cli/config.toml` if set, otherwise
/// `$HOME/.config/llm-cli/config.toml`.
pub fn config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("llm-cli").join("config.toml")
}

/// Load configuration from the default config file path.
pub fn load(instance_name: Option<&str>) -> Result<InstanceConfig, ConfigError> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).map_err(|_| ConfigError::NotFound(path))?;
    select_instance(&content, instance_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn select_single_instance_auto() {
        let config = select_instance(
            indoc! {"
                [discourse.my-forum]
                base_url = \"https://forum.example.com\"
                op_item_id = \"abc-123\"
                api_username = \"james\"
            "},
            None,
        )
        .unwrap();
        assert_eq!(config.name, "my-forum");
        assert_eq!(config.base_url, "https://forum.example.com");
        assert_eq!(config.op_item_id, "abc-123");
        assert_eq!(config.api_username, "james");
    }

    #[test]
    fn select_named_instance() {
        let config = select_instance(
            indoc! {"
                [discourse.forum-a]
                base_url = \"https://a.example.com\"
                op_item_id = \"id-a\"
                api_username = \"user-a\"

                [discourse.forum-b]
                base_url = \"https://b.example.com\"
                op_item_id = \"id-b\"
                api_username = \"user-b\"
            "},
            Some("forum-b"),
        )
        .unwrap();
        assert_eq!(config.name, "forum-b");
        assert_eq!(config.base_url, "https://b.example.com");
    }

    #[test]
    fn multiple_instances_without_selection_is_ambiguous() {
        let err = select_instance(
            indoc! {"
                [discourse.forum-a]
                base_url = \"https://a.example.com\"
                op_item_id = \"id-a\"
                api_username = \"user-a\"

                [discourse.forum-b]
                base_url = \"https://b.example.com\"
                op_item_id = \"id-b\"
                api_username = \"user-b\"
            "},
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::AmbiguousInstance(_)));
    }

    #[test]
    fn instance_not_found() {
        let err = select_instance(
            indoc! {"
                [discourse.my-forum]
                base_url = \"https://forum.example.com\"
                op_item_id = \"abc-123\"
                api_username = \"james\"
            "},
            Some("other-forum"),
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::InstanceNotFound(_)));
    }

    #[test]
    fn no_discourse_section() {
        let err = select_instance(
            indoc! {"
                [linear]
                op_item_id = \"abc-123\"
            "},
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::NoInstances));
    }

    #[test]
    fn missing_base_url() {
        let err = select_instance(
            indoc! {"
                [discourse.my-forum]
                op_item_id = \"abc-123\"
                api_username = \"james\"
            "},
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField { field: "base_url", .. }
        ));
    }

    #[test]
    fn missing_op_item_id() {
        let err = select_instance(
            indoc! {"
                [discourse.my-forum]
                base_url = \"https://forum.example.com\"
                api_username = \"james\"
            "},
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField {
                field: "op_item_id",
                ..
            }
        ));
    }

    #[test]
    fn missing_api_username() {
        let err = select_instance(
            indoc! {"
                [discourse.my-forum]
                base_url = \"https://forum.example.com\"
                op_item_id = \"abc-123\"
            "},
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField {
                field: "api_username",
                ..
            }
        ));
    }

    #[test]
    fn invalid_toml() {
        let err = select_instance("not valid {{{", None).unwrap_err();
        assert!(matches!(err, ConfigError::ParseError(_)));
    }

    #[test]
    fn ignores_other_sections() {
        let config = select_instance(
            indoc! {"
                [linear]
                op_item_id = \"linear-id\"

                [discourse.my-forum]
                base_url = \"https://forum.example.com\"
                op_item_id = \"abc-123\"
                api_username = \"james\"
            "},
            None,
        )
        .unwrap();
        assert_eq!(config.name, "my-forum");
    }
}
