//! Configuration for the Linear CLI tool.
//!
//! Loaded from `$XDG_CONFIG_HOME/llm-cli/config.toml` (defaults to
//! `~/.config/llm-cli/config.toml`). The `[linear]` section is required
//! and must contain `op_item_id`. The `api_url` field is optional and
//! defaults to `https://api.linear.app`.

use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_API_URL: &str = "https://api.linear.app";

#[derive(Debug, Deserialize)]
struct FileConfig {
    linear: Option<LinearSection>,
}

#[derive(Debug, Deserialize)]
struct LinearSection {
    api_url: Option<String>,
    op_item_id: Option<String>,
}

/// Resolved configuration with all defaults applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub api_url: String,
    pub op_item_id: String,
}

/// Errors that can occur when loading configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// Config file not found at the expected path.
    NotFound(PathBuf),
    /// Config file exists but cannot be parsed.
    ParseError(String),
    /// The `[linear]` section is missing from the config file.
    MissingSection,
    /// The `op_item_id` field is missing from the `[linear]` section.
    MissingOpItemId,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => {
                write!(f, "Config file not found at {}", path.display())
            }
            ConfigError::ParseError(msg) => write!(f, "Failed to parse config: {msg}"),
            ConfigError::MissingSection => write!(f, "Missing [linear] section in config"),
            ConfigError::MissingOpItemId => {
                write!(f, "Missing op_item_id in [linear] config section")
            }
        }
    }
}

/// Parse a TOML string into a resolved `Config`.
pub fn parse(toml_str: &str) -> Result<Config, ConfigError> {
    let file_config: FileConfig =
        toml::from_str(toml_str).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    let section = file_config.linear.ok_or(ConfigError::MissingSection)?;
    let op_item_id = section.op_item_id.ok_or(ConfigError::MissingOpItemId)?;
    let api_url = section
        .api_url
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());

    Ok(Config {
        api_url,
        op_item_id,
    })
}

/// Return the expected config file path.
pub fn config_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("llm-cli").join("config.toml")
    } else {
        // Fallback if XDG_CONFIG_HOME and HOME are both unset.
        PathBuf::from(".config/llm-cli/config.toml")
    }
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
            [linear]
            api_url = \"https://custom.linear.app\"
            op_item_id = \"abc-123\"
        "})
        .unwrap();
        assert_eq!(config.api_url, "https://custom.linear.app");
        assert_eq!(config.op_item_id, "abc-123");
    }

    #[test]
    fn parse_config_defaults_api_url() {
        let config = parse(indoc! {"
            [linear]
            op_item_id = \"abc-123\"
        "})
        .unwrap();
        assert_eq!(config.api_url, "https://api.linear.app");
        assert_eq!(config.op_item_id, "abc-123");
    }

    #[test]
    fn parse_config_missing_linear_section() {
        let err = parse(indoc! {"
            [other]
            foo = \"bar\"
        "})
        .unwrap_err();
        assert!(matches!(err, ConfigError::MissingSection));
    }

    #[test]
    fn parse_config_missing_op_item_id() {
        let err = parse(indoc! {"
            [linear]
            api_url = \"https://api.linear.app\"
        "})
        .unwrap_err();
        assert!(matches!(err, ConfigError::MissingOpItemId));
    }

    #[test]
    fn parse_config_invalid_toml() {
        let err = parse("this is not valid toml {{{").unwrap_err();
        assert!(matches!(err, ConfigError::ParseError(_)));
    }

    #[test]
    fn parse_config_ignores_extra_sections() {
        let config = parse(indoc! {"
            [linear]
            op_item_id = \"abc-123\"

            [discourse.my-forum]
            base_url = \"https://forum.example.com\"
        "})
        .unwrap();
        assert_eq!(config.op_item_id, "abc-123");
    }
}
