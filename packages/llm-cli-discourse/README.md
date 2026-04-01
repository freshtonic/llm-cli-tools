# llm-cli-discourse

A CLI tool for interacting with the Discourse API - designed to be used by AI agents.

Returns JSON by default. Use `--human` for human-readable output. Retrieves API credentials from 1Password at call time. Supports multiple instances via `--instance`.

## Usage

```sh
llm-cli-discourse posts latest
llm-cli-discourse posts get --id 42
llm-cli-discourse posts create --title "Topic" --category general --raw "Body"
llm-cli-discourse comments create --topic-id 42 --raw "Reply"
```

## Configuration

Add to `~/.config/llm-cli/config.toml`:

```toml
[discourse.my-forum]
base_url = "https://forum.example.com"
op_item_id = "your-1password-item-id"
api_username = "your-username"
```

## Install

```sh
cargo install --path .
```

## License

MIT
