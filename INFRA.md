# tag1bot Infrastructure Documentation

## Overview

tag1bot is a long-running Slack bot written in Rust that provides utility features including karma tracking, user activity monitoring, currency conversion with price alerts, and ChatGPT integration. The bot operates via Slack Socket Mode and maintains state in a local SQLite database.

**Version:** 0.2.2
**Runtime:** Tokio 1.x (async)
**Language:** Rust 2021 Edition
**State:** SQLite 3 (./state.sqlite3)

## Architecture

### System Design

```
┌─────────────┐
│ Slack (Socket Mode) │
└────────┬────────┘
         │
         ▼
┌──────────────────────┐
│ tag1bot Process      │
├──────────────────────┤
│ Event Router (main.rs)
│  ├─ AppMention      │
│  └─ Message         │
│      ├─ Karma       │
│      ├─ Seen        │
│      ├─ Convert     │
│      └─ ChatGPT     │
└────┬─────────────────┘
     │
     ▼
┌──────────────────────┐
│ SQLite (state.sqlite3)│
├──────────────────────┤
│ karma                │
│ seen                 │
│ currency_alert       │
│ chatgpt_context      │
└──────────────────────┘
```

### External Dependencies

- **Slack API:** Socket Mode connection for real-time events
- **XE.com:** Currency conversion data (optional, gated on credentials)
- **QuickChart.io:** Chart rendering for price alerts (optional, requires XE credentials)
- **OpenAI:** ChatGPT API for conversation integration (optional, gated on API key)

### Codebase Structure

```
src/
├── main.rs (228 lines)
│   ├── Entry point and Tokio runtime
│   ├── Socket Mode connection handler
│   ├── Event dispatching to feature modules
│   └── Graceful shutdown handling
├── karma.rs (133 lines)
│   ├── Increment/decrement counters (word++/word--)
│   ├── Self-karma penalty enforcement
│   └── Database operations
├── seen.rs (166 lines)
│   ├── Last message tracking per user
│   ├── Channel context (public vs private)
│   └── Metadata persistence
├── convert.rs (485 lines)
│   ├── XE.com API integration
│   ├── Price alert creation and management
│   ├── Chart generation via QuickChart.io
│   └── Alert persistence and notification
├── chatgpt.rs (181 lines)
│   ├── OpenAI API client
│   ├── Conversation history per thread
│   ├── GPT-4 model interaction
│   └── Context management
├── claude3.rs (181 lines)
│   └── Placeholder for future Claude integration
├── slack.rs (228 lines)
│   ├── Slack API wrappers
│   ├── Message formatting helpers
│   └── API error handling
├── db.rs (81 lines)
│   ├── SQLite connection initialization
│   ├── Connection pool (Arc<Mutex<Connection>>)
│   └── Database schema creation
├── util.rs (223 lines)
│   ├── Timestamp utilities
│   ├── Time formatting (human-readable durations)
│   ├── Auth token validation
│   └── Helper functions
└── Cargo.toml
    ├── tokio 1.x
    ├── slack_sdk
    ├── sqlx (SQLite)
    ├── reqwest (HTTP client)
    └── serde (JSON/serialization)
```

### Event Processing Pipeline

All incoming events flow through a single handler with sequential processing:

```
Slack Event → on_events_api()
    │
    ├─ AppMention
    │   └─ Send random multilingual greeting
    │
    └─ Message
        ├─ karma::process_message() — Always runs
        ├─ seen::process_message() — Always runs
        ├─ convert::process_message() — If XE_ACCOUNT_ID/XE_API_KEY set
        └─ chatgpt::process_message() — If CHATGPT_API_KEY set
```

Processing is **sequential within each message**, no parallelization. Features are gated on environment variables.

## Command Reference

Users trigger features by typing the exact patterns below in Slack messages:

### Karma
- **Increment:** `word++` (e.g., `rust++`, `coffee++`)
- **Decrement:** `word--` (e.g., `javascript--`)
- **User karma:** `@user++` or `@user--` (e.g., `@alice++`)
- **Tag karma:** `#tag++` or `#tag--` (e.g., `#frontend++`)
- **Word length:** 2-20 characters (numbers and underscores allowed)
- **Self-karma:** Users cannot increment/decrement their own karma

### Seen
- **Query format:** `seen username?` or `seen username` (e.g., `seen alice`, `seen bob?`)
- **Response:** Last message, channel, and timestamp

### Currency Conversion
- **Convert currency:** `convert 100 USD to EUR` (e.g., `convert 50 BTC to USD`)
- **Price alerts:** `alert me when 1 BTC is greater than 50000 USD`
- **Alert syntax:** `alert me when [amount] [currency] is [greater/less] than [price] [target_currency]`
- **Requires:** XE_ACCOUNT_ID and XE_API_KEY environment variables

### ChatGPT
- **Query format:** `chatgpt <prompt>` (e.g., `chatgpt why is rust popular?`)
- **Conversation context:** Maintains thread history automatically
- **Requires:** CHATGPT_API_KEY environment variable

### App Mention
- **Trigger:** Mention the bot anywhere (`@tag1bot`)
- **Response:** Random multilingual greeting

## Initial Setup from Scratch

### Prerequisites

1. **Build environment:**
   - Rust 1.70+ (install via rustup)
   - Cargo
   - Linux/macOS/Windows with Tokio support
   - SQLite development libraries (libsqlite3-dev on Ubuntu)

2. **Slack workspace administration:**
   - Create a Slack app in your workspace (api.slack.com)
   - Generate app-level token (xapp-...) with connections:write scope
   - Generate bot user token (xoxb-...) with permissions listed below
   - Install bot to workspace

3. **Optional external accounts:**
   - XE.com developer account (for currency conversion)
   - OpenAI API key (for ChatGPT features)

### Step 1: Obtain Slack Tokens

1. Go to https://api.slack.com/apps
2. Create New App → From scratch → Name: "tag1bot" → Select workspace
3. Navigate to "Socket Mode" → Enable Socket Mode
4. Copy the generated App-Level Token (starts with xapp-...)
5. Navigate to "OAuth & Permissions" → Copy Bot User OAuth Token (starts with xoxb-...)
6. Under "Scopes" → Bot Token Scopes, add these permissions:
   - app_mentions:read
   - channels:history
   - channels:write
   - chat:write
   - groups:history
   - groups:read
   - im:history
   - im:read
   - mpim:history
   - users:read
   - users:write

### Step 2: Clone and Build

```bash
git clone <repo-url> tag1bot
cd tag1bot
cargo build --release
```

Build output: `./target/release/tag1bot`

### Step 3: Set Up Environment

Create `.env` or set environment variables:

**Required:**
```bash
export SLACK_APP_TOKEN="xapp-..."      # App-level token from Socket Mode
export SLACK_BOT_TOKEN="xoxb-..."      # Bot user token from OAuth
export SLACK_CHANNEL_ID="C01234..."    # Default channel ID (e.g., general)
```

**Optional (currency conversion):**
```bash
export XE_ACCOUNT_ID="your_xe_id"
export XE_API_KEY="your_xe_api_key"
```

**Optional (ChatGPT integration):**
```bash
export CHATGPT_API_KEY="sk-..."
```

**Optional (logging):**
```bash
export RUST_LOG="tag1bot=debug,warn"   # Default: "warn"
```

### Step 4: Initialize Database

The database is created automatically on first run at `./state.sqlite3` in the current working directory. **This means the location where state.sqlite3 is created depends on the directory from which the bot is launched.** If using systemd, the `WorkingDirectory` directive determines where the database will be stored, so ensure it points to your desired location (e.g., `/opt/tag1bot`). No manual initialization needed beyond running the bot once.

Schema is created automatically:

```sql
CREATE TABLE IF NOT EXISTS karma (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    counter INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_karma_name ON karma(name);

CREATE TABLE IF NOT EXISTS seen (
    id INTEGER PRIMARY KEY,
    user TEXT NOT NULL,
    channel TEXT NOT NULL,
    last_said TEXT,
    last_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_private INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_seen_user ON seen(user);

CREATE TABLE IF NOT EXISTS currency_alert (
    id INTEGER PRIMARY KEY,
    channel TEXT,
    user TEXT,
    from_currency TEXT,
    from_amount REAL,
    comparison TEXT,
    to_currency TEXT,
    to_amount REAL
);

CREATE TABLE IF NOT EXISTS chatgpt_context (
    id INTEGER PRIMARY KEY,
    thread TEXT NOT NULL UNIQUE,
    context TEXT
);
CREATE INDEX IF NOT EXISTS idx_context_thread ON chatgpt_context(thread);
```

### Step 5: First Run

```bash
./target/release/tag1bot
```

Expected output (with RUST_LOG=debug):
```
[INFO] Connecting to Slack...
[INFO] Socket Mode connected
[INFO] Listening for events...
```

Bot is now live. Invite it to channels and test:
- Type `@tag1bot` → should respond with greeting
- Type `test++` → increments karma
- Type `where was user seen` → checks last activity

## Configuration

### Environment Variables Reference

| Variable | Type | Required | Notes |
|----------|------|----------|-------|
| SLACK_APP_TOKEN | string | Yes | Socket Mode token (xapp-...) |
| SLACK_BOT_TOKEN | string | Yes | Bot user token (xoxb-...) |
| SLACK_CHANNEL_ID | string | Yes | Default channel (e.g., C123456) |
| XE_ACCOUNT_ID | string | No | XE.com account ID for currency |
| XE_API_KEY | string | No | XE.com API key for currency |
| CHATGPT_API_KEY | string | No | OpenAI API key (sk-...) |
| RUST_LOG | string | No | Log level: error, warn (default), info, debug |

### Feature Gating

Features activate only when their environment variables are present:

- **Currency conversion:** Requires both XE_ACCOUNT_ID and XE_API_KEY
- **ChatGPT integration:** Requires CHATGPT_API_KEY
- **Karma and Seen:** Always enabled
- **App mentions:** Always enabled

### Database Location

Default: `./state.sqlite3` in the working directory where the bot is launched.

To use alternate location, modify `db.rs` and rebuild (or use symlink).

## Operations

### Starting the Bot

```bash
export SLACK_APP_TOKEN="xapp-..."
export SLACK_BOT_TOKEN="xoxb-..."
export SLACK_CHANNEL_ID="C123456"
./target/release/tag1bot
```

Or with systemd (see example below).

### Systemd Service (Linux)

Create `/etc/systemd/system/tag1bot.service`:

```ini
[Unit]
Description=tag1bot Slack Bot
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=tag1bot
WorkingDirectory=/opt/tag1bot
Environment="SLACK_APP_TOKEN=xapp-..."
Environment="SLACK_BOT_TOKEN=xoxb-..."
Environment="SLACK_CHANNEL_ID=C123456"
Environment="RUST_LOG=warn"
ExecStart=/opt/tag1bot/target/release/tag1bot
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

**IMPORTANT:** The `WorkingDirectory` directive is critical. The bot MUST be run from the repository root (or the directory containing the `prompts/` folder) because the ChatGPT feature references `prompts/translate.md` using a relative path. The WorkingDirectory in the example points to `/opt/tag1bot`, which assumes the bot source code (including the `prompts/` directory) is located there. Adjust accordingly if your installation directory differs.

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable tag1bot
sudo systemctl start tag1bot
sudo systemctl status tag1bot
```

View logs:

```bash
journalctl -u tag1bot -f
```

### Restarting the Bot

**Graceful restart:**

```bash
systemctl restart tag1bot
```

The bot closes its Slack connection and re-establishes it on restart. Current conversation contexts (chatgpt_context table) are preserved.

**Quick restart (systemd):**

```bash
systemctl stop tag1bot
sleep 2
systemctl start tag1bot
```

Wait 5-10 seconds for Socket Mode reconnection.

### Database Backup

Backup the state file regularly:

```bash
# Simple copy
cp /opt/tag1bot/state.sqlite3 /backups/tag1bot-state-$(date +%Y%m%d-%H%M%S).sqlite3

# Or use sqlite3 backup command
sqlite3 /opt/tag1bot/state.sqlite3 ".backup /backups/tag1bot-state-$(date +%Y%m%d-%H%M%S).sqlite3"
```

**Backup frequency:** At least daily. Consider more frequent backups if karma/alert data is critical.

**Retention:** Keep at least 7 days of backups.

### Database Maintenance

Check database integrity:

```bash
sqlite3 state.sqlite3 "PRAGMA integrity_check;"
```

Should return "ok".

Vacuum (optimize storage):

```bash
sqlite3 state.sqlite3 "VACUUM;"
```

Monitor size:

```bash
ls -lh state.sqlite3
# Should be <10MB under normal use
```

### Monitoring

Monitor via systemd:

```bash
# Current status
systemctl status tag1bot

# Recent logs
journalctl -u tag1bot -n 50

# Follow logs in real-time
journalctl -u tag1bot -f
```

Monitor in Slack:

1. Add a simple health check command by having the bot respond to `@tag1bot health`
2. Periodically test from a monitoring bot or cron job
3. Alert if bot stops responding within 60 seconds

**Key metrics to track:**
- Process uptime (systemd Restart behavior)
- Database size growth (should be linear with usage)
- Slack connection status (check logs for reconnects)
- API error rates (search logs for "error" or "failed")

### Database Queries for Operations

**View top karma scores:**

```sql
SELECT name, counter FROM karma ORDER BY counter DESC LIMIT 10;
```

**Check recent activity (seen):**

```sql
SELECT user, channel, last_seen FROM seen ORDER BY last_seen DESC LIMIT 20;
```

**List active currency alerts:**

```sql
SELECT user, channel, from_currency, from_amount, comparison, to_currency, to_amount
FROM currency_alert;
```

**View ChatGPT conversation threads:**

```sql
SELECT thread, context FROM chatgpt_context;
```

**Delete old ChatGPT contexts (if storage is an issue):**

```sql
DELETE FROM chatgpt_context WHERE rowid NOT IN
  (SELECT rowid FROM chatgpt_context ORDER BY rowid DESC LIMIT 100);
```

## Troubleshooting

### Bot Not Responding

**Check 1: Is the process running?**

```bash
systemctl status tag1bot
ps aux | grep tag1bot
```

If not running, check logs:

```bash
journalctl -u tag1bot -n 100
```

**Check 2: Are Slack tokens valid?**

Try re-generating tokens in Slack API console. Tokens expire or may be revoked. Verify in logs:

```bash
journalctl -u tag1bot -n 20 | grep -i "error\|failed\|unauthorized"
```

**Check 3: Is Socket Mode connected?**

Look for "Socket Mode connected" in logs. If absent, check:
- App-level token (xapp-...) is set correctly
- "Socket Mode" is enabled in Slack app settings
- Workspace hasn't revoked permissions

**Check 4: Database lock issues**

If you see "database is locked" errors, check:

```bash
# Verify database is accessible
sqlite3 state.sqlite3 "SELECT COUNT(*) FROM karma;"
```

If locked, restart the bot:

```bash
systemctl restart tag1bot
```

### Feature Not Working

**Currency conversion returns no results:**

1. Verify XE_ACCOUNT_ID and XE_API_KEY are set
2. Check XE API status: `curl -v https://xcdapi.xe.com/v1/currencies/ -H "Authorization: Bearer XE_API_KEY"`
3. Ensure message format matches expected pattern (e.g., "100 USD to EUR")
4. Check logs: `journalctl -u tag1bot | grep -i "convert\|currency"`

**ChatGPT not responding:**

1. Verify CHATGPT_API_KEY is set and valid
2. Check OpenAI API status: https://status.openai.com/
3. Ensure thread context is being saved (check chatgpt_context table)
4. Look for quota/rate limit errors in logs

**Karma or Seen not tracking:**

These features always run. If not working:

1. Check database is writable: `touch state.sqlite3` (should succeed)
2. Verify tables exist: `sqlite3 state.sqlite3 ".tables"`
3. Restart bot: `systemctl restart tag1bot`

### Performance Issues

**High memory usage:**

- ChatGPT context table may have grown large. Clean old threads:

```sql
DELETE FROM chatgpt_context WHERE rowid NOT IN
  (SELECT rowid FROM chatgpt_context ORDER BY rowid DESC LIMIT 100);
```

- Restart bot: `systemctl restart tag1bot`

**Slow message processing:**

- Check database file size: `ls -lh state.sqlite3`
- If >50MB, run VACUUM: `sqlite3 state.sqlite3 "VACUUM;"`
- Check XE API response times (may be hitting rate limits)
- Check ChatGPT API latency

**Connection dropping frequently:**

Check logs for reconnection patterns:

```bash
journalctl -u tag1bot | grep -i "disconnect\|reconnect\|error"
```

If Slack-side issue:
- Verify app has stable network
- Check firewall/proxy isn't blocking Socket Mode (port 443)

### Database Corruption

If PRAGMA integrity_check shows errors:

```bash
# Stop bot
systemctl stop tag1bot

# Restore from backup
cp /backups/tag1bot-state-YYYYMMDD-HHMMSS.sqlite3 state.sqlite3

# Start bot
systemctl start tag1bot
```

If no backup available, the bot will recreate the database schema on next start (but all state data is lost).

## Security Notes

### Credential Management

**DO NOT:**
- Commit tokens to Git
- Log tokens (they won't be, but check RUST_LOG settings)
- Share .env files
- Use the same token across multiple environments

**DO:**
- Store tokens in environment variables or secrets manager
- Rotate tokens regularly (at least quarterly)
- Use systemd EnvironmentFile or container secrets for production
- Restrict file permissions on .env: `chmod 600 .env`

### Token Rotation

To rotate Slack tokens without downtime:

1. Generate new App-Level Token in Slack API console (Socket Mode)
2. Copy new token
3. Update SLACK_APP_TOKEN environment variable
4. Restart bot: `systemctl restart tag1bot`
5. Delete old token from Slack console (after confirming new token works)

Same process for SLACK_BOT_TOKEN.

### External API Keys

- **XE.com credentials:** Store securely. If compromised, request new credentials from XE.com
- **ChatGPT API key:** Revoke and regenerate from OpenAI console if exposed
- **Slack tokens:** Revoke immediately from Slack console if exposed

### Database Permissions

Ensure state.sqlite3 has restricted permissions:

```bash
chmod 600 /opt/tag1bot/state.sqlite3
chown tag1bot:tag1bot /opt/tag1bot/state.sqlite3
```

### Network Security

- Bot requires outbound HTTPS (443) to:
  - api.slack.com (Socket Mode)
  - xcdapi.xe.com (if currency enabled)
  - api.openai.com (if ChatGPT enabled)
  - quickchart.io (if currency alerts enabled)
- No inbound ports required (Socket Mode is outbound-only)

### Known Issues and Limitations

From the codebase review (FIX.md):

1. **Silent API failures:** Currency conversion and ChatGPT may fail silently without user feedback. Monitor logs for errors.

2. **Relative path issue:** The translate feature references `prompts/translate.md` with a relative path, which may fail if bot is not run from the repo root. Work around by running bot from its installation directory.

3. **Regex mismatch:** Documentation vs code may have inconsistencies in feature triggers. Test features after deployment.

4. **No error feedback to users:** API failures don't generate Slack messages to users. Users won't know why features didn't work.

## Deployment Checklist

- [ ] Rust 1.70+ installed
- [ ] Slack app created and app-level + bot tokens generated
- [ ] Bot permissions scopes added and app installed to workspace
- [ ] Code cloned and `cargo build --release` successful
- [ ] Environment variables set (required: SLACK_APP_TOKEN, SLACK_BOT_TOKEN, SLACK_CHANNEL_ID)
- [ ] Database initialized on first run
- [ ] Systemd service file created (if using systemd)
- [ ] Backup procedure documented
- [ ] Monitoring in place (logs, process health)
- [ ] Bot tested in Slack (greeting, karma, seen features)
- [ ] Optional features enabled if credentials provided
- [ ] Credentials stored securely (not in git, systemd secrets manager)
- [ ] Production restart/recovery procedure tested

## Support and Monitoring Contacts

Document your team's contacts and escalation procedures:

- **Bot admin:** [Name/Team]
- **Slack workspace admin:** [Name/Team]
- **Database/Storage admin:** [Name/Team]
- **On-call escalation:** [Process]

---

**Last Updated:** 2025-02-06
**Version:** 1.0
**Tag1bot Version:** 0.2.2
