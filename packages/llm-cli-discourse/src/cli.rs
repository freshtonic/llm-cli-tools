//! CLI argument parsing using clap derive.
//!
//! Subcommands: `posts get|create|delete`, `comments create|delete`.
//! Global flags: `--human` for human-readable output, `--instance` to
//! select which Discourse instance to use from config.

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
    name = "llm-cli-discourse",
    version,
    about = "CLI tool for interacting with the Discourse API. Returns JSON by default. \
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

    /// Which Discourse instance to use (maps to [discourse.<name>] in config).
    /// Required when multiple instances are configured; auto-selected if only one exists.
    #[arg(long, global = true)]
    pub instance: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage Discourse posts (topics).
    Posts {
        #[command(subcommand)]
        action: PostsAction,
    },
    /// Manage Discourse comments (replies).
    Comments {
        #[command(subcommand)]
        action: CommentsAction,
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
pub enum PostsAction {
    /// List the latest posts across all topics.
    Latest {
        /// Page number for pagination (starts at 0).
        #[arg(long)]
        page: Option<u32>,
    },
    /// Fetch a single post/topic by ID.
    Get {
        /// The topic ID.
        #[arg(long)]
        id: u64,
    },
    /// Create a new topic.
    Create {
        /// Topic title.
        #[arg(long, required_unless_present = "input")]
        title: Option<String>,
        /// Category name or ID.
        #[arg(long, required_unless_present = "input")]
        category: Option<String>,
        /// Post body (raw markdown).
        #[arg(long)]
        raw: Option<String>,
        /// JSON input from file or stdin. Use "-" for stdin. Overrides individual flags.
        /// Expected format: {"title": "...", "category": "...", "raw": "..."}
        #[arg(long, conflicts_with_all = ["title", "category", "raw"])]
        input: Option<String>,
    },
    /// Delete a post/topic by ID.
    Delete {
        /// The topic ID.
        #[arg(long)]
        id: u64,
    },
}

#[derive(Debug, Subcommand)]
pub enum CommentsAction {
    /// Reply to an existing topic.
    Create {
        /// The topic ID to reply to.
        #[arg(long)]
        topic_id: u64,
        /// Reply body (raw markdown).
        #[arg(long)]
        raw: String,
    },
    /// Delete a comment (post) by ID.
    Delete {
        /// The post ID to delete.
        #[arg(long)]
        id: u64,
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
        Cli::try_parse_from(std::iter::once("llm-cli-discourse").chain(args.iter().copied()))
    }

    #[test]
    fn posts_latest() {
        let cli = parse_args(&["posts", "latest"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Posts {
                action: PostsAction::Latest { .. },
            }
        ));
    }

    #[test]
    fn posts_latest_with_page() {
        let cli = parse_args(&["posts", "latest", "--page", "2"]).unwrap();
        match cli.command {
            Command::Posts {
                action: PostsAction::Latest { page },
            } => {
                assert_eq!(page, Some(2));
            }
            _ => panic!("Expected posts latest"),
        }
    }

    #[test]
    fn posts_latest_without_page() {
        let cli = parse_args(&["posts", "latest"]).unwrap();
        match cli.command {
            Command::Posts {
                action: PostsAction::Latest { page },
            } => {
                assert!(page.is_none());
            }
            _ => panic!("Expected posts latest"),
        }
    }

    #[test]
    fn posts_get_with_id() {
        let cli = parse_args(&["posts", "get", "--id", "42"]).unwrap();
        assert!(!cli.human);
        match cli.command {
            Command::Posts {
                action: PostsAction::Get { id },
            } => assert_eq!(id, 42),
            _ => panic!("Expected posts get"),
        }
    }

    #[test]
    fn posts_get_requires_id() {
        assert!(parse_args(&["posts", "get"]).is_err());
    }

    #[test]
    fn posts_create_required_fields() {
        let cli = parse_args(&[
            "posts",
            "create",
            "--title",
            "My Topic",
            "--category",
            "general",
        ])
        .unwrap();
        match cli.command {
            Command::Posts {
                action:
                    PostsAction::Create {
                        title,
                        category,
                        raw,
                        input,
                    },
            } => {
                assert_eq!(title.as_deref(), Some("My Topic"));
                assert_eq!(category.as_deref(), Some("general"));
                assert!(raw.is_none());
                assert!(input.is_none());
            }
            _ => panic!("Expected posts create"),
        }
    }

    #[test]
    fn posts_create_with_raw() {
        let cli = parse_args(&[
            "posts",
            "create",
            "--title",
            "T",
            "--category",
            "C",
            "--raw",
            "Body text",
        ])
        .unwrap();
        match cli.command {
            Command::Posts {
                action: PostsAction::Create { raw, .. },
            } => assert_eq!(raw.as_deref(), Some("Body text")),
            _ => panic!("Expected posts create"),
        }
    }

    #[test]
    fn posts_create_with_input_flag() {
        let cli = parse_args(&["posts", "create", "--input", "topic.json"]).unwrap();
        match cli.command {
            Command::Posts {
                action:
                    PostsAction::Create {
                        input,
                        title,
                        category,
                        ..
                    },
            } => {
                assert_eq!(input.as_deref(), Some("topic.json"));
                assert!(title.is_none());
                assert!(category.is_none());
            }
            _ => panic!("Expected posts create"),
        }
    }

    #[test]
    fn posts_create_input_conflicts_with_title() {
        let result = parse_args(&["posts", "create", "--input", "topic.json", "--title", "T"]);
        assert!(result.is_err());
    }

    #[test]
    fn posts_create_input_conflicts_with_category() {
        let result = parse_args(&[
            "posts",
            "create",
            "--input",
            "topic.json",
            "--category",
            "C",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn posts_delete_with_id() {
        let cli = parse_args(&["posts", "delete", "--id", "7"]).unwrap();
        match cli.command {
            Command::Posts {
                action: PostsAction::Delete { id },
            } => assert_eq!(id, 7),
            _ => panic!("Expected posts delete"),
        }
    }

    #[test]
    fn comments_create_with_args() {
        let cli =
            parse_args(&["comments", "create", "--topic-id", "10", "--raw", "A reply"]).unwrap();
        match cli.command {
            Command::Comments {
                action: CommentsAction::Create { topic_id, raw },
            } => {
                assert_eq!(topic_id, 10);
                assert_eq!(raw, "A reply");
            }
            _ => panic!("Expected comments create"),
        }
    }

    #[test]
    fn comments_delete_with_id() {
        let cli = parse_args(&["comments", "delete", "--id", "55"]).unwrap();
        match cli.command {
            Command::Comments {
                action: CommentsAction::Delete { id },
            } => assert_eq!(id, 55),
            _ => panic!("Expected comments delete"),
        }
    }

    #[test]
    fn human_flag_global() {
        let cli = parse_args(&["--human", "posts", "get", "--id", "1"]).unwrap();
        assert!(cli.human);
    }

    #[test]
    fn instance_flag_global() {
        let cli = parse_args(&["--instance", "my-forum", "posts", "get", "--id", "1"]).unwrap();
        assert_eq!(cli.instance.as_deref(), Some("my-forum"));
    }

    #[test]
    fn schema_subcommand_parses() {
        let cli = parse_args(&["schema"]).unwrap();
        assert!(matches!(cli.command, Command::Schema));
    }

    #[test]
    fn no_subcommand_shows_error() {
        assert!(parse_args(&[]).is_err());
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
