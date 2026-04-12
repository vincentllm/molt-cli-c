# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build commands

This machine uses the **GNU toolchain** (msys64). Plain `cargo build` will fail because Git's `link.exe` shadows MSVC's linker. Always build with:

```bash
export PATH="/c/msys64/mingw64/bin:$PATH"
rustup run stable-x86_64-pc-windows-gnu cargo build --release
```

For WSL/Linux (production target), `cargo build --release` works directly.

There are no automated tests. To manually test a command after building:

```bash
./target/release/molt.exe --help
./target/release/molt.exe list
./target/release/molt.exe run --dry-run
```

## Architecture overview

Molt is a three-phase CLI: **record → extract → execute**.

**Phase 1 — Record** (`molt record` / `molt mark` / `molt stop`):  
On Unix, `molt record` spawns a native PTY session (`recording/pty_session.rs`) and writes the **recorder's own PID** (not the shell's) to `/tmp/molt_session.pid`. `molt stop` sends SIGTERM to that PID; the recorder's handler converts it to SIGHUP on the child shell (interactive bash/zsh ignore SIGTERM but respond to SIGHUP). On Windows, `asciinema rec` is used as a fallback. `molt mark` echoes `MOLT_MARK <index> <ISO8601> [label]` to stdout, captured inline — no IPC.

**Phase 2 — Extract** (`molt stop` continued):  
`src/recording/cast.rs` parses the `.cast` file (asciinema v2 format) into `MarkSlice` structs split at each `MOLT_MARK`. Each slice is ANSI-stripped and truncated to 2000 chars. These slices go to `AiBackend::extract_pipeline()` which returns raw AI text; `pipeline::store::extract_yaml_from_response()` pulls the YAML block out.

**Phase 3 — Execute** (`molt run`):  
Parsed `Pipeline` structs are dispatched per step by `executor` value: `"local"` → subprocess, `"feishu_bot"` → Interactive Card + poll, `"ask"` → inquire::Confirm. After execution, `RunRecord` is appended to `~/.molt/history.jsonl` (JSONL, one record per line).

## Module map

```
src/
  main.rs           — clap Commands enum, routes to commands/*
  config.rs         — MoltConfig / BackendConfig (serde tag = "type")
  session.rs        — /tmp path constants (CAST_FILE, PID_FILE, MARK_COUNT_FILE)
  history.rs        — RunRecord / StepRecord structs, append_run(), load_history()
  recording/
    cast.rs         — parse_cast() → Vec<MarkSlice>; MOLT_MARK regex split; merges VTE snapshots
    cast_writer.rs  — CastWriter: writes asciinema v2 .cast format (header + [t,"o",data] events)
    pty_session.rs  — native PTY recorder (Unix only): PTY pair, stdin/stdout threads, VTE feed
    virtual_screen.rs — VirtualScreen: 2D char grid; handles CUP/EL/ED/scroll/alt-screen/wide-chars
    stats.rs        — CastStats, command histogram, timeline rendering
  pipeline/
    schema.rs       — Pipeline / PipelineStep structs
    store.rs        — extract_yaml_from_response(), save_pipeline()
  backends/
    mod.rs          — AiBackend trait, build_backend() factory, build_extraction_prompt()
    direct_llm.rs   — HTTP to Anthropic or any OpenAI-compat endpoint
    feishu/
      client.rs     — FeishuClient: token auth, send_text, send_card, find_text_after (polling)
      extractor.rs  — AiBackend impl via [MOLT_REQUEST/RESPONSE:<id>] protocol
      executor.rs   — run_step() via Interactive Card + [MOLT_CALLBACK:<id>] poll
  commands/
    record/mark/stop/stats/run/list/recap.rs
```

## Key design decisions

**Feishu uses polling, not webhooks.** `molt` is a CLI with no public URL. The bot has a persistent WebSocket to Feishu. `molt` polls `GET /im/v1/messages` every 3 seconds, filtering by `create_time > send_time_ms`. Messages are double-decoded: the API returns `body.content` as a JSON string `{"text":"..."}`.

**Correlation IDs** tie async requests to responses. Format: `[MOLT_REQUEST:<8-char-uuid>]` for extraction, `[MOLT_CALLBACK:<8-char-uuid>]` for execution. Both are short UUIDs (`uuid[..8]`).

**DirectLlm routing**: if `base_url.contains("anthropic.com")` → uses `/v1/messages` with `x-api-key` header and 4096 max_tokens. Everything else (OpenAI, AutoClaw/GLM, Ollama) → `/v1/chat/completions` with Bearer auth.

**BackendConfig serde discriminant**: uses `#[serde(tag = "type", rename_all = "snake_case")]`, so `~/.molt/config.yaml` must have `type: feishu_bot` or `type: direct_llm` as the discriminant field.

**`PipelineStep.cmd` accepts `command:` alias** via `#[serde(alias = "command")]` for backward compatibility with older pipeline files.

**VirtualScreen handles alternate screen buffers.** vim/less/man switch to the alternate screen (`\x1b[?1049h`) and restore main on exit (`\x1b[?1049l`). VirtualScreen maintains two cell grids and swaps them on these private-mode sequences so the alt-screen content never bleeds into main-screen snapshots. In vte 0.13 the `?` byte is passed in `intermediates`, not params.

**AI extraction prefers VTE snapshots over raw ANSI.** `build_extraction_prompt()` uses `slice.screen_snapshot` when present (written to `molt_snapshots.jsonl` by the native PTY recorder at each MOLT_MARK). This gives the LLM clean readable text instead of escape-sequence noise.

## Runtime data layout

```
~/.molt/
  config.yaml          — MoltConfig (never in git)
  pipelines/*.yaml     — saved Pipeline structs
  history.jsonl        — RunRecord append-only log

/tmp/
  molt_session.cast      — active recording (.cast asciinema v2)
  molt_session.pid       — recorder process PID (NOT the shell's PID)
  molt_mark_count        — monotonic mark index counter
  molt_snapshots.jsonl   — per-mark VTE screen snapshots ({mark_index, timestamp, label, screen})
```

## Adding an AI backend

1. Create `src/backends/my_backend.rs`, implement `AiBackend` trait (`fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String>`)
2. Add variant to `BackendConfig` in `src/config.rs`
3. Add match arm in `build_backend()` in `src/backends/mod.rs`

## Adding a command

1. Create `src/commands/my_cmd.rs` with `pub fn run(...)`
2. Add `pub mod my_cmd;` to `src/commands/mod.rs`
3. Add variant to `Commands` enum and match arm in `src/main.rs`
