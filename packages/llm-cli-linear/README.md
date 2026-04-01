# llm-cli-linear

A CLI tool for interacting with the Linear API - designed to be used by AI agents.

Returns JSON by default. Use `--human` for human-readable output. Retrieves API credentials from 1Password at call time.

## Usage

```sh
llm-cli-linear issues list --limit 10
llm-cli-linear issues get --id PROJ-123
llm-cli-linear issues create --title "Bug" --team ENG
llm-cli-linear issues close --id PROJ-123
```

## Configuration

Add to `~/.config/llm-cli/config.toml`:

```toml
[linear]
op_item_id = "your-1password-item-id"
```

## Install

```sh
cargo install --path .
```

## License

MIT
