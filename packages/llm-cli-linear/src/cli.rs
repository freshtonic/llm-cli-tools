//! CLI argument parsing using clap derive.
//!
//! Subcommands: `issues list`, `issues get`, `issues create`, `issues close`.
//! Global flag: `--human` for human-readable output instead of JSON.

use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// Parsed debug configuration from comma-separated flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugConfig {
    pub pretty: bool,
    pub curl: bool,
    pub dangerous_no_redact: bool,
}

impl DebugConfig {
    /// Parse a comma-separated debug mode string like "pretty,curl".
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut config = DebugConfig {
            pretty: false,
            curl: false,
            dangerous_no_redact: false,
        };
        for flag in s.split(',') {
            match flag.trim() {
                "compact" => {}
                "pretty" => config.pretty = true,
                "curl" => config.curl = true,
                "dangerous_no_redact" => config.dangerous_no_redact = true,
                other => {
                    return Err(format!(
                        "unknown debug mode: '{other}'. Valid modes: compact, pretty, curl, dangerous_no_redact"
                    ));
                }
            }
        }
        Ok(config)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "llm-cli-linear",
    version,
    about = "CLI tool for interacting with the Linear API. Returns JSON by default. \
             Use --human for human-readable output. Retrieves API credentials from \
             1Password at call time."
)]
pub struct Cli {
    /// Output human-readable text instead of JSON.
    #[arg(long, global = true)]
    pub human: bool,

    /// Print raw HTTP requests and responses to stderr.
    /// Comma-separated modes: compact (default), pretty, curl, dangerous_no_redact.
    /// Examples: --debug, --debug=pretty, --debug=curl,dangerous_no_redact
    #[arg(long, global = true, default_missing_value = "compact", num_args = 0..=1, require_equals = true)]
    pub debug: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage Linear issues.
    Issues {
        #[command(subcommand)]
        action: IssuesAction,
    },
    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        #[arg(long)]
        shell: Shell,
    },
    /// Output a JSON description of this tool's commands and arguments for automated discovery.
    Schema,
}

#[derive(Debug, Subcommand)]
pub enum IssuesAction {
    /// List issues. Returns all visible issues by default. Use filters to narrow results.
    List {
        /// Maximum number of issues to return (default: 25).
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Only show issues assigned to the authenticated user.
        #[arg(long)]
        mine: bool,
        /// Filter by team key (e.g., "ENG").
        #[arg(long)]
        team: Option<String>,
        /// Filter by workflow state name (e.g., "In Progress", "Todo").
        #[arg(long)]
        state: Option<String>,
        /// Filter by priority (1=urgent, 2=high, 3=medium, 4=low).
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,
        /// Filter by label name.
        #[arg(long)]
        label: Option<String>,
        /// Pagination cursor from a previous response. Pass the `next_cursor` value to fetch the next page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Fetch a single issue by identifier.
    Get {
        /// The issue identifier (e.g., "PROJ-123").
        #[arg(long)]
        id: String,
    },
    /// Create a new issue.
    Create {
        /// Issue title.
        #[arg(long, required_unless_present = "input")]
        title: Option<String>,
        /// Team key or identifier.
        #[arg(long, required_unless_present = "input")]
        team: Option<String>,
        /// Issue description (markdown).
        #[arg(long)]
        description: Option<String>,
        /// Priority (1 = urgent, 2 = high, 3 = medium, 4 = low).
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,
        /// JSON input from file or stdin. Use "-" for stdin. Overrides individual flags.
        /// Expected format: {"title": "...", "team": "...", "description": "...", "priority": 1}
        #[arg(long, conflicts_with_all = ["title", "team", "description", "priority"])]
        input: Option<String>,
    },
    /// Close an issue by setting its state to "Done".
    Close {
        /// The issue identifier (e.g., "PROJ-123").
        #[arg(long)]
        id: String,
    },
}

/// Parse CLI arguments from the process args.
pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse_args(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("llm-cli-linear").chain(args.iter().copied()))
    }

    #[test]
    fn issues_list_default_limit() {
        let cli = parse_args(&["issues", "list"]).unwrap();
        assert!(!cli.human);
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { limit, .. },
            } => {
                assert_eq!(limit, 25);
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_custom_limit() {
        let cli = parse_args(&["issues", "list", "--limit", "10"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { limit, .. },
            } => {
                assert_eq!(limit, 10);
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_with_filters() {
        let cli = parse_args(&[
            "issues",
            "list",
            "--mine",
            "--team",
            "ENG",
            "--state",
            "In Progress",
        ])
        .unwrap();
        match cli.command {
            Command::Issues {
                action:
                    IssuesAction::List {
                        mine, team, state, ..
                    },
            } => {
                assert!(mine);
                assert_eq!(team.as_deref(), Some("ENG"));
                assert_eq!(state.as_deref(), Some("In Progress"));
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_with_cursor() {
        let cli = parse_args(&["issues", "list", "--cursor", "abc123"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { cursor, .. },
            } => {
                assert_eq!(cursor.as_deref(), Some("abc123"));
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_without_cursor() {
        let cli = parse_args(&["issues", "list"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { cursor, .. },
            } => {
                assert!(cursor.is_none());
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_get_requires_id() {
        let result = parse_args(&["issues", "get"]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_get_with_id() {
        let cli = parse_args(&["issues", "get", "--id", "PROJ-123"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::Get { id },
            } => {
                assert_eq!(id, "PROJ-123");
            }
            _ => panic!("Expected issues get"),
        }
    }

    #[test]
    fn issues_create_required_fields() {
        let cli =
            parse_args(&["issues", "create", "--title", "My Issue", "--team", "ENG"]).unwrap();
        match cli.command {
            Command::Issues {
                action:
                    IssuesAction::Create {
                        title,
                        team,
                        description,
                        priority,
                        input,
                    },
            } => {
                assert_eq!(title.as_deref(), Some("My Issue"));
                assert_eq!(team.as_deref(), Some("ENG"));
                assert!(description.is_none());
                assert!(priority.is_none());
                assert!(input.is_none());
            }
            _ => panic!("Expected issues create"),
        }
    }

    #[test]
    fn issues_create_all_fields() {
        let cli = parse_args(&[
            "issues",
            "create",
            "--title",
            "Bug fix",
            "--team",
            "ENG",
            "--description",
            "Fix the thing",
            "--priority",
            "2",
        ])
        .unwrap();
        match cli.command {
            Command::Issues {
                action:
                    IssuesAction::Create {
                        title,
                        team,
                        description,
                        priority,
                        ..
                    },
            } => {
                assert_eq!(title.as_deref(), Some("Bug fix"));
                assert_eq!(team.as_deref(), Some("ENG"));
                assert_eq!(description.as_deref(), Some("Fix the thing"));
                assert_eq!(priority, Some(2));
            }
            _ => panic!("Expected issues create"),
        }
    }

    #[test]
    fn issues_create_rejects_invalid_priority() {
        let result = parse_args(&[
            "issues",
            "create",
            "--title",
            "T",
            "--team",
            "E",
            "--priority",
            "5",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_create_rejects_zero_priority() {
        let result = parse_args(&[
            "issues",
            "create",
            "--title",
            "T",
            "--team",
            "E",
            "--priority",
            "0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_close_with_id() {
        let cli = parse_args(&["issues", "close", "--id", "PROJ-456"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::Close { id },
            } => {
                assert_eq!(id, "PROJ-456");
            }
            _ => panic!("Expected issues close"),
        }
    }

    #[test]
    fn human_flag_before_subcommand() {
        let cli = parse_args(&["--human", "issues", "list"]).unwrap();
        assert!(cli.human);
    }

    #[test]
    fn human_flag_after_subcommand() {
        let cli = parse_args(&["issues", "--human", "list"]).unwrap();
        assert!(cli.human);
    }

    #[test]
    fn issues_list_with_priority_filter() {
        let cli = parse_args(&["issues", "list", "--priority", "2"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { priority, .. },
            } => {
                assert_eq!(priority, Some(2));
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_with_label_filter() {
        let cli = parse_args(&["issues", "list", "--label", "bug"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::List { label, .. },
            } => {
                assert_eq!(label.as_deref(), Some("bug"));
            }
            _ => panic!("Expected issues list"),
        }
    }

    #[test]
    fn issues_list_rejects_invalid_priority_filter() {
        let result = parse_args(&["issues", "list", "--priority", "5"]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_list_rejects_zero_priority_filter() {
        let result = parse_args(&["issues", "list", "--priority", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_create_with_input_flag() {
        let cli = parse_args(&["issues", "create", "--input", "data.json"]).unwrap();
        match cli.command {
            Command::Issues {
                action:
                    IssuesAction::Create {
                        input, title, team, ..
                    },
            } => {
                assert_eq!(input.as_deref(), Some("data.json"));
                assert!(title.is_none());
                assert!(team.is_none());
            }
            _ => panic!("Expected issues create"),
        }
    }

    #[test]
    fn issues_create_with_input_stdin() {
        let cli = parse_args(&["issues", "create", "--input", "-"]).unwrap();
        match cli.command {
            Command::Issues {
                action: IssuesAction::Create { input, .. },
            } => {
                assert_eq!(input.as_deref(), Some("-"));
            }
            _ => panic!("Expected issues create"),
        }
    }

    #[test]
    fn issues_create_input_conflicts_with_title() {
        let result = parse_args(&["issues", "create", "--input", "data.json", "--title", "T"]);
        assert!(result.is_err());
    }

    #[test]
    fn issues_create_input_conflicts_with_team() {
        let result = parse_args(&["issues", "create", "--input", "data.json", "--team", "ENG"]);
        assert!(result.is_err());
    }

    #[test]
    fn schema_subcommand_parses() {
        let cli = parse_args(&["schema"]).unwrap();
        assert!(matches!(cli.command, Command::Schema));
    }

    #[test]
    fn no_subcommand_shows_error() {
        let result = parse_args(&[]);
        assert!(result.is_err());
    }

    // ---- DebugConfig parsing tests ----

    #[test]
    fn debug_config_compact() {
        let config = DebugConfig::parse("compact").unwrap();
        assert!(!config.pretty);
        assert!(!config.curl);
        assert!(!config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_pretty() {
        let config = DebugConfig::parse("pretty").unwrap();
        assert!(config.pretty);
        assert!(!config.curl);
        assert!(!config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_curl() {
        let config = DebugConfig::parse("curl").unwrap();
        assert!(!config.pretty);
        assert!(config.curl);
        assert!(!config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_dangerous_no_redact() {
        let config = DebugConfig::parse("dangerous_no_redact").unwrap();
        assert!(!config.pretty);
        assert!(!config.curl);
        assert!(config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_curl_and_dangerous_no_redact() {
        let config = DebugConfig::parse("curl,dangerous_no_redact").unwrap();
        assert!(!config.pretty);
        assert!(config.curl);
        assert!(config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_all_modes() {
        let config = DebugConfig::parse("pretty,curl,dangerous_no_redact").unwrap();
        assert!(config.pretty);
        assert!(config.curl);
        assert!(config.dangerous_no_redact);
    }

    #[test]
    fn debug_config_unknown_mode() {
        let err = DebugConfig::parse("invalid").unwrap_err();
        assert!(err.contains("unknown debug mode"));
        assert!(err.contains("invalid"));
    }

    #[test]
    fn debug_config_rejects_old_curl_cmd() {
        let err = DebugConfig::parse("curl_cmd").unwrap_err();
        assert!(err.contains("unknown debug mode"));
    }
}
