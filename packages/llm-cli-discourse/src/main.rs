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
            suggestion: "Check the ID and try again".to_string(),
        },
        human,
    }
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
        clap_complete::generate(
            *shell,
            &mut cmd,
            "llm-cli-discourse",
            &mut std::io::stdout(),
        );
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
            cli::PostsAction::Latest { page } => {
                let response = client
                    .list_latest_posts(page)
                    .map_err(|e| api_error_to_cli(e, human))?;
                if human {
                    output::format_latest_posts_human(&response)
                } else {
                    let pagination = if response.has_more {
                        Some(output::Pagination {
                            has_more: true,
                            next_cursor: response.next_page.map(|p| p.to_string()),
                        })
                    } else {
                        None
                    };
                    format!(
                        "{}\n",
                        output::format_success_with_pagination(&response, pagination.as_ref())
                    )
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
                input,
            } => {
                let (title, category, raw) = if let Some(ref source) = input {
                    let json = read_json_input(source).map_err(|e| api_error_to_cli(e, human))?;
                    let t =
                        required_string(&json, "title").map_err(|e| api_error_to_cli(e, human))?;
                    let c = required_string(&json, "category")
                        .map_err(|e| api_error_to_cli(e, human))?;
                    let r = json
                        .get("raw")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (t, c, r)
                } else {
                    (title.unwrap(), category.unwrap(), raw)
                };
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
        assert_eq!(schema["name"], "llm-cli-discourse");
        assert!(schema.get("version").is_some());
        assert!(schema.get("description").is_some());
        assert!(schema.get("commands").is_some());
        assert!(schema.get("global_args").is_some());
    }

    #[test]
    fn build_schema_contains_posts_and_comments_commands() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let commands = schema.get("commands").unwrap();
        assert!(commands.get("posts").is_some());
        assert!(commands.get("comments").is_some());
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
        let dir = std::env::temp_dir().join("llm-cli-discourse-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_input.json");
        std::fs::write(&path, r#"{"title": "My Topic", "category": "general"}"#).unwrap();
        let result = read_json_input(path.to_str().unwrap()).unwrap();
        assert_eq!(result["title"], "My Topic");
        assert_eq!(result["category"], "general");
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
        let json = serde_json::json!({"title": "Hello"});
        assert_eq!(required_string(&json, "title").unwrap(), "Hello");
    }

    #[test]
    fn required_string_missing_field() {
        let json = serde_json::json!({});
        let err = required_string(&json, "title").unwrap_err();
        assert!(err.contains("Missing required field 'title'"));
    }

    #[test]
    fn build_schema_global_args_include_human_debug_instance() {
        let cmd = <cli::Cli as clap::CommandFactory>::command();
        let schema = build_schema(&cmd);
        let global_args = schema.get("global_args").unwrap();
        assert!(global_args.get("--human").is_some());
        assert!(global_args.get("--debug").is_some());
        assert!(global_args.get("--instance").is_some());
    }
}
