# Molt — Architecture

## Overview

Molt is a three-phase tool:

1. **Record** — capture terminal sessions as asciinema `.cast` files with semantic markers
2. **Extract** — parse markers, send slices to AI, get structured pipeline YAML
3. **Execute** — replay pipelines locally or via Feishu ClawBot

---

## Module dependency graph

```
main.rs
  ├── commands/record.rs        uses: record.rs helpers
  ├── commands/mark.rs          uses: record.rs (read/write mark count)
  ├── commands/stop.rs          uses: cast_parser, ai/, pipeline, config
  │     ├── cast_parser.rs      pure parser, no external deps
  │     ├── ai/mod.rs           AiBackend trait + build_extraction_prompt
  │     │   ├── direct_llm.rs   reqwest blocking → Anthropic / OpenAI-compat
  │     │   └── feishu_bot.rs   reqwest blocking → Feishu Open Platform API
  │     ├── pipeline.rs         serde_yaml → ~/.molt/pipelines/
  │     └── config.rs           serde_yaml → ~/.molt/config.yaml
  ├── commands/stats.rs         uses: cast_parser (full stats), comfy-table
  └── commands/run_pipeline.rs  uses: pipeline, config, ai/feishu_bot
```

---

## Recording layer

### asciinema v2 `.cast` format

```
{"version":2,"width":220,"height":50,...}   ← header (line 1)
[0.123, "o", "$ kubectl get pods\r\n"]      ← output event
[0.456, "i", "k"]                           ← input event
[1.234, "o", "MOLT_MARK 1 2026-04-12T... setup\r\n"]  ← anchor
```

### MOLT_MARK protocol

`MOLT_MARK <index> <ISO8601-timestamp> [label]`

Injected via `echo` to stdout while asciinema is recording — no IPC, no PTY hacks. The anchor appears in the `.cast` output stream and is detected during parsing.

```
mark.rs: println!("MOLT_MARK {} {} {}", index, timestamp, label)
           ↓ (asciinema captures stdout)
cast_parser.rs: regex match → segment boundary
```

---

## AI extraction layer

### AiBackend trait

```rust
pub trait AiBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String>;
}
```

### Prompt structure

```
[system context]
Per-slice sections:
  --- MARK N ---
  label: <label>
  <ANSI-stripped terminal content, ≤2000 chars>

[format instruction: return YAML only, inside ```yaml block]
```

### Feishu ClawBot transport

```
molt → POST /im/v1/messages (text)
     body: "[MOLT_REQUEST:ab12cd34]\n<prompt>\n请回复 [MOLT_RESPONSE:ab12cd34]"

ClawBot (WebSocket long-connection via Feishu SDK)
     receives message → internal LLM → replies:
     "[MOLT_RESPONSE:ab12cd34]\n```yaml\n...\n```"

molt → GET /im/v1/messages?start_time=<send_time_sec>
     polls every 3s, filters create_time > send_time_ms
     body.content is JSON-string: {"text":"..."} — parsed twice
```

**Why polling, not webhook:** ClawBot already has the WebSocket persistent connection to Feishu. `molt` is a CLI with no public URL. Polling with a correlation ID is stateless and requires no server.

---

## Execution layer

### Pipeline step routing (`molt run`)

```
for each step:
  executor == "local"      → sh -c cmd (subprocess)
  executor == "feishu_bot" → send Interactive Card → poll [MOLT_CALLBACK:id]
  executor == "ask"        → inquire::Confirm → if yes, local exec
```

### Feishu Interactive Card for execution

```json
{
  "config": {"wide_screen_mode": true},
  "header": {"title": {"content": "🦞 Molt 执行请求 [2/3]", "tag": "plain_text"}, "template": "blue"},
  "elements": [
    {"tag": "div", "fields": [{"is_short": true, "text": {"tag": "lark_md", "content": "**Pipeline**\ndeploy-k8s"}}]},
    {"tag": "div", "text": {"tag": "lark_md", "content": "**命令**\n```\ndocker push ...\n```"}},
    {"tag": "action", "actions": [
      {"tag": "button", "type": "primary",  "text": {"content": "✅ 立即执行"}, "value": {"action":"execute","corr_id":"ab12cd34"}},
      {"tag": "button", "type": "default",  "text": {"content": "🤔 分析再执行"}, "value": {"action":"analyze","corr_id":"ab12cd34"}},
      {"tag": "button", "type": "danger",   "text": {"content": "⏭ 跳过"}, "value": {"action":"skip","corr_id":"ab12cd34"}}
    ]},
    {"tag": "note", "elements": [{"tag": "plain_text", "content": "关联 ID: ab12cd34"}]}
  ]
}
```

ClawBot receives button click via WebSocket event → executes → replies:
```
[MOLT_CALLBACK:ab12cd34] result: <stdout>
```

---

## Configuration & secrets model

```
~/.molt/
  config.yaml      ← credentials (NEVER in git, outside repo by design)
  pipelines/
    deploy-k8s.yaml
    setup-dev.yaml
```

The `config.yaml` is never inside the project directory. There is no `.env` file. No secret ever touches version control.

---

## Extending Molt

### Add a new AI backend

1. Create `src/ai/my_backend.rs`, implement `AiBackend` trait
2. Add `MyBackend { ... }` variant to `BackendConfig` in `src/config.rs`
3. Add match arm in `ai::build_backend()` in `src/ai/mod.rs`

### Add a new command

1. Create `src/commands/my_cmd.rs` with `pub fn run(...)`
2. Add `pub mod my_cmd;` to `src/commands/mod.rs`
3. Add variant to `Commands` enum in `src/main.rs`
4. Add match arm in `main()`

### Add a new pipeline executor

1. Add a new executor value to `PipelineStep.executor` field (string, forward-compatible)
2. Add match arm in `run_pipeline.rs`'s per-step dispatch
