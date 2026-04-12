# Contributing to Molt

## Development setup

```bash
git clone git@github.com:vincentllm/molt-cli-c.git
cd molt-cli-c
cargo build
```

See [docs/building.md](docs/building.md) for platform-specific instructions.

## Project conventions

- **Rust edition 2021**, stable toolchain
- No `unwrap()` in command handlers — use `anyhow::Result` and propagate errors
- Color output via `colored` crate; tables via `comfy-table`; spinners via `indicatif`
- All user-facing strings use Chinese (this is a Feishu/China-market tool)
- No secrets in source code — credentials live in `~/.molt/config.yaml` only

## Adding a feature

1. Read [docs/architecture.md](docs/architecture.md) first
2. Follow the extensibility patterns documented there
3. Keep PRs focused — one feature per PR
4. Update the relevant doc in `docs/` if behavior changes

## Commit style

```
Day N: <short description>

<optional longer explanation>
```

## Security

- Never commit API keys, app secrets, or tokens
- `~/.molt/config.yaml` is outside the repo by design
- Run `git diff --staged` before every commit to verify no secrets slip in
