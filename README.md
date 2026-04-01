# llm-cli-tools

A suite of CLI tools designed for LLM agents to interact with SaaS APIs. JSON output by default, `--human` flag for readable output.

## Tools

| Binary | Service | API |
|--------|---------|-----|
| `llm-cli` | Dispatcher | Execs `llm-cli-<subcommand>` from `$PATH` |
| `llm-cli-linear` | [Linear](https://linear.app) | GraphQL |
| `llm-cli-discourse` | [Discourse](https://www.discourse.org) | REST |
| `llm-cli-slack` | [Slack](https://slack.com) | REST |

## Install

```sh
./install.sh
```

This discovers all binary crates in the workspace and runs `cargo install --path` for each.

## Configuration

All tools read from `~/.config/llm-cli/config.toml` (or `$XDG_CONFIG_HOME/llm-cli/config.toml`).

```toml
[linear]
op_item_id = "your-1password-item-id"

[discourse.my-forum]
base_url = "https://forum.example.com"
op_item_id = "your-1password-item-id"
api_username = "your-username"

[slack]
op_item_id = "your-1password-item-id"
```

API keys are retrieved from 1Password at call time via the `op` CLI. Each config section requires an `op_item_id` pointing to a 1Password item. The key is read from the `credential` field by default; set `op_field` to use a different field.

## Usage

```sh
# Dispatcher
llm-cli linear issues list
llm-cli discourse posts latest

# Direct invocation
llm-cli-linear issues list --limit 10
llm-cli-linear issues get --id PROJ-123
llm-cli-linear issues create --title "Bug" --team ENG
llm-cli-linear issues close --id PROJ-123

llm-cli-discourse posts latest
llm-cli-discourse posts get --id 42
llm-cli-discourse posts create --title "Topic" --category general --raw "Body"
llm-cli-discourse comments create --post-id 42 --raw "Reply"

llm-cli-slack messages send --channel general --text "hello"
llm-cli-slack messages read --channel general
llm-cli-slack messages dm --user U12345 --text "hey"
llm-cli-slack messages mentions
llm-cli-slack summary --channel general
```

## Common flags

- `--human` — human-readable output instead of JSON
- `--debug` — log HTTP requests/responses to stderr
- `--debug=pretty` — pretty-print JSON bodies and GraphQL queries
- `--debug=curl_cmd` — print reproducible curl commands (warns about unredacted secrets)
- `--debug=pretty,curl_cmd` — both

## Design principles

See [PRINCIPLES.md](PRINCIPLES.md) for the CLI design philosophy. These tools are agent-first: JSON output, structured errors with suggestions, named flags, no interactive prompts.

## Project structure

```
packages/
  llm-cli/           # Dispatcher (std only, no deps)
  llm-cli-linear/    # Linear GraphQL client
  llm-cli-discourse/ # Discourse REST client
  llm-cli-slack/     # Slack REST client
docs/
  plans/             # Design documents
```
