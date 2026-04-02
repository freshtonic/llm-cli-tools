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

### Quick setup

Run the interactive setup wizard to generate your config file:

```sh
llm-cli init
```

This detects which `llm-cli-*` tools are installed, provides instructions for creating API keys, and prompts for the required configuration fields.

### Manual setup

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
llm-cli slack messages read --channel general

# Direct invocation
llm-cli-linear issues list --limit 10 --mine --team ENG
llm-cli-linear issues list --priority 1 --label bug
llm-cli-linear issues list --cursor <next_cursor>
llm-cli-linear issues get --id PROJ-123
llm-cli-linear issues create --title "Bug" --team ENG
llm-cli-linear issues create --input issue.json
llm-cli-linear issues close --id PROJ-123

llm-cli-discourse posts latest --page 2
llm-cli-discourse posts get --id 42
llm-cli-discourse posts create --title "Topic" --category general --raw "Body"
llm-cli-discourse posts create --input topic.json
llm-cli-discourse comments create --topic-id 42 --raw "Reply"

llm-cli-slack messages send --channel general --text "hello"
llm-cli-slack messages send --input message.json
llm-cli-slack messages read --channel general --oldest 1711900000 --latest 1711990000
llm-cli-slack messages read --channel general --cursor <next_cursor>
llm-cli-slack messages dm --user U12345 --text "hey"
llm-cli-slack messages mentions
llm-cli-slack summary --channel general
```

## Shell completions

`llm-cli completions` generates completions for the dispatcher **and** all installed `llm-cli-*` subcommands in a single script. One file gives you tab-completion for everything.

### Bash

```sh
llm-cli completions --shell bash > ~/.local/share/bash-completion/completions/llm-cli
```

### Zsh

```sh
# Ensure completions directory exists and is in fpath.
# Add to ~/.zshrc if not already present:
#   fpath=(~/.zfunc $fpath)
#   autoload -Uz compinit && compinit
mkdir -p ~/.zfunc
llm-cli completions --shell zsh > ~/.zfunc/_llm-cli
```

### Fish

```sh
llm-cli completions --shell fish > ~/.config/fish/completions/llm-cli.fish
```

Re-run after installing new subcommands to pick up their completions.

## Common flags

- `--human` — human-readable output instead of JSON
- `--debug` — log HTTP requests/responses to stderr
- `--debug=pretty` — pretty-print JSON bodies and GraphQL queries
- `--debug=curl` — print reproducible curl commands (secrets redacted by default)
- `--debug=dangerous_no_redact` — show secrets in debug output
- `--debug=curl,dangerous_no_redact` — curl commands with secrets exposed
- `--debug=pretty,curl` — pretty + curl, secrets redacted

## JSON output format

### Success

```json
{
  "success": true,
  "data": { ... }
}
```

List commands include a `pagination` object when more results are available:

```json
{
  "success": true,
  "data": { ... },
  "pagination": {
    "has_more": true,
    "next_cursor": "WyIyMDI2LTA0LTAxIl0"
  }
}
```

### Errors

Errors are output as structured JSON **to stdout** (not stderr) with a non-zero exit code:

```json
{
  "success": false,
  "error": {
    "code": "CONFIG_NOT_FOUND",
    "message": "Config file not found at ~/.config/llm-cli/config.toml",
    "suggestion": "Create a config file with..."
  }
}
```

In `--human` mode, errors go to stderr as plain text.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Unknown/general error |
| 2 | Configuration error (missing config file, bad TOML, missing section) |
| 3 | Authentication error (1Password CLI missing, credential retrieval failed) |
| 4 | API error (HTTP failure, bad response) |
| 5 | Invalid CLI input (bad debug mode) |

## JSON input

Create commands accept `--input <file>` for structured JSON input instead of individual flags. Use `--input -` to read from stdin:

```sh
echo '{"title": "Bug", "team": "ENG"}' | llm-cli-linear issues create --input -
llm-cli-slack messages send --input message.json
```

## Automated discovery

Each API tool has a `schema` subcommand that outputs a JSON description of available commands and arguments:

```sh
llm-cli-linear schema
llm-cli-discourse schema
llm-cli-slack schema
```

## Resilience

All API crates retry once with a 1-second backoff on transient HTTP errors (429 rate limits, 5xx server errors). Slack respects the `Retry-After` header when present. Destructive operations (delete) are not retried.

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
