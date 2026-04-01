mod api;
mod cli;
mod config;
mod credential;
mod output;

fn main() {
    let args = cli::parse();
    match run(args) {
        Ok(()) => {}
        Err(e) => {
            e.render();
            std::process::exit(e.exit_code());
        }
    }
}

fn config_error_to_cli(e: config::ConfigError, human: bool) -> output::CliError {
    let config_path = config::config_path();
    let detail = match e {
        config::ConfigError::NotFound(_) => output::ErrorDetail {
            code: "CONFIG_NOT_FOUND",
            message: format!("Config file not found at {}", config_path.display()),
            suggestion: format!(
                "Create a config file at {} with:\n\n\
                 [slack]\n\
                 op_item_id = \"<your-1password-item-id>\"",
                config_path.display()
            ),
        },
        config::ConfigError::ParseError(msg) => output::ErrorDetail {
            code: "CONFIG_PARSE_ERROR",
            message: format!(
                "Failed to parse config file at {}: {msg}",
                config_path.display()
            ),
            suggestion: "Check the config file for syntax errors".to_string(),
        },
        config::ConfigError::MissingSection => output::ErrorDetail {
            code: "CONFIG_MISSING_SECTION",
            message: "Missing [slack] section in config file".to_string(),
            suggestion: format!(
                "Add a [slack] section to {}:\n\n\
                 [slack]\n\
                 op_item_id = \"<your-1password-item-id>\"",
                config_path.display()
            ),
        },
        config::ConfigError::MissingOpItemId => output::ErrorDetail {
            code: "CONFIG_MISSING_OP_ITEM_ID",
            message: "Missing op_item_id in [slack] config section".to_string(),
            suggestion: "Add op_item_id to the [slack] section in your config file".to_string(),
        },
    };
    output::CliError { detail, human }
}

fn credential_error_to_cli(
    e: credential::CredentialError,
    op_item_id: &str,
    human: bool,
) -> output::CliError {
    let detail = match e {
        credential::CredentialError::OpNotFound => output::ErrorDetail {
            code: "OP_NOT_FOUND",
            message: "1Password CLI (op) not found on PATH".to_string(),
            suggestion:
                "Install the 1Password CLI: https://developer.1password.com/docs/cli/get-started/"
                    .to_string(),
        },
        credential::CredentialError::OpFailed(msg) => output::ErrorDetail {
            code: "OP_FAILED",
            message: format!(
                "Failed to retrieve API key from 1Password (item: {op_item_id}): {msg}"
            ),
            suggestion: "Ensure the 1Password desktop app is running and unlocked".to_string(),
        },
    };
    output::CliError { detail, human }
}

fn api_error_to_cli(msg: String, human: bool) -> output::CliError {
    output::CliError {
        detail: output::ErrorDetail {
            code: "API_ERROR",
            message: msg,
            suggestion: "Check the channel/user ID and try again".to_string(),
        },
        human,
    }
}

/// Parse an ISO 8601 date string to a Unix timestamp (start of day UTC).
fn date_to_timestamp(date: &str) -> Result<String, String> {
    // Expected format: YYYY-MM-DD
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!(
            "Invalid date format: '{date}'. Expected ISO 8601 (YYYY-MM-DD)."
        ));
    }
    let year: i64 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid year in date: '{date}'"))?;
    let month: i64 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid month in date: '{date}'"))?;
    let day: i64 = parts[2]
        .parse()
        .map_err(|_| format!("Invalid day in date: '{date}'"))?;

    // Simple days-from-epoch calculation (good enough for dates near present).
    // Uses the algorithm from https://howardhinnant.github.io/date_algorithms.html
    let y = if month <= 2 { year - 1 } else { year };
    let era = y / 400;
    let yoe = y - era * 400;
    let m = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let timestamp = days * 86400;

    Ok(timestamp.to_string())
}

/// Get yesterday's and today's dates as ISO 8601 strings.
fn default_date_range() -> (String, String) {
    // Read from system clock via a simple approach.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let today_days = now / 86400;
    let yesterday_days = today_days - 1;

    (
        days_to_date(yesterday_days),
        days_to_date(today_days),
    )
}

/// Convert days since Unix epoch to ISO 8601 date string.
fn days_to_date(days: i64) -> String {
    // Inverse of the date algorithm.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn run(args: cli::Cli) -> Result<(), output::CliError> {
    let human = args.human;
    let debug = args.debug;

    let cfg = config::load().map_err(|e| config_error_to_cli(e, human))?;

    let token = credential::get_api_key(&cfg.op_item_id)
        .map_err(|e| credential_error_to_cli(e, &cfg.op_item_id, human))?;

    let client = api::Client { token, debug };

    match args.command {
        cli::Command::Messages { action } => match action {
            cli::MessagesAction::Send {
                channel,
                text,
                thread_ts,
            } => {
                let result = client
                    .send_message(&channel, &text, thread_ts.as_deref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    println!("{}", output::format_send_human(&result));
                } else {
                    println!("{}", output::format_success(&result));
                }
            }
            cli::MessagesAction::Read { channel, limit } => {
                let result = client
                    .read_history(&channel, limit)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    print!("{}", output::format_history_human(&result));
                } else {
                    println!("{}", output::format_success(&result));
                }
            }
            cli::MessagesAction::Dm { user, text } => {
                let result = client
                    .send_dm(&user, &text)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    println!("{}", output::format_send_human(&result));
                } else {
                    println!("{}", output::format_success(&result));
                }
            }
            cli::MessagesAction::Mentions { limit } => {
                let result = client
                    .search_mentions(limit)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    print!("{}", output::format_search_human(&result));
                } else {
                    println!("{}", output::format_success(&result));
                }
            }
        },
        cli::Command::Summary {
            channel,
            oldest,
            latest,
        } => {
            let (default_oldest, default_latest) = default_date_range();
            let oldest_date = oldest.as_deref().unwrap_or(&default_oldest);
            let latest_date = latest.as_deref().unwrap_or(&default_latest);

            let oldest_ts =
                date_to_timestamp(oldest_date).map_err(|e| api_error_to_cli(e, human))?;
            let latest_ts =
                date_to_timestamp(latest_date).map_err(|e| api_error_to_cli(e, human))?;

            let result = client
                .get_summary(&channel, &oldest_ts, &latest_ts)
                .map_err(|e| api_error_to_cli(e, human))?;

            if human {
                println!("{}", output::format_summary_human(&result));
            } else {
                println!("{}", output::format_success(&result));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_to_timestamp_epoch() {
        assert_eq!(date_to_timestamp("1970-01-01").unwrap(), "0");
    }

    #[test]
    fn date_to_timestamp_known_date() {
        assert_eq!(date_to_timestamp("2026-04-01").unwrap(), "1775001600");
    }

    #[test]
    fn date_to_timestamp_invalid() {
        assert!(date_to_timestamp("not-a-date").is_err());
        assert!(date_to_timestamp("2026/04/01").is_err());
    }

    #[test]
    fn days_to_date_epoch() {
        assert_eq!(days_to_date(0), "1970-01-01");
    }

    #[test]
    fn days_to_date_roundtrip() {
        // 2026-04-01 should roundtrip.
        let ts: i64 = date_to_timestamp("2026-04-01").unwrap().parse().unwrap();
        let days = ts / 86400;
        assert_eq!(days_to_date(days), "2026-04-01");
    }
}
