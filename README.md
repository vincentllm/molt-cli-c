# 🦞 Molt

**Terminal workflow recorder → AI pipeline extractor → ClawBot executor**

Record your terminal sessions, let AI extract reusable pipelines, replay them locally or delegate steps to **Feishu OpenClaw** — with authorization cards, callbacks, and team notifications.

Built in Rust. Designed for the **Feishu ClawBot Demo Competition**.

---

## How it works

```
molt record              # start asciinema recording
molt mark -l "deploy"   # drop semantic anchor at a key moment
molt stop               # stop → AI extracts YAML pipeline → save
molt stats              # timeline, segments, command histogram
molt run                # interactive picker → execute pipeline
molt recap              # usage analytics + OpenClaw lift report
```

```
┌──────────────────────────────────────────────────────────────────┐
│  Terminal              AI Backend           Feishu / OpenClaw    │
│                                                                  │
│  molt record ────────────────────────────────────────────────    │
│  [work]                                                          │
│  molt mark -l setup ──────────────────────────────────────────   │
│  [more work]                                                     │
│  molt stop ──────► DirectLLM  ──────────► pipeline.yaml         │
│               or ► ClawBot ─────────────► pipeline.yaml         │
│                                                                  │
│  molt run deploy ─────────────── step(local): sh -c "cmd"       │
│                   ─────────────── step(bot): Feishu Card ──────► │
│                                  [Execute] [Analyze] [Skip]      │
│                              ◄── [MOLT_CALLBACK:id] ────────────  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Quick start (WSL / Linux)

> **Windows users:** Run inside WSL2. The recording pipeline uses `/tmp` paths and `asciinema`, both Linux-native.

### 1. Install prerequisites

```bash
# Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# asciinema
pip install asciinema
# or: sudo apt install asciinema  /  brew install asciinema

# OpenSSL dev headers (needed by reqwest on some Linux distros)
# Ubuntu/Debian:
sudo apt install pkg-config libssl-dev
# Fedora/RHEL:
sudo dnf install pkgconfig openssl-devel
```

### 2. Clone and build

```bash
git clone https://github.com/vincentllm/molt-cli-c.git
cd molt-cli-c

cargo build --release          # ~30 s first build, faster after

# Add to PATH
echo 'export PATH="$HOME/molt-cli-c/target/release:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Or install system-wide
sudo cp target/release/molt /usr/local/bin/
```

### 3. Configure AI backend

Create `~/.molt/config.yaml` before first use (or let `molt stop` prompt you):

**Option A — OpenClaw / any OpenAI-compatible API (recommended):**
```yaml
backend:
  type: direct_llm
  api_key: "your-api-key"
  base_url: "https://your-openclaw-or-openai-endpoint"
  model: "glm-5.1"           # or gpt-4o, claude-opus-4-6, etc.
```

**Option B — Feishu ClawBot as AI extractor + executor:**
```yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90
```

See [docs/openclaw-setup.md](docs/openclaw-setup.md) and [docs/feishu-setup.md](docs/feishu-setup.md) for full setup guides.

### 4. First recording session

```bash
# Start recording
molt record

# Do your work — drop anchors at key moments
kubectl get pods
molt mark -l "check-pods"

docker build -t myapp:v1 .
molt mark -l "build"

docker push registry.example.com/myapp:v1
molt mark -l "push"

# Stop and extract pipeline
molt stop
# → AI reads your session and outputs structured YAML
# → You review and name the pipeline
# → Saved to ~/.molt/pipelines/my-pipeline.yaml

# View recording analytics
molt stats

# Run the pipeline later
molt run my-pipeline

# Or use intent matching
molt run -v "build and push docker image"
```

---

## Commands

### `molt record`

Start asciinema recording to `/tmp/molt_session.cast`.

```bash
molt record
```

Displays a recording banner. Use `molt mark` to drop anchors, then `molt stop` when done.

---

### `molt mark [-l label]`

Drop a semantic `MOLT_MARK` anchor into the recording stream. These anchors are used by the AI to segment your session into discrete steps.

```bash
molt mark                    # numbered anchor
molt mark -l "deploy"        # named anchor
molt mark -l "smoke-test"
```

---

### `molt stop`

Stop recording, run AI extraction, review and save the pipeline.

```bash
molt stop
```

Flow:
1. Sends SIGTERM to asciinema
2. Parses the `.cast` file into segments
3. Calls AI backend (spinner for DirectLLM, correlation ID protocol for ClawBot)
4. Displays extracted pipeline steps for review
5. Prompts for pipeline name and confirmation
6. Saves to `~/.molt/pipelines/<name>.yaml`
7. Sends Feishu notification (if feishu_bot backend configured)

---

### `molt stats [--file path]`

Visual analytics for the last recording (or a specified `.cast` file).

```bash
molt stats                           # last recording
molt stats --file /path/to/session.cast
```

Output:
- Duration, event counts, segment count
- ASCII timeline with MARK positions
- Segment table (start, duration, event count)
- Command frequency bar chart

---

### `molt run [NAME] [-v QUERY] [--yes] [--dry-run]`

Execute a saved pipeline. Three modes:

```bash
molt run                             # interactive fuzzy picker
molt run my-pipeline                 # exact name match
molt run -v "deploy to staging"      # intent matching (NL query)
molt run -v "deploy" --yes           # auto-run if confidence > 80%
molt run my-pipeline --dry-run       # preview steps, no execution
```

Each step is executed according to its `executor`:
- `local` → runs in the current shell
- `feishu_bot` → sends an Interactive Card to ClawBot, waits for `[MOLT_CALLBACK:id]`
- `ask` → prompts for confirmation before running

Run history is written to `~/.molt/history.jsonl` after every execution.

---

### `molt recap [--days N] [--pipeline NAME]`

Usage analytics and OpenClaw capability lift report.

```bash
molt recap                           # last 30 days
molt recap --days 7                  # last 7 days
molt recap --pipeline my-pipeline    # filter to one pipeline
```

Sections:
- **RUNS** — total, success rate, avg/day, total time
- **PIPELINES** — bar chart ranked by run count
- **ACTIVITY** — calendar view (last 14 active days)
- **OPENCLAW LIFT** — ClawBot delegations, time saved estimate, lift factor
- **INTENT MATCHING** — `-v` usage stats, avg confidence, auto-run rate
- **RELIABILITY** — failure list, flaky pipeline detection

---

## Configuration reference

`~/.molt/config.yaml` — never commit this file, it lives outside the repo.

### Direct LLM backend

```yaml
backend:
  type: direct_llm
  api_key: "sk-ant-..."                        # your API key
  base_url: "https://api.anthropic.com"        # Anthropic (default)
  model: "claude-opus-4-6"                     # model name
```

Any OpenAI-compatible endpoint works — change `base_url`:

```yaml
# OpenAI
base_url: "https://api.openai.com"
model: "gpt-4o"

# AutoClaw / OpenClaw proxy
base_url: "https://autoglm-api.zhipuai.cn/autoclaw-proxy/proxy/autoclaw"
model: "glm-5.1"
api_key: "your-autoclaw-key"

# Local Ollama
base_url: "http://localhost:11434"
model: "llama3"
api_key: "ollama"
```

### Feishu ClawBot backend

```yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90                        # optional, default 90
```

When using this backend, `molt stop` sends `[MOLT_REQUEST:<id>]` to the Feishu group and waits for a reply containing `[MOLT_RESPONSE:<id>]` with the pipeline YAML.

See [docs/feishu-setup.md](docs/feishu-setup.md) for how to create the Feishu app and get these values.  
See [docs/openclaw-setup.md](docs/openclaw-setup.md) for configuring OpenClaw as the ClawBot.

---

## Pipeline YAML format

Pipelines live in `~/.molt/pipelines/<name>.yaml`:

```yaml
name: deploy-k8s
description: Build, push, and deploy to Kubernetes
created_at: "2026-04-12T10:30:00Z"
steps:
  - name: Build Docker image
    mark: 1
    cmd: "docker build -t myapp:v1.0 ."
    description: Builds the application container
    executor: local

  - name: Push to registry
    mark: 2
    cmd: "docker push registry.example.com/myapp:v1.0"
    description: Push image — delegated to ClawBot
    executor: feishu_bot

  - name: Deploy to cluster
    mark: 3
    cmd: "kubectl apply -f k8s/deployment.yaml"
    description: Apply manifests — requires confirmation
    executor: ask
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Step display name |
| `cmd` | yes | Shell command to execute (also accepts `command:` alias) |
| `executor` | no | `local` (default) / `feishu_bot` / `ask` |
| `mark` | no | MOLT_MARK index this step corresponds to |
| `description` | no | Human-readable description |

---

## Project structure

```
molt-cli-c/
├── src/
│   ├── main.rs               # CLI entry (clap routing)
│   ├── config.rs             # MoltConfig + BackendConfig
│   ├── session.rs            # /tmp path constants
│   ├── history.rs            # RunRecord schema, history.jsonl read/write
│   ├── recording/
│   │   ├── cast.rs           # asciinema v2 .cast parser → MarkSlice
│   │   └── stats.rs          # CastStats, SegmentStats, command detection
│   ├── pipeline/
│   │   ├── schema.rs         # Pipeline + PipelineStep structs
│   │   └── store.rs          # YAML extract, parse, save to ~/.molt/pipelines/
│   ├── backends/
│   │   ├── mod.rs            # AiBackend trait + build_backend factory + prompt
│   │   ├── direct_llm.rs     # Anthropic Claude / OpenAI-compat HTTP call
│   │   └── feishu/
│   │       ├── client.rs     # HTTP layer: token, send text/card, poll messages
│   │       ├── extractor.rs  # AiBackend impl: MOLT_REQUEST/RESPONSE protocol
│   │       └── executor.rs   # run_step: Interactive Card + MOLT_CALLBACK poll
│   └── commands/
│       ├── record.rs         # molt record — asciinema subprocess
│       ├── mark.rs           # molt mark — MOLT_MARK anchor injection
│       ├── stop.rs           # molt stop — full AI extraction flow
│       ├── stats.rs          # molt stats — timeline + table rendering
│       ├── run.rs            # molt run — picker / intent / exact + history write
│       └── recap.rs          # molt recap — usage analytics rendering
├── docs/
│   ├── wsl-quickstart.md     # WSL setup and first run walkthrough
│   ├── feishu-setup.md       # Feishu app creation and permissions
│   ├── openclaw-setup.md     # OpenClaw / AutoClaw configuration
│   ├── architecture.md       # Technical design and extensibility notes
│   └── building.md           # Platform-specific build instructions
├── Cargo.toml
├── Cargo.lock
└── README.md
```

---

## Architecture summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Recording | `asciinema rec` subprocess | No custom PTY — battle-tested |
| Anchors | `MOLT_MARK N ts [label]` echoed to stdout | Captured by asciinema, no IPC |
| AI backend | `Box<dyn AiBackend>` trait object | Runtime selection, easy to extend |
| Feishu extraction | `[MOLT_REQUEST:id]` / `[MOLT_RESPONSE:id]` | No webhook URL needed (WebSocket) |
| Feishu execution | Interactive Card + `[MOLT_CALLBACK:id]` poll | ClawBot decides exec path |
| History | `~/.molt/history.jsonl` (JSONL append) | Simple, grep-friendly, no DB |
| Config | `~/.molt/config.yaml` (outside repo) | Secrets never in version control |

Full technical design: [docs/architecture.md](docs/architecture.md)

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `asciinema not found` | `pip install asciinema` or `sudo apt install asciinema` |
| `molt stop` asks for config every time | Config format mismatch — see Configuration section above |
| Feishu auth error 10003 | App not published — republish in Feishu developer console |
| Timeout waiting for ClawBot | Check OpenClaw is running; increase `poll_timeout_secs` |
| Pipeline fails to parse | Ensure `cmd:` field exists (or use `command:` alias) |
| `molt recap` shows no data | Run `molt run <pipeline>` first to generate history |

---

## License

MIT
