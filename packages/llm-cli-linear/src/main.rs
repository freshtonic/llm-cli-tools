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

/// Map a config error to a CliError with appropriate code, message, and suggestion.
fn config_error_to_cli(e: config::ConfigError, human: bool) -> output::CliError {
    let config_path = config::config_path();
    let detail = match e {
        config::ConfigError::NotFound(_) => output::ErrorDetail {
            code: "CONFIG_NOT_FOUND",
            message: format!("Config file not found at {}", config_path.display()),
            suggestion: format!(
                "Create a config file at {} with:\n\n[linear]\nop_item_id = \"<your-1password-item-id>\"",
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
            message: "Missing [linear] section in config file".to_string(),
            suggestion: format!(
                "Add a [linear] section to {}:\n\n[linear]\nop_item_id = \"<your-1password-item-id>\"",
                config_path.display()
            ),
        },
        config::ConfigError::MissingOpItemId => output::ErrorDetail {
            code: "CONFIG_MISSING_OP_ITEM_ID",
            message: "Missing op_item_id in [linear] config section".to_string(),
            suggestion: "Add op_item_id to the [linear] section in your config file".to_string(),
        },
    };
    output::CliError { detail, human }
}

/// Map a credential error to a CliError.
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

/// Map an API error string to a CliError.
fn api_error_to_cli(msg: String, human: bool) -> output::CliError {
    output::CliError {
        detail: output::ErrorDetail {
            code: "API_ERROR",
            message: msg,
            suggestion: "Check the issue identifier and try again".to_string(),
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

    // Load config.
    let cfg = config::load().map_err(|e| config_error_to_cli(e, human))?;

    // Retrieve API key from 1Password.
    let api_key = credential::get_api_key(&cfg.op_item_id, &cfg.op_field)
        .map_err(|e| credential_error_to_cli(e, &cfg.op_item_id, human))?;

    let out = match args.command {
        cli::Command::Issues { action } => match action {
            cli::IssuesAction::List {
                limit,
                mine,
                team,
                state,
            } => {
                let mut filters = api::IssueFilters {
                    team_key: team,
                    state_name: state,
                    ..Default::default()
                };

                if mine {
                    let viewer_req = api::build_viewer_id_query();
                    let viewer_resp =
                        api::execute(&cfg.api_url, &api_key, &viewer_req, debug.as_ref())
                            .map_err(|e| api_error_to_cli(e, human))?;
                    let viewer_id = api::parse_viewer_id(&viewer_resp)
                        .map_err(|e| api_error_to_cli(e, human))?;
                    filters.assignee_id = Some(viewer_id);
                }

                let request = api::build_list_query(limit, &filters);
                let response = api::execute(&cfg.api_url, &api_key, &request, debug.as_ref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                let result = api::parse_list_response(&response, limit)
                    .map_err(|e| api_error_to_cli(e, human))?;

                if human {
                    output::format_issue_list_human(&result)
                } else {
                    format!("{}\n", output::format_success(&result))
                }
            }
            cli::IssuesAction::Get { id } => {
                let request = api::build_get_query(&id);
                let response = api::execute(&cfg.api_url, &api_key, &request, debug.as_ref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                let issue =
                    api::parse_get_response(&response).map_err(|e| api_error_to_cli(e, human))?;

                if human {
                    format!("{}\n", output::format_issue_human(&issue))
                } else {
                    format!("{}\n", output::format_success(&issue))
                }
            }
            cli::IssuesAction::Create {
                title,
                team,
                description,
                priority,
            } => {
                let request =
                    api::build_create_mutation(&title, &team, description.as_deref(), priority);
                let response = api::execute(&cfg.api_url, &api_key, &request, debug.as_ref())
                    .map_err(|e| api_error_to_cli(e, human))?;
                let issue = api::parse_create_response(&response)
                    .map_err(|e| api_error_to_cli(e, human))?;

                if human {
                    format!("Created: {}\n", output::format_issue_human(&issue))
                } else {
                    format!("{}\n", output::format_success(&issue))
                }
            }
            cli::IssuesAction::Close { id } => {
                let team_request = api::build_issue_team_query(&id);
                let team_response =
                    api::execute(&cfg.api_url, &api_key, &team_request, debug.as_ref())
                        .map_err(|e| api_error_to_cli(e, human))?;
                let (issue_id, done_state_id) = api::parse_done_state_id(&team_response)
                    .map_err(|e| api_error_to_cli(e, human))?;

                let close_request = api::build_close_mutation(&issue_id, &done_state_id);
                let close_response =
                    api::execute(&cfg.api_url, &api_key, &close_request, debug.as_ref())
                        .map_err(|e| api_error_to_cli(e, human))?;
                let issue = api::parse_close_response(&close_response)
                    .map_err(|e| api_error_to_cli(e, human))?;

                if human {
                    format!("Closed: {}\n", output::format_issue_human(&issue))
                } else {
                    format!("{}\n", output::format_success(&issue))
                }
            }
        },
    };

    pager::print_with_pager(&out, human, debug.is_some());
    Ok(())
}
