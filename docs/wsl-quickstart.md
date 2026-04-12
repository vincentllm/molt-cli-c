# WSL Quickstart

Get Molt running inside WSL2 in under 5 minutes.

## Prerequisites

| Tool | Install |
|------|---------|
| WSL2 | `wsl --install` in PowerShell (Windows 10/11) |
| Git | pre-installed on most distros; or `sudo apt install git` |
| Rust | see below |
| asciinema | see below |
| OpenSSL dev | `sudo apt install pkg-config libssl-dev` |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Follow prompts, then:
source ~/.cargo/env
```

### Install asciinema

```bash
pip install asciinema
# or:
sudo apt install asciinema
```

---

## Clone and build

```bash
# Clone via HTTPS (no SSH key needed)
git clone https://github.com/vincentllm/molt-cli-c.git
cd molt-cli-c

# Build (first build ~30 s, subsequent builds ~2 s)
cargo build --release
```

### Add `molt` to your PATH

**Option A — per-session (quick test):**
```bash
export PATH="$PWD/target/release:$PATH"
```

**Option B — permanent:**
```bash
echo "export PATH=\"$HOME/molt-cli-c/target/release:\$PATH\"" >> ~/.bashrc
source ~/.bashrc
```

**Option C — system-wide:**
```bash
sudo cp target/release/molt /usr/local/bin/
```

Verify:
```bash
molt --help
```

---

## Configure AI backend

Create `~/.molt/config.yaml`. Choose one option:

### Option A: OpenClaw / AutoClaw proxy (if you have access)

```yaml
backend:
  type: direct_llm
  api_key: "your-autoclaw-api-key"
  base_url: "https://autoglm-api.zhipuai.cn/autoclaw-proxy/proxy/autoclaw"
  model: "glm-5.1"
```

See [openclaw-setup.md](openclaw-setup.md) for getting your API key.

### Option B: Feishu ClawBot (bot handles AI + execution)

```yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90
```

See [feishu-setup.md](feishu-setup.md) for getting these values.

### Option C: Anthropic Claude directly

```yaml
backend:
  type: direct_llm
  api_key: "sk-ant-..."
  base_url: "https://api.anthropic.com"
  model: "claude-opus-4-6"
```

---

## First session walkthrough

```bash
# 1. Start recording
molt record

# 2. Do some work — the recording captures everything
echo "hello world"
ls -la

# 3. Drop a semantic anchor
molt mark -l "setup"

# 4. More work
mkdir -p /tmp/myproject && cd /tmp/myproject
echo "print('hello')" > app.py
python3 app.py

# 5. Another anchor
molt mark -l "run-app"

# 6. Stop and extract pipeline
molt stop
#   → Shows segment summary
#   → Calls AI (spinner appears)
#   → Displays extracted pipeline steps
#   → Prompts: "Pipeline name:" → type a name → Enter
#   → Prompts: "Save to ~/.molt/pipelines/?" → Y
#   → Saved!

# 7. View recording analytics
molt stats

# 8. See the saved pipeline
cat ~/.molt/pipelines/<your-name>.yaml

# 9. Run it again (interactive picker)
molt run

# 10. Or run with intent matching
molt run -v "setup and run the app"

# 11. After a few runs, view the analytics
molt recap
```

---

## SSH key setup (optional, for git push)

If you want to push changes to GitHub from WSL:

```bash
# Generate key
ssh-keygen -t ed25519 -C "your-email@example.com"

# Copy public key to clipboard
cat ~/.ssh/id_ed25519.pub
# → paste into GitHub → Settings → SSH Keys → New SSH key

# Test
ssh -T git@github.com
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `cargo: command not found` | Run `source ~/.cargo/env` or restart terminal |
| `pkg-config not found` during build | `sudo apt install pkg-config libssl-dev` |
| `asciinema: command not found` | `pip install asciinema` |
| `molt stop` says "no recording in progress" | Make sure `molt record` is running in the **same** terminal |
| Feishu API timeout | Check WSL has internet access: `curl https://open.feishu.cn` |
| `~/.molt/config.yaml` format error | See Configuration section in README for exact YAML structure |
