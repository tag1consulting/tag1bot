# Tag1bot

Porting parts of https://www.drupal.org/project/bot into a Rust-powered slackbot.

## Karma

The bot increases karma for `foo++`-style commands, and decreases karma for `foo--`-style commands. Karma is the total number of times a given word has been incremented or decremented. Words must be 2 to 20 characters long, without any spaces.

