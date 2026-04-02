mod api;
mod cli;
mod config;
mod credential;
mod output;
mod pager;

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

/// Read JSON input from a file path or stdin ("-").
fn read_json_input(source: &str) -> Result<serde_json::Value, String> {
    let content = if source == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(source)
            .map_err(|e| format!("Failed to read file '{source}': {e}"))?
    };
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON input: {e}"))
}

/// Extract a required string field from a JSON value.
fn required_string(json: &serde_json::Value, field: &str) -> Result<String, String> {
    json.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required field '{field}' in JSON input"))
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

    (days_to_date(yesterday_days), days_to_date(today_days))
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

/// Build a JSON schema description of the CLI for automated discovery.
fn build_schema(cmd: &clap::Command) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert("name".to_string(), serde_json::json!(cmd.get_name()));
    if let Some(version) = cmd.get_version() {
        root.insert("version".to_string(), serde_json::json!(version));
    }
    if let Some(about) = cmd.get_about() {
        root.insert(
            "description".to_string(),
            serde_json::json!(about.to_string()),
        );
    }

    let global_args = build_args_schema(cmd);
    if !global_args.is_empty() {
        root.insert(
            "global_args".to_string(),
            serde_json::Value::Object(global_args),
        );
    }

    let commands = build_subcommands_schema(cmd);
    if !commands.is_empty() {
        root.insert("commands".to_string(), serde_json::Value::Object(commands));
    }

    serde_json::Value::Object(root)
}

fn build_args_schema(cmd: &clap::Command) -> serde_json::Map<String, serde_json::Value> {
    let mut args_map = serde_json::Map::new();
    for arg in cmd.get_arguments() {
        let id = arg.get_id().as_str();
        if id == "help" || id == "version" {
            continue;
        }
        let mut arg_obj = serde_json::Map::new();

        let type_name = if arg.get_action().takes_values() {
            "string"
        } else {
            "boolean"
        };
        arg_obj.insert("type".to_string(), serde_json::json!(type_name));

        if let Some(help) = arg.get_help() {
            arg_obj.insert(
                "description".to_string(),
                serde_json::json!(help.to_string()),
            );
        }

        let defaults: Vec<&str> = arg
            .get_default_values()
            .iter()
            .filter_map(|v| v.to_str())
            .collect();
        if !defaults.is_empty() {
            arg_obj.insert("default".to_string(), serde_json::json!(defaults.join(",")));
        }

        if arg.is_required_set() {
            arg_obj.insert("required".to_string(), serde_json::json!(true));
        }

        let flag_name = format!("--{id}");
        args_map.insert(flag_name, serde_json::Value::Object(arg_obj));
    }
    args_map
}

fn build_subcommands_schema(cmd: &clap::Command) -> serde_json::Map<String, serde_json::Value> {
    let mut commands_map = serde_json::Map::new();
    for sub in cmd.get_subcommands() {
        let name = sub.get_name();
        if name == "help" || name == "completions" || name == "schema" {
            continue;
        }

        let mut sub_obj = serde_json::Map::new();
        if let Some(about) = sub.get_about() {
            sub_obj.insert(
                "description".to_string(),
                serde_json::json!(about.to_string()),
            );
        }

        let args = build_args_schema(sub);
        if !args.is_empty() {
            sub_obj.insert("args".to_string(), serde_json::Value::Object(args));
        }

        let nested = build_subcommands_schema(sub);
        if !nested.is_empty() {
            sub_obj.insert("subcommands".to_string(), serde_json::Value::Object(nested));
        }

        commands_map.insert(name.to_string(), serde_json::Value::Object(sub_obj));
    }
    commands_map
}

fn run(args: cli::Cli) -> Result<(), output::CliError> {
    if let cli::Command::Completions { shell } = &args.command {
        let mut cmd = <cli::Cli as clap::CommandFactory>::command();
        clap_complete::generate(*shell, &mut cmd, "llm-cli-slack", &mut std::io::stdout());
        return Ok(());
    }

    if let cli::Command::Schema = &args.command {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        return Ok(());
    }

    let human = args.human;
    let debug = args
        .debug
        .map(|s| cli::DebugConfig::parse(&s))
        .transpose()
        .map_err(|e| output::CliError {
            detail: output::ErrorDetail {
                code: "INVALID_DEBUG_MODE",
                message: e,
                suggestion: "Valid modes: compact, pretty, curl, dangerous_no_redact".to_string(),
            },
            human,
        })?;

    let cfg = config::load().map_err(|e| config_error_to_cli(e, human))?;

    let token = credential::get_api_key(&cfg.op_item_id, &cfg.op_field)
        .map_err(|e| credential_error_to_cli(e, &cfg.op_item_id, human))?;

    let debug_active = debug.is_some();
    let client = api::Client { token, debug };

    let out = match args.command {
        cli::Command::Messages { action } => match action {
            cli::MessagesAction::Send {
                channel,
                text,
                thread_ts,
                input,
            } => {
                let (channel, text, thread_ts) = if let Some(ref source) = input {
                    let json = read_json_input(source).map_err(|e| api_error_to_cli(e, human))?;
                    let ch = required_string(&json, "channel")
                        .map_err(|e| api_error_to_cli(e, human))?;
                    let tx =
                        required_string(&json, "text").map_err(|e| api_error_to_cli(e, human))?;
                    let ts = json
                        .get("thread_ts")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (ch, tx, ts)
                } else {
                    (channel.unwrap(), text.unwrap(), thread_ts)
                };
                let result = client
                    .send_message(&channel, &text, thread_ts.as_deref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("{}\n", output::format_send_human(&result))
                } else {
                    format!("{}\n", output::format_success(&result))
                }
            }
            cli::MessagesAction::Read {
                channel,
                limit,
                cursor,
                oldest,
                latest,
            } => {
                let result = client
                    .read_history(
                        &channel,
                        limit,
                        cursor.as_deref(),
                        oldest.as_deref(),
                        latest.as_deref(),
                    )
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    output::format_history_human(&result)
                } else {
                    let pagination = if result.has_more {
                        Some(output::Pagination {
                            has_more: true,
                            next_cursor: result.next_cursor.clone(),
                        })
                    } else {
                        None
                    };
                    format!(
                        "{}\n",
                        output::format_success_with_pagination(&result, pagination.as_ref())
                    )
                }
            }
            cli::MessagesAction::Dm { user, text } => {
                let result = client
                    .send_dm(&user, &text)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("{}\n", output::format_send_human(&result))
                } else {
                    format!("{}\n", output::format_success(&result))
                }
            }
            cli::MessagesAction::Mentions { limit } => {
                let result = client
                    .search_mentions(limit)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    output::format_search_human(&result)
                } else {
                    format!("{}\n", output::format_success(&result))
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
                format!("{}\n", output::format_summary_human(&result))
            } else {
                format!("{}\n", output::format_success(&result))
            }
        }
        cli::Command::Completions { .. } | cli::Command::Schema => unreachable!(),
    };

    pager::print_with_pager(&out, human, debug_active);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_schema_contains_top_level_fields() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        assert_eq!(schema["name"], "llm-cli-slack");
        assert!(schema.get("version").is_some());
        assert!(schema.get("description").is_some());
        assert!(schema.get("commands").is_some());
        assert!(schema.get("global_args").is_some());
    }

    #[test]
    fn build_schema_contains_messages_command() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let commands = schema.get("commands").unwrap();
        assert!(commands.get("messages").is_some());
    }

    #[test]
    fn build_schema_contains_summary_command() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let commands = schema.get("commands").unwrap();
        assert!(commands.get("summary").is_some());
    }

    #[test]
    fn build_schema_excludes_help_completions_schema() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let commands = schema.get("commands").unwrap();
        assert!(commands.get("completions").is_none());
        assert!(commands.get("schema").is_none());
        assert!(commands.get("help").is_none());
    }

    #[test]
    fn build_schema_is_valid_json() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let json_str = serde_json::to_string_pretty(&schema).unwrap();
        let _: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    }

    #[test]
    fn read_json_input_from_file() {
        let dir = std::env::temp_dir().join("llm-cli-slack-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_input.json");
        std::fs::write(&path, r#"{"channel": "general", "text": "hello"}"#).unwrap();
        let result = read_json_input(path.to_str().unwrap()).unwrap();
        assert_eq!(result["channel"], "general");
        assert_eq!(result["text"], "hello");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_json_input_file_not_found() {
        let result = read_json_input("/nonexistent/path.json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read file"));
    }

    #[test]
    fn required_string_extracts_field() {
        let json = serde_json::json!({"channel": "general"});
        assert_eq!(required_string(&json, "channel").unwrap(), "general");
    }

    #[test]
    fn required_string_missing_field() {
        let json = serde_json::json!({});
        let err = required_string(&json, "channel").unwrap_err();
        assert!(err.contains("Missing required field 'channel'"));
    }

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
