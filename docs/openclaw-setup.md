# OpenClaw Setup

OpenClaw (AutoClaw) is an AI agent platform built on Feishu. Molt integrates with it in two ways:

1. **As an AI extractor** — use the AutoClaw LLM proxy as the `direct_llm` backend for `molt stop`
2. **As a ClawBot executor** — OpenClaw listens in a Feishu group and responds to Molt's Interactive Cards during `molt run`

---

## Part 1: AutoClaw LLM proxy (for `molt stop`)

If you have access to the AutoClaw proxy endpoint, configure Molt to use it as the AI extractor:

```yaml
# ~/.molt/config.yaml
backend:
  type: direct_llm
  api_key: "your-autoclaw-api-key"
  base_url: "https://autoglm-api.zhipuai.cn/autoclaw-proxy/proxy/autoclaw"
  model: "glm-5.1"
```

Molt appends `/v1/chat/completions` to `base_url`, making the full request URL:
```
POST https://autoglm-api.zhipuai.cn/autoclaw-proxy/proxy/autoclaw/v1/chat/completions
```

### Getting your AutoClaw API key

1. Log in to the AutoClaw / OpenClaw platform
2. Go to **Settings → API Keys** (or equivalent)
3. Generate a new key and paste it into `api_key` above

> **Note:** The `base_url` should end **without** `/v1/chat/completions` — Molt adds that automatically.

---

## Part 2: OpenClaw as ClawBot executor (for `molt run`)

When a pipeline step has `executor: feishu_bot`, Molt sends an Interactive Card to a Feishu group and waits for OpenClaw to respond.

### How it works

```
molt run my-pipeline
   │
   ├─ step(executor: feishu_bot)
   │     ↓
   │  Feishu Interactive Card sent to group:
   │  ┌──────────────────────────────────────┐
   │  │  🦞 Molt 执行请求 [1/3]              │
   │  │  Pipeline: my-pipeline               │
   │  │  Step: deploy-to-prod               │
   │  │  Command: kubectl apply -f k8s/      │
   │  │  [✅ 立即执行] [🤔 分析再执行] [⏭ 跳过] │
   │  │  关联 ID: ab12cd34                   │
   │  └──────────────────────────────────────┘
   │     ↓
   │  OpenClaw receives card button event
   │  OpenClaw executes command / analyzes / skips
   │  OpenClaw replies in group:
   │  "[MOLT_CALLBACK:ab12cd34] result: <output>"
   │     ↓
   └─ Molt receives callback → continues to next step
```

### OpenClaw configuration

Your OpenClaw bot must be configured to:

1. **Listen in the Feishu group** where Molt sends cards
2. **Handle card button events** and reply with the callback format
3. **Use the correlation ID** from the card's button value

#### Card button value format

When the user clicks a button on Molt's card, OpenClaw receives:
```json
{
  "action": "execute",    // or "analyze" / "skip"
  "cmd": "kubectl apply -f k8s/deployment.yaml",
  "corr_id": "ab12cd34"
}
```

#### Required callback reply format

OpenClaw must send a **text message** in the group containing:
```
[MOLT_CALLBACK:ab12cd34] result: <output here>
```

For skip:
```
[MOLT_CALLBACK:ab12cd34] result: skipped
```

Molt polls the group every 3 seconds looking for this message (up to `poll_timeout_secs`, default 90 s).

### OpenClaw bot prompt suggestion

Configure your OpenClaw bot with a system prompt similar to:

```
You are a DevOps execution agent integrated with Molt CLI.

When you receive a card button click with action "execute":
1. Execute the `cmd` field in the appropriate environment
2. Capture stdout/stderr (truncated to ~500 chars)
3. Reply in the group: [MOLT_CALLBACK:<corr_id>] result: <output>

When action is "analyze":
1. Analyze the command and its likely effects
2. Suggest whether to proceed, modify, or skip
3. Execute if safe, then reply: [MOLT_CALLBACK:<corr_id>] result: <analysis + output>

When action is "skip":
1. Reply: [MOLT_CALLBACK:<corr_id>] result: skipped

Always include [MOLT_CALLBACK:<corr_id>] at the START of your reply.
```

---

## Part 3: Full combined config

Using AutoClaw for extraction AND Feishu ClawBot (OpenClaw) for execution:

```yaml
# ~/.molt/config.yaml — AI extraction via AutoClaw proxy
backend:
  type: direct_llm
  api_key: "your-autoclaw-api-key"
  base_url: "https://autoglm-api.zhipuai.cn/autoclaw-proxy/proxy/autoclaw"
  model: "glm-5.1"
```

```yaml
# ~/.molt/pipelines/my-pipeline.yaml — execution via ClawBot
name: my-pipeline
description: Deploy to production
steps:
  - name: Build
    cmd: docker build -t myapp:latest .
    executor: local           # runs locally

  - name: Deploy
    cmd: kubectl apply -f k8s/
    executor: feishu_bot      # delegated to OpenClaw
```

For the `feishu_bot` executor to work, you also need the Feishu group `chat_id` — but Molt reads it from the config only if `backend.type: feishu_bot`. If you're using `direct_llm` for extraction but still want `executor: feishu_bot` steps, you need to add the Feishu credentials separately.

> **Current limitation:** The `executor: feishu_bot` steps always read Feishu credentials from `backend` config. If you use `direct_llm` as the backend, ClawBot execution will be skipped with a "no Feishu backend configured" warning, and the step falls back to local execution. This will be improved in a future version with a separate `im:` config section.

**Workaround:** Use `feishu_bot` as the primary backend to get both extraction (via ClawBot's internal LLM) and execution delegation.

```yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90
```

See [feishu-setup.md](feishu-setup.md) for how to get `app_id`, `app_secret`, and `chat_id`.

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `[MOLT_CALLBACK]` timeout | OpenClaw is not running, or bot not in the group chat |
| No callback received | Check OpenClaw's reply includes `[MOLT_CALLBACK:<id>]` at the start |
| AutoClaw proxy 401 | API key is invalid or expired |
| AutoClaw proxy 404 | Check `base_url` — should not include `/v1/chat/completions` |
| Wrong pipeline YAML from extraction | Adjust the OpenClaw bot prompt to strictly follow the YAML format in `feishu-setup.md` |
