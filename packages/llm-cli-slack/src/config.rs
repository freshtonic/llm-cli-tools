//! Configuration for the Slack CLI tool.
//!
//! Loaded from `$XDG_CONFIG_HOME/llm-cli/config.toml`. The `[slack]`
//! section is required and must contain `op_item_id`.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FileConfig {
    slack: Option<SlackSection>,
}

#[derive(Debug, Deserialize)]
struct SlackSection {
    op_item_id: Option<String>,
}

/// Resolved configuration with all defaults applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub op_item_id: String,
}

/// Errors that can occur when loading configuration.
#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    ParseError(String),
    MissingSection,
    MissingOpItemId,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => {
                write!(f, "Config file not found at {}", path.display())
            }
            ConfigError::ParseError(msg) => write!(f, "Failed to parse config: {msg}"),
            ConfigError::MissingSection => write!(f, "Missing [slack] section in config"),
            ConfigError::MissingOpItemId => {
                write!(f, "Missing op_item_id in [slack] config section")
            }
        }
    }
}

/// Parse a TOML string into a resolved `Config`.
pub fn parse(toml_str: &str) -> Result<Config, ConfigError> {
    let file_config: FileConfig =
        toml::from_str(toml_str).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    let section = file_config.slack.ok_or(ConfigError::MissingSection)?;
    let op_item_id = section.op_item_id.ok_or(ConfigError::MissingOpItemId)?;

    Ok(Config { op_item_id })
}

/// Return the expected config file path.
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
pub fn load() -> Result<Config, ConfigError> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).map_err(|_| ConfigError::NotFound(path))?;
    parse(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parse_full_config() {
        let config = parse(indoc! {"
            [slack]
            op_item_id = \"xoxb-123\"
        "})
        .unwrap();
        assert_eq!(config.op_item_id, "xoxb-123");
    }

    #[test]
    fn parse_config_missing_slack_section() {
        let err = parse(indoc! {"
            [linear]
            op_item_id = \"abc\"
        "})
        .unwrap_err();
        assert!(matches!(err, ConfigError::MissingSection));
    }

    #[test]
    fn parse_config_missing_op_item_id() {
        let err = parse(indoc! {"
            [slack]
        "})
        .unwrap_err();
        assert!(matches!(err, ConfigError::MissingOpItemId));
    }

    #[test]
    fn parse_config_invalid_toml() {
        let err = parse("not valid {{{").unwrap_err();
        assert!(matches!(err, ConfigError::ParseError(_)));
    }

    #[test]
    fn parse_config_ignores_extra_sections() {
        let config = parse(indoc! {"
            [slack]
            op_item_id = \"xoxb-123\"

            [linear]
            op_item_id = \"abc\"
        "})
        .unwrap();
        assert_eq!(config.op_item_id, "xoxb-123");
    }
}
