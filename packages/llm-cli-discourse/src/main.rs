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
                 [discourse.my-forum]\n\
                 base_url = \"https://forum.example.com\"\n\
                 op_item_id = \"<your-1password-item-id>\"\n\
                 api_username = \"<your-username>\"",
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
        config::ConfigError::NoInstances => output::ErrorDetail {
            code: "CONFIG_NO_INSTANCES",
            message: "No [discourse.*] sections found in config file".to_string(),
            suggestion: format!(
                "Add a Discourse instance to {}:\n\n\
                 [discourse.my-forum]\n\
                 base_url = \"https://forum.example.com\"\n\
                 op_item_id = \"<your-1password-item-id>\"\n\
                 api_username = \"<your-username>\"",
                config_path.display()
            ),
        },
        config::ConfigError::InstanceNotFound(name) => output::ErrorDetail {
            code: "CONFIG_INSTANCE_NOT_FOUND",
            message: format!("Discourse instance '{name}' not found in config"),
            suggestion: format!(
                "Add [discourse.{name}] to {} or use a different --instance value",
                config_path.display()
            ),
        },
        config::ConfigError::AmbiguousInstance(names) => output::ErrorDetail {
            code: "CONFIG_AMBIGUOUS_INSTANCE",
            message: format!(
                "Multiple Discourse instances configured: {}",
                names.join(", ")
            ),
            suggestion: format!(
                "Use --instance <name> to select one. Available: {}",
                names.join(", ")
            ),
        },
        config::ConfigError::MissingField { instance, field } => output::ErrorDetail {
            code: "CONFIG_MISSING_FIELD",
            message: format!("Missing '{field}' in [discourse.{instance}] config section"),
            suggestion: format!(
                "Add {field} to [discourse.{instance}] in {}",
                config_path.display()
            ),
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
            suggestion: "Check the ID and try again".to_string(),
        },
        human,
    }
}

fn run(args: cli::Cli) -> Result<(), output::CliError> {
    let human = args.human;
    let debug = args
        .debug
        .map(|s| cli::DebugConfig::parse(&s))
        .transpose()
        .map_err(|e| output::CliError {
            detail: output::ErrorDetail {
                code: "INVALID_DEBUG_MODE",
                message: e,
                suggestion: "Valid modes: compact, pretty, curl_cmd".to_string(),
            },
            human,
        })?;

    let cfg = config::load(args.instance.as_deref()).map_err(|e| config_error_to_cli(e, human))?;

    let api_key = credential::get_api_key(&cfg.op_item_id, &cfg.op_field)
        .map_err(|e| credential_error_to_cli(e, &cfg.op_item_id, human))?;

    let debug_active = debug.is_some();
    let client = api::Client {
        base_url: cfg.base_url,
        api_key,
        api_username: cfg.api_username,
        debug,
    };

    let out = match args.command {
        cli::Command::Posts { action } => match action {
            cli::PostsAction::Latest => {
                let response = client
                    .list_latest_posts()
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    output::format_latest_posts_human(&response)
                } else {
                    format!("{}\n", output::format_success(&response))
                }
            }
            cli::PostsAction::Get { id } => {
                let response = client
                    .get_topic(id)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    output::format_topic_human(&response)
                } else {
                    format!("{}\n", output::format_success(&response))
                }
            }
            cli::PostsAction::Create {
                title,
                category,
                raw,
            } => {
                let response = client
                    .create_topic(&title, &category, raw.as_deref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("Created: {}\n", output::format_post_human(&response))
                } else {
                    format!("{}\n", output::format_success(&response))
                }
            }
            cli::PostsAction::Delete { id } => {
                client
                    .delete_topic(id)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("Deleted topic #{id}\n")
                } else {
                    format!(
                        "{}\n",
                        output::format_success(&serde_json::json!({
                            "deleted": true,
                            "topic_id": id
                        }))
                    )
                }
            }
        },
        cli::Command::Comments { action } => match action {
            cli::CommentsAction::Create { topic_id, raw } => {
                let response = client
                    .create_reply(topic_id, &raw)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("Replied: {}\n", output::format_post_human(&response))
                } else {
                    format!("{}\n", output::format_success(&response))
                }
            }
            cli::CommentsAction::Delete { id } => {
                client
                    .delete_post(id)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    format!("Deleted comment #{id}\n")
                } else {
                    format!(
                        "{}\n",
                        output::format_success(&serde_json::json!({
                            "deleted": true,
                            "post_id": id
                        }))
                    )
                }
            }
        },
    };

    pager::print_with_pager(&out, human, debug_active);
    Ok(())
}
