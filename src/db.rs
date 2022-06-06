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
        .expect("failed to create index i_name");
}
