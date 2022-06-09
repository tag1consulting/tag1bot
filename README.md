# Tag1bot

Porting parts of https://www.drupal.org/project/bot into a Rust-powered slackbot.

Why "Tag1bot"? Because we created this for Tag1's internal Slack server, but then released it as open source feeling it's likely other people also missed the wonderful Drupal irc bot!

## Karma

The bot increases karma for `foo++`-style commands, and decreases karma for `foo--`-style commands. Karma is the total number of times a given word has been incremented or decremented. Words must be 2 to 20 characters long, without any spaces.

## Seen

The bot records the last message per user posted to any public channel it is in, and responds to `seen foo?` with the details.

## Convert

The bot recognizes "convert # FOO to BAR" style requests. For example, `convert 1 BTC to USD` or `convert 100 USD to EUR`.

The bot also recognizes "alert me when # FOO is [greater|less] # bar" style requests. For example `alert me when 1 USD is greater than .95 EUR`, or `alert when BTC is less than 20000 USD`. Alerts will be delivered in the channel the alert was configured in.

The convert features require that you set up an account on https://www.xe.com/xecurrencydata/ and configure the `XE_ACCOUNT_ID` and `XE_API_KEY` environment variables when starting the bot.

# How To Use

First, register a new bot in your workspace by clicking `Create New App` at https://api.slack.com/apps. Create from scratch. You can name your bot whatever you want, `Tag1bot`, `Sea Cow`, `Druplicon`, whatever you prefer!

Next, clone the `tag1bot` repo:
```bash
git clone git@github.com:tag1consulting/tag1bot.git
```

Next, find your secrets:

 - `SLACK_APP_TOKEN` -- go to `Basic Information` in your newly created app, and `Generate Token and Scopes`, starting with `connections:write`. You need the token that starts with `xapp-`.
 - `SLACK_BOT_TOKEN` -- go to `OAuth & Permissions` and grab the `User OAuth token` that starts with `xoxp-`. Also scroll down to `Bot Token Scopes` and grant the following:
   - `app_mentions:read`
   - `channels:history`
   - `channels:write`
   - `chat:write`
   - `groups:history`
   - `groups:read`
   - `im:history`
   - `im:read`
   - `mpim:history`
   - `users:read`
   - `users:write`
- `SLACK_CHANNEL_ID` -- pick the main home for your bot, for example `general`

Finally, start your app setting the above secrets in your environment in the most secure way you know. With all these secrets set, you should be able to start the bot with `cargo run --release`, for example:

```bash
SLACK_APP_TOKEN=xapp-... \
SLACK_BOT_TOKEN=xoxb-... \
SLACK_CHANNEL_ID=general \
RUST_LOG=warn \
cargo run --release
```

The bot will create an sqlite database called `state.sqlite` which stores all state. If you delete this file, the bot will forget all recorded karma, the last time it's seen users, and so on.

## Why didn't you port my favorite bot feature?

PR's welcome!! https://github.com/tag1consulting/tag1bot/pulls