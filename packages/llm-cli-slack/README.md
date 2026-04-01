# llm-cli-slack

A CLI tool for interacting with the Slack API - designed to be used by AI agents.

Returns JSON by default. Use `--human` for human-readable output. Retrieves API credentials from 1Password at call time.

## Usage

```sh
llm-cli-slack messages send --channel general --text "hello"
llm-cli-slack messages read --channel general
llm-cli-slack messages dm --user U12345 --text "hey"
llm-cli-slack messages mentions
llm-cli-slack summary --channel general
```

## Configuration

Add to `~/.config/llm-cli/config.toml`:

```toml
[slack]
op_item_id = "your-1password-item-id"
```

## Install

```sh
cargo install --path .
```

## License

MIT
