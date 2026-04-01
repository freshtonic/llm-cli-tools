//! Interactive setup wizard for generating ~/.config/llm-cli/config.toml.
//!
//! Discovers installed llm-cli-* binaries and prompts the user for the
//! configuration fields each tool requires. Provides instructions for
//! generating API keys where applicable.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

/// Return the config file path, respecting $XDG_CONFIG_HOME.
fn config_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("llm-cli").join("config.toml")
}

/// Prompt the user for a line of input. Returns the trimmed response.
/// If `default` is non-empty, it is shown in brackets and used when
/// the user presses Enter without typing anything.
fn prompt(message: &str, default: &str) -> Result<String, String> {
    let stdin = io::stdin();
    let mut stdout = io::stderr();

    if default.is_empty() {
        write!(stdout, "{message}: ").map_err(|e| e.to_string())?;
    } else {
        write!(stdout, "{message} [{default}]: ").map_err(|e| e.to_string())?;
    }
    stdout.flush().map_err(|e| e.to_string())?;

    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed)
    }
}

/// Ask a yes/no question. Returns true for yes.
fn confirm(message: &str, default_yes: bool) -> Result<bool, String> {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    let input = prompt(&format!("{message} {suffix}"), "")?;
    if input.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(input.to_lowercase().as_str(), "y" | "yes"))
}

/// Print a message to stderr (where all interactive output goes).
fn info(msg: &str) {
    eprintln!("{msg}");
}

/// Collected configuration for a Linear tool.
struct LinearConfig {
    op_item_id: String,
}

/// Collected configuration for a Discourse tool.
struct DiscourseConfig {
    instance_name: String,
    base_url: String,
    op_item_id: String,
    api_username: String,
}

/// Collected configuration for a Slack tool.
struct SlackConfig {
    op_item_id: String,
}

fn setup_linear() -> Result<Option<LinearConfig>, String> {
    info("");
    info("=== Linear ===");
    info("");
    info("To create a Linear API key:");
    info("  1. Go to https://linear.app/settings/account/api");
    info("  2. Under \"Personal API keys\", enter a label and click \"Create key\"");
    info("  3. Copy the key and save it in 1Password");
    info("     - Create a new item (e.g. type \"API Credential\" or \"Password\")");
    info("     - Paste the API key into the \"credential\" field");
    info("     - Note the item's ID from the 1Password URL or `op item list`");
    info("");

    if !confirm("Configure Linear?", true)? {
        return Ok(None);
    }

    let op_item_id = prompt("  1Password item ID for your Linear API key", "")?;
    if op_item_id.is_empty() {
        info("  Skipping Linear (no item ID provided).");
        return Ok(None);
    }

    Ok(Some(LinearConfig { op_item_id }))
}

fn setup_discourse() -> Result<Option<DiscourseConfig>, String> {
    info("");
    info("=== Discourse ===");
    info("");
    info("To create a Discourse API key:");
    info("  1. Go to your Discourse instance's admin panel:");
    info("       https://<your-forum>/admin/api/keys");
    info("  2. Click \"New API Key\"");
    info("  3. Set description, choose \"Single User\" scope for your user");
    info("  4. For \"Scope\", use \"Global\" unless you want to restrict access");
    info("  5. Click \"Save\" and copy the key");
    info("  6. Save it in 1Password");
    info("     - Create a new item and paste the API key into the \"credential\" field");
    info("     - Note the item's ID from the 1Password URL or `op item list`");
    info("");

    if !confirm("Configure Discourse?", true)? {
        return Ok(None);
    }

    let instance_name = prompt("  Instance name (used in config as [discourse.<name>])", "my-forum")?;
    let base_url = prompt("  Base URL (e.g. https://forum.example.com)", "")?;
    if base_url.is_empty() {
        info("  Skipping Discourse (no base URL provided).");
        return Ok(None);
    }
    let api_username = prompt("  API username", "")?;
    if api_username.is_empty() {
        info("  Skipping Discourse (no username provided).");
        return Ok(None);
    }
    let op_item_id = prompt("  1Password item ID for your Discourse API key", "")?;
    if op_item_id.is_empty() {
        info("  Skipping Discourse (no item ID provided).");
        return Ok(None);
    }

    Ok(Some(DiscourseConfig {
        instance_name,
        base_url,
        op_item_id,
        api_username,
    }))
}

fn setup_slack() -> Result<Option<SlackConfig>, String> {
    info("");
    info("=== Slack ===");
    info("");

    if !confirm("Configure Slack?", true)? {
        return Ok(None);
    }

    let op_item_id = prompt("  1Password item ID for your Slack bot token", "")?;
    if op_item_id.is_empty() {
        info("  Skipping Slack (no item ID provided).");
        return Ok(None);
    }

    Ok(Some(SlackConfig { op_item_id }))
}

/// Build the TOML config string from collected configurations.
fn build_toml(
    linear: Option<&LinearConfig>,
    discourse: Option<&DiscourseConfig>,
    slack: Option<&SlackConfig>,
) -> String {
    let mut out = String::new();

    if let Some(cfg) = linear {
        out.push_str("[linear]\n");
        out.push_str(&format!("op_item_id = \"{}\"\n", cfg.op_item_id));
    }

    if let Some(cfg) = discourse {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("[discourse.{}]\n", cfg.instance_name));
        out.push_str(&format!("base_url = \"{}\"\n", cfg.base_url));
        out.push_str(&format!("op_item_id = \"{}\"\n", cfg.op_item_id));
        out.push_str(&format!("api_username = \"{}\"\n", cfg.api_username));
    }

    if let Some(cfg) = slack {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[slack]\n");
        out.push_str(&format!("op_item_id = \"{}\"\n", cfg.op_item_id));
    }

    out
}

/// Run the interactive init wizard.
pub fn run(subcommands: &[String]) -> Result<(), String> {
    let path = config_path();

    info("llm-cli init");
    info("============");
    info("");
    info(&format!("Config file: {}", path.display()));

    if path.exists() {
        info("");
        info("WARNING: Config file already exists.");
        if !confirm("Overwrite?", false)? {
            info("Aborted.");
            return Ok(());
        }
    }

    let has_linear = subcommands.iter().any(|s| s == "linear");
    let has_discourse = subcommands.iter().any(|s| s == "discourse");
    let has_slack = subcommands.iter().any(|s| s == "slack");

    if !has_linear && !has_discourse && !has_slack {
        info("");
        info("No llm-cli-* tool binaries found on PATH.");
        info("Install tools first, then re-run `llm-cli init`.");
        return Ok(());
    }

    info("");
    info(&format!(
        "Detected tools: {}",
        subcommands.join(", ")
    ));

    let linear_cfg = if has_linear { setup_linear()? } else { None };
    let discourse_cfg = if has_discourse {
        setup_discourse()?
    } else {
        None
    };
    let slack_cfg = if has_slack { setup_slack()? } else { None };

    if linear_cfg.is_none() && discourse_cfg.is_none() && slack_cfg.is_none() {
        info("");
        info("No tools configured. Config file not written.");
        return Ok(());
    }

    let toml = build_toml(
        linear_cfg.as_ref(),
        discourse_cfg.as_ref(),
        slack_cfg.as_ref(),
    );

    info("");
    info("Generated config:");
    info("---");
    // Print the config preview to stderr so the user can see it.
    eprint!("{toml}");
    info("---");
    info("");

    if !confirm("Write this config?", true)? {
        info("Aborted.");
        return Ok(());
    }

    // Create parent directories if needed.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
    }

    std::fs::write(&path, &toml)
        .map_err(|e| format!("Failed to write config to {}: {e}", path.display()))?;

    info("");
    info(&format!("Config written to {}", path.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_toml_linear_only() {
        let linear = LinearConfig {
            op_item_id: "abc-123".to_string(),
        };
        let toml = build_toml(Some(&linear), None, None);
        assert_eq!(
            toml,
            "[linear]\nop_item_id = \"abc-123\"\n"
        );
    }

    #[test]
    fn build_toml_discourse_only() {
        let discourse = DiscourseConfig {
            instance_name: "my-forum".to_string(),
            base_url: "https://forum.example.com".to_string(),
            op_item_id: "def-456".to_string(),
            api_username: "james".to_string(),
        };
        let toml = build_toml(None, Some(&discourse), None);
        assert!(toml.contains("[discourse.my-forum]"));
        assert!(toml.contains("base_url = \"https://forum.example.com\""));
        assert!(toml.contains("op_item_id = \"def-456\""));
        assert!(toml.contains("api_username = \"james\""));
    }

    #[test]
    fn build_toml_slack_only() {
        let slack = SlackConfig {
            op_item_id: "ghi-789".to_string(),
        };
        let toml = build_toml(None, None, Some(&slack));
        assert_eq!(
            toml,
            "[slack]\nop_item_id = \"ghi-789\"\n"
        );
    }

    #[test]
    fn build_toml_all_tools() {
        let linear = LinearConfig {
            op_item_id: "lin-id".to_string(),
        };
        let discourse = DiscourseConfig {
            instance_name: "forum".to_string(),
            base_url: "https://forum.test".to_string(),
            op_item_id: "disc-id".to_string(),
            api_username: "user".to_string(),
        };
        let slack = SlackConfig {
            op_item_id: "slack-id".to_string(),
        };
        let toml = build_toml(Some(&linear), Some(&discourse), Some(&slack));
        assert!(toml.contains("[linear]"));
        assert!(toml.contains("[discourse.forum]"));
        assert!(toml.contains("[slack]"));
        // Sections are separated by blank lines.
        assert!(toml.contains("\n\n[discourse.forum]"));
        assert!(toml.contains("\n\n[slack]"));
    }

    #[test]
    fn build_toml_empty() {
        let toml = build_toml(None, None, None);
        assert!(toml.is_empty());
    }

    #[test]
    fn config_path_uses_home() {
        // Just verify it returns a path ending in the expected suffix.
        let path = config_path();
        assert!(path.ends_with("llm-cli/config.toml"));
    }
}
