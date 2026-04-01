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
    pub curl_cmd: bool,
}

impl DebugConfig {
    /// Parse a comma-separated debug mode string like "pretty,curl_cmd".
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut config = DebugConfig {
            pretty: false,
            curl_cmd: false,
        };
        for flag in s.split(',') {
            match flag.trim() {
                "compact" => {}
                "pretty" => config.pretty = true,
                "curl_cmd" => config.curl_cmd = true,
                other => {
                    return Err(format!(
                        "unknown debug mode: '{other}'. Valid modes: compact, pretty, curl_cmd"
                    ));
                }
            }
        }
        config.confirm_curl_cmd()?;
        Ok(config)
    }

    fn confirm_curl_cmd(&self) -> Result<(), String> {
        if !self.curl_cmd {
            return Ok(());
        }
        eprint!("WARNING: curl_cmd mode will print secrets (API keys) to stderr. Continue? [y/N] ");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {e}"))?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            return Err("Aborted.".to_string());
        }
        Ok(())
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
    /// Comma-separated modes: compact (default), pretty, curl_cmd.
    /// Examples: --debug, --debug=pretty, --debug=pretty,curl_cmd
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
        #[arg(long)]
        title: String,
        /// Team key or identifier.
        #[arg(long)]
        team: String,
        /// Issue description (markdown).
        #[arg(long)]
        description: Option<String>,
        /// Priority (1 = urgent, 2 = high, 3 = medium, 4 = low).
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,
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
                    },
            } => {
                assert_eq!(title, "My Issue");
                assert_eq!(team, "ENG");
                assert!(description.is_none());
                assert!(priority.is_none());
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
                    },
            } => {
                assert_eq!(title, "Bug fix");
                assert_eq!(team, "ENG");
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
    fn no_subcommand_shows_error() {
        let result = parse_args(&[]);
        assert!(result.is_err());
    }
}
