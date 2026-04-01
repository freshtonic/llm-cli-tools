//! CLI argument parsing using clap derive.
//!
//! Subcommands: `messages send|read|dm|mentions`, `summary`.
//! Global flag: `--human` for human-readable output.

use clap::{Parser, Subcommand, ValueEnum};

/// Debug output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DebugMode {
    /// Compact single-line output.
    Compact,
    /// Pretty-printed JSON bodies.
    Pretty,
}

#[derive(Debug, Parser)]
#[command(
    name = "llm-cli-slack",
    about = "CLI tool for interacting with the Slack API. Returns JSON by default. \
             Use --human for human-readable output. Retrieves API credentials from \
             1Password at call time."
)]
pub struct Cli {
    /// Output human-readable text instead of JSON.
    #[arg(long, global = true)]
    pub human: bool,

    /// Print raw HTTP requests and responses to stderr.
    /// Use --debug for compact output, --debug=pretty for formatted bodies.
    #[arg(long, global = true, default_missing_value = "compact", num_args = 0..=1, require_equals = true)]
    pub debug: Option<DebugMode>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Send and read Slack messages.
    Messages {
        #[command(subcommand)]
        action: MessagesAction,
    },
    /// Get Slack AI-generated channel summary for a date range.
    /// Defaults to today and yesterday.
    Summary {
        /// Channel name or ID.
        #[arg(long)]
        channel: String,
        /// Oldest date to summarize (ISO 8601, e.g. 2026-03-30). Defaults to yesterday.
        #[arg(long)]
        oldest: Option<String>,
        /// Latest date to summarize (ISO 8601, e.g. 2026-04-01). Defaults to today.
        #[arg(long)]
        latest: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum MessagesAction {
    /// Send a message to a channel. Use --thread-ts to reply in a thread.
    Send {
        /// Channel name or ID.
        #[arg(long)]
        channel: String,
        /// Message text.
        #[arg(long)]
        text: String,
        /// Thread timestamp to reply to (makes this a threaded reply).
        #[arg(long)]
        thread_ts: Option<String>,
    },
    /// Read recent messages from a channel.
    Read {
        /// Channel name or ID.
        #[arg(long)]
        channel: String,
        /// Maximum number of messages to return (default: 25).
        #[arg(long, default_value = "25")]
        limit: u32,
    },
    /// Send a direct message to a user.
    Dm {
        /// User ID or email.
        #[arg(long)]
        user: String,
        /// Message text.
        #[arg(long)]
        text: String,
    },
    /// Retrieve messages mentioning the authenticated user, including DMs.
    Mentions {
        /// Maximum number of messages to return (default: 25).
        #[arg(long, default_value = "25")]
        limit: u32,
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
        Cli::try_parse_from(std::iter::once("llm-cli-slack").chain(args.iter().copied()))
    }

    #[test]
    fn messages_send_to_channel() {
        let cli =
            parse_args(&["messages", "send", "--channel", "general", "--text", "hello"]).unwrap();
        match cli.command {
            Command::Messages {
                action:
                    MessagesAction::Send {
                        channel,
                        text,
                        thread_ts,
                    },
            } => {
                assert_eq!(channel, "general");
                assert_eq!(text, "hello");
                assert!(thread_ts.is_none());
            }
            _ => panic!("Expected messages send"),
        }
    }

    #[test]
    fn messages_send_to_thread() {
        let cli = parse_args(&[
            "messages",
            "send",
            "--channel",
            "general",
            "--text",
            "reply",
            "--thread-ts",
            "1234567890.123456",
        ])
        .unwrap();
        match cli.command {
            Command::Messages {
                action:
                    MessagesAction::Send {
                        thread_ts: Some(ts),
                        ..
                    },
            } => assert_eq!(ts, "1234567890.123456"),
            _ => panic!("Expected messages send with thread_ts"),
        }
    }

    #[test]
    fn messages_read_defaults() {
        let cli = parse_args(&["messages", "read", "--channel", "general"]).unwrap();
        match cli.command {
            Command::Messages {
                action: MessagesAction::Read { channel, limit },
            } => {
                assert_eq!(channel, "general");
                assert_eq!(limit, 25);
            }
            _ => panic!("Expected messages read"),
        }
    }

    #[test]
    fn messages_read_custom_limit() {
        let cli =
            parse_args(&["messages", "read", "--channel", "general", "--limit", "10"]).unwrap();
        match cli.command {
            Command::Messages {
                action: MessagesAction::Read { limit, .. },
            } => assert_eq!(limit, 10),
            _ => panic!("Expected messages read"),
        }
    }

    #[test]
    fn messages_dm() {
        let cli = parse_args(&["messages", "dm", "--user", "U12345", "--text", "hey"]).unwrap();
        match cli.command {
            Command::Messages {
                action: MessagesAction::Dm { user, text },
            } => {
                assert_eq!(user, "U12345");
                assert_eq!(text, "hey");
            }
            _ => panic!("Expected messages dm"),
        }
    }

    #[test]
    fn messages_mentions_defaults() {
        let cli = parse_args(&["messages", "mentions"]).unwrap();
        match cli.command {
            Command::Messages {
                action: MessagesAction::Mentions { limit },
            } => assert_eq!(limit, 25),
            _ => panic!("Expected messages mentions"),
        }
    }

    #[test]
    fn summary_defaults() {
        let cli = parse_args(&["summary", "--channel", "general"]).unwrap();
        match cli.command {
            Command::Summary {
                channel,
                oldest,
                latest,
            } => {
                assert_eq!(channel, "general");
                assert!(oldest.is_none());
                assert!(latest.is_none());
            }
            _ => panic!("Expected summary"),
        }
    }

    #[test]
    fn summary_with_dates() {
        let cli = parse_args(&[
            "summary",
            "--channel",
            "general",
            "--oldest",
            "2026-03-30",
            "--latest",
            "2026-04-01",
        ])
        .unwrap();
        match cli.command {
            Command::Summary {
                oldest: Some(o),
                latest: Some(l),
                ..
            } => {
                assert_eq!(o, "2026-03-30");
                assert_eq!(l, "2026-04-01");
            }
            _ => panic!("Expected summary with dates"),
        }
    }

    #[test]
    fn human_flag_global() {
        let cli = parse_args(&["--human", "messages", "mentions"]).unwrap();
        assert!(cli.human);
    }

    #[test]
    fn no_subcommand_shows_error() {
        assert!(parse_args(&[]).is_err());
    }
}
