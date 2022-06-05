// Custom Slack functionality.
use serde::{Deserialize, Serialize};
use std::env;

// Calls to users_info return the following.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UserWrapper {
    ok: bool,
    user: Option<User>,
    error: Option<String>,
}

// All available user info, see https://api.slack.com/methods/users.info.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct User {
    pub(crate) id: String,
    pub(crate) team_id: String,
    pub(crate) name: String,
    pub(crate) real_name: String,
    pub(crate) tz: Option<String>,
    pub(crate) tz_label: Option<String>,
    pub(crate) tz_offset: Option<i32>,
    pub(crate) profile: Profile,
    pub(crate) is_admin: bool,
    pub(crate) is_owner: bool,
    pub(crate) is_restricted: bool,
    pub(crate) is_ultra_restricted: bool,
    pub(crate) is_bot: bool,
    pub(crate) updated: u32,
    pub(crate) is_app_user: bool,
    pub(crate) has_2fa: Option<bool>,
}

// Profile information included about user.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Profile {
    pub(crate) status_text: String,
    pub(crate) status_emoji: String,
    pub(crate) real_name: String,
    pub(crate) display_name: String,
    pub(crate) real_name_normalized: String,
    pub(crate) display_name_normalized: String,
    pub(crate) email: Option<String>,
    pub(crate) team: String,
}

// Get full details about a user by id.
pub(crate) async fn users_info(user_id: &str) -> Result<User, String> {
    let slack_bot_token = env::var("SLACK_BOT_TOKEN")
        .unwrap_or_else(|_| panic!("slack bot token is not set (starts with 'xoxb')."));

    println!("user request: {:#?}", user_id);

    let user_wrapper: UserWrapper =
        surf::post(format!("https://slack.com/api/users.info?user={}", user_id))
            .header("Authorization", format!("Bearer {}", slack_bot_token))
            .recv_json()
            .await
            .unwrap();

    // No need to check `ok`, just check if the user exists.
    if let Some(user) = user_wrapper.user {
        Ok(user)
    // Otherwise we got an error.
    } else if let Some(error) = user_wrapper.error {
        Err(error)
    } else {
        // Debug output if somehow this happened:
        log::error!("user_wrapper: {:#?}", user_wrapper);
        unreachable!("No user and no error!?");
    }
}
