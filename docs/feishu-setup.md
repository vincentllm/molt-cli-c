# Feishu ClawBot Setup

This guide walks through creating a Feishu custom app and obtaining the credentials needed for Molt's `feishu_bot` backend.

## 1. Create a Feishu app

1. Go to [Feishu Open Platform](https://open.feishu.cn/) → **Developer Console**
2. Click **Create App** → Custom App
3. Fill in app name (e.g. "Molt Pipeline Bot") and description
4. Note your **App ID** (`cli_xxx...`) and **App Secret**

## 2. Configure app permissions

In the app console, go to **Permissions & Scopes** and enable:

| Permission | Scope | Required for |
|-----------|-------|-------------|
| `im:message` | Read/Write | Send & receive messages |
| `im:message:send_as_bot` | Write | Bot sends messages |
| `im:chat` | Read | List chat messages |

Click **Save** after adding permissions.

## 3. Enable event subscription (for ClawBot)

If you're running an OpenClaw-based ClawBot:

1. Go to **Event Subscriptions**
2. Set subscription method to **Persistent Connection** (WebSocket — no public URL needed)
3. Subscribe to event: `im.message.receive_v1`
4. Save and **republish the app**

## 4. Add bot to a group chat

1. Open Feishu → Create or open a group chat
2. Invite the bot: type `@` → search for your app name → add
3. Get the `chat_id`:
   - In Feishu Web: the URL contains the chat ID (starts with `oc_`)
   - Or: use the API — `GET https://open.feishu.cn/open-apis/im/v1/chats` after getting a token

## 5. Get tenant_access_token (for testing)

```bash
curl -X POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal \
  -H "Content-Type: application/json" \
  -d '{"app_id":"cli_xxx","app_secret":"xxx"}'
```

Response: `{"code":0,"tenant_access_token":"t-xxx","expire":7200}`

## 6. Configure Molt

```yaml
# ~/.molt/config.yaml
backend:
  type: feishu_bot
  app_id: "cli_xxxxxxxxxxxxxxxx"
  app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  chat_id: "oc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  poll_timeout_secs: 90
```

## ClawBot response protocol

Molt uses a correlation ID pattern. Your ClawBot must:

**For pipeline extraction (`molt stop`):**
- Receive a message containing `[MOLT_REQUEST:<8-char-id>]`
- Process it through LLM
- Reply with the YAML result **followed by** `[MOLT_RESPONSE:<same-id>]`

Example reply:
```
```yaml
name: deploy-k8s
steps:
  - name: Build image
    cmd: docker build .
    executor: local
```
[MOLT_RESPONSE:ab12cd34]
```

**For pipeline execution (`molt run` with `executor: feishu_bot`):**
- Receive an Interactive Card with button values containing `corr_id`
- User clicks Execute/Analyze/Skip
- ClawBot processes and replies: `[MOLT_CALLBACK:<corr_id>] result: <output>`

Example callback reply:
```
[MOLT_CALLBACK:ab12cd34] result: Successfully pushed image to registry
```

## Troubleshooting

| Issue | Fix |
|-------|-----|
| `Auth error code 10003` | App not published — republish in developer console |
| `Send message error code 99991663` | Bot not added to the group chat |
| Timeout waiting for response | Check ClawBot is running; increase `poll_timeout_secs` |
| Old messages matched | `start_time` filter uses seconds; check system clock sync |
