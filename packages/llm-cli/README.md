# llm-cli

A CLI tool for dispatching to llm-cli-* subcommands - designed to be used by AI agents.

Git-style dispatcher that scans `$PATH` for `llm-cli-*` binaries and execs the matching subcommand.

## Usage

```sh
llm-cli linear issues list
llm-cli discourse posts latest
llm-cli slack messages read --channel general
```

## Install

```sh
cargo install --path .
```

## License

MIT
