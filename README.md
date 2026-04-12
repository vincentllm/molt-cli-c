# 🦞 Molt

**Terminal workflow recorder → AI pipeline extractor → ClawBot executor**

Molt records your terminal sessions, uses AI to extract structured reusable pipelines from them, and executes those pipelines via Feishu ClawBot — enabling delegation, authorization, and async execution across your team.

Built in Rust. Designed for the **Feishu ClawBot Demo Competition**.

---

## How it works

```
molt record           # start asciinema recording
molt mark -l "setup"  # drop semantic anchor (repeatable)
molt stop             # stop → AI extracts pipeline YAML → save
molt stats            # view timeline, segments, detected commands
molt run deploy       # execute: local / ClawBot card / human confirm
```

**Full flow:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Terminal                     AI Backend         Feishu ClawBot  │
│                                                                  │
│  molt record ──────────────────────────────────────────────────  │
│  [work work work]                                                │
│  molt mark -l setup ──────────────────────────────────────────   │
│  [more work]                                                     │
│  molt stop ──────► DirectLLM / ClawBot ──► pipeline.yaml        │
│                                                      │           │
│  molt stats ◄──── visual timeline + commands         │           │
│                                                      │           │
│  molt run deploy ────────────────────────────────────┤           │
│     step(local)  ──► sh -c "cmd"                     │           │
│     step(bot)    ──► Feishu Interactive Card ────────►           │
│                          [Execute] [Analyze] [Skip]  │           │
│                      ◄──── [MOLT_CALLBACK:id] ───────┘           │
└─────────────────────────────────────────────────────────────────┘
```

---

## Quick start

### Prerequisites

- Rust stable (≥ 1.75)
- [asciinema](https://asciinema.org/) — `pip install asciinema` or `brew install asciinema`
- (Optional) Feishu app credentials for ClawBot integration

### Install

```bash
git clone git@github.com:vincentllm/molt-cli-c.git
cd molt-cli-c

# Linux / macOS
cargo build --release
sudo cp target/release/molt /usr/local/bin/

# Windows (GNU toolchain) — see docs/building.md
rustup run stable-x86_64-pc-windows-gnu cargo build --release
```

### First run

```bash
# 1. Start recording
molt record

# 2. Do your work. Drop anchors at key moments:
kubectl get pods
molt mark -l "check-pods"

docker build -t myapp:v1 .
molt mark -l "build"

# 3. Stop and extract
molt stop
# → prompts for AI backend (API key or Feishu ClawBot)
# → saves to ~/.molt/pipelines/my-pipeline.yaml

# 4. View stats
molt stats

# 5. Re-run the pipeline
molt run my-pipeline
```

---

## Commands

| Command | Description |
|---------|-------------|
| `molt record` | Start recording via asciinema |
| `molt mark [-l label]` | Drop a `MOLT_MARK` semantic anchor |
| `molt stop` | Stop recording, AI-extract pipeline, save YAML |
| `molt stats [--file path]` | Visual analytics: timeline, segments, commands |
| `molt run <name>` | Execute a saved pipeline (local / ClawBot) |

---

## Configuration

On first `molt stop`, you are prompted to choose a backend and create `~/.molt/config.yaml`.

**`~/.molt/config.yaml`** (never committed — lives outside the repo)

### Option A: Direct LLM (Anthropic Claude)

```yaml
backend:
  type: direct_llm
  api_key: "sk-ant-..."         # from console.anthropic.com
  model: "claude-opus-4-6"      # optional, this is the default
  base_url: "https://api.anthropic.com"  # optional
```

Supports any OpenAI-compatible endpoint by changing `base_url`.

### Option B: Feishu ClawBot (recommended for demo)

```yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90
```

See [docs/feishu-setup.md](docs/feishu-setup.md) for how to obtain these values.

### Pipeline executor field

Each pipeline step has an `executor` field controlling how `molt run` executes it:

| Value | Behavior |
|-------|----------|
| `local` | Run directly in the local shell (default) |
| `feishu_bot` | Send an interactive Feishu card to ClawBot, wait for `[MOLT_CALLBACK:id]` |
| `ask` | Prompt for confirmation before running locally |

---

## Pipeline YAML format

Pipelines are saved to `~/.molt/pipelines/<name>.yaml`:

```yaml
name: deploy-k8s
description: Deploy application to Kubernetes cluster
created_at: "2026-04-12T10:30:00Z"
steps:
  - name: Build Docker image
    mark: 1
    cmd: "docker build -t myapp:v1.0 ."
    description: Builds the application container image
    executor: local

  - name: Push to registry
    mark: 2
    cmd: "docker push registry.example.com/myapp:v1.0"
    description: Push image to container registry
    executor: feishu_bot   # delegated to ClawBot

  - name: Deploy to cluster
    mark: 3
    cmd: "kubectl apply -f k8s/deployment.yaml"
    description: Apply Kubernetes manifests
    executor: ask          # requires human confirmation
```

---

## Project structure

```
molt-cli-c/
├── src/
│   ├── main.rs              # CLI entry point (clap routing)
│   ├── config.rs            # MoltConfig + BackendConfig (Feishu / LLM)
│   ├── cast_parser.rs       # asciinema v2 .cast parser + stats extractor
│   ├── pipeline.rs          # Pipeline YAML schema + file management
│   ├── ai/
│   │   ├── mod.rs           # AiBackend trait + factory + prompt builder
│   │   ├── direct_llm.rs    # Anthropic Claude / OpenAI-compat backend
│   │   └── feishu_bot.rs    # Feishu ClawBot: auth, cards, polling
│   └── commands/
│       ├── mod.rs
│       ├── record.rs        # molt record — asciinema subprocess
│       ├── mark.rs          # molt mark — MOLT_MARK anchor injection
│       ├── stop.rs          # molt stop — full AI extraction flow
│       ├── stats.rs         # molt stats — visual timeline + table
│       └── run_pipeline.rs  # molt run — local / ClawBot execution
├── docs/
│   ├── architecture.md      # Technical design & extensibility notes
│   ├── feishu-setup.md      # Feishu app configuration guide
│   └── building.md          # Platform-specific build instructions
├── .cargo/
│   └── config.toml          # GNU linker config (Windows only, see comments)
├── .github/
│   └── ISSUE_TEMPLATE.md
├── Cargo.toml
├── Cargo.lock               # pinned for reproducible builds
└── README.md
```

### Extensibility points

- **New AI backend**: implement `AiBackend` trait in `src/ai/`, add variant to `BackendConfig`
- **New command**: add module to `src/commands/`, wire in `main.rs`
- **New executor**: add branch in `run_pipeline.rs` `run_via_*` functions
- **Pipeline format**: extend `PipelineStep` in `pipeline.rs` — YAML is forward-compatible

---

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full technical design.

**Key design decisions:**

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Recording backend | `asciinema rec` subprocess | Avoids custom PTY — battle-tested |
| Semantic anchors | `MOLT_MARK N ts [label]` to stdout | Captured by asciinema, no IPC needed |
| AI extraction | Trait object `Box<dyn AiBackend>` | Runtime backend selection |
| Feishu polling | Correlation ID `[MOLT_REQUEST:id]` / `[MOLT_RESPONSE:id]` | No public webhook URL needed |
| Execution delegation | Interactive Card + `[MOLT_CALLBACK:id]` poll | ClawBot decides exec path; molt just waits |
| Config location | `~/.molt/config.yaml` (outside repo) | Secrets never enter version control |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

MIT
