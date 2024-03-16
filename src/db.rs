// Functions for creating, updating and sharing the sqlite database.

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

// Write state database in the current working direcrtly.
const DATABASE_FILE: &str = "./state.sqlite3";

// Open the database file once and share as needed.
lazy_static! {
    pub(crate) static ref DB: Arc<Mutex<Connection>> = Arc::new(Mutex::new(
        Connection::open(DATABASE_FILE)
            .unwrap_or_else(|_| panic!("failed to open {}", DATABASE_FILE))
    ));
}

// Create all tables and indexes at startup.
pub(crate) fn setup() {
    let db = DB.lock().unwrap_or_else(|_| panic!("DB mutex poisoned!"));

    // Create the karma table if it doesn't already exist.
    db.execute(
        "CREATE TABLE IF NOT EXISTS karma (
        id              INTEGER PRIMARY KEY,
        name            TEXT NOT NULL,
        counter         INTEGER
            )",
        [],
    )
    .expect("failed to create karma table");
    db.execute("CREATE INDEX IF NOT EXISTS i_name ON karma (name)", [])
        .expect("failed to create index karma.i_name");

    // Create the seen table if it doesn't already exist.
    db.execute(
        "CREATE TABLE IF NOT EXISTS seen (
        id              INTEGER PRIMARY KEY,
        channel         TEXT NOT NULL,
        user            TEXT NOT NULL,
        last_said       TEXT NOT NULL,
        last_seen       INTEGER,
        last_private    INTEGER
            )",
        [],
    )
    .expect("failed to create seen table");
    db.execute("CREATE INDEX IF NOT EXISTS i_name ON seen (name)", [])
        .expect("failed to create seen seen.i_name");

    // Create the currency_alert table if it doesn't already exist.
    db.execute(
        "CREATE TABLE IF NOT EXISTS currency_alert (
        id              INTEGER PRIMARY KEY,
        channel         TEXT NOT NULL,
        user            TEXT NOT NULL,
        from_currency   TEXT NOT NULL,
        from_amount     REAL,
        comparison      TEXT NOT NULL,
        to_currency     TEXT NOT NULL,
        to_amount       REAL
            )",
        [],
    )
    .expect("failed to create currency_alert table");

    // Create the chatgpt_threads table if it doesn't already exist.
    db.execute(
        "CREATE TABLE IF NOT EXISTS chatgpt_context (
        id              INTEGER PRIMARY KEY,
        thread          TEXT NOT NULL,
        context         TEXT NOT NULL
            )",
        [],
    )
    .expect("failed to create chatgpt_context table");
    db.execute(
        "CREATE INDEX IF NOT EXISTS i_thread ON chatgpt_context (thread)",
        [],
    )
    .expect("failed to create index chatgpt_context.i_thread");

    // Create the claude_context table if it doesn't already exist.
    db.execute(
        "CREATE TABLE IF NOT EXISTS claude_context (
        id              INTEGER PRIMARY KEY,
        thread          TEXT NOT NULL,
        context         TEXT NOT NULL
            )",
        [],
    )
    .expect("failed to create claude_context table");
    db.execute(
        "CREATE INDEX IF NOT EXISTS i_thread ON claude_context (thread)",
        [],
    )
    .expect("failed to create index claude.i_thread");
}
