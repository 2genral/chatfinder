use rusqlite::Connection;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

const INDEX_VERSION: i64 = 2;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub indexing: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        let state = Self {
            db_path,
            indexing: Arc::new(AtomicBool::new(false)),
        };
        state.initialize_database()?;
        Ok(state)
    }

    pub fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.db_path).map_err(|error| error.to_string())?;
        configure_connection(&connection)?;
        Ok(connection)
    }

    fn initialize_database(&self) -> Result<(), String> {
        let connection = self.connect()?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS sessions (
                    source_path TEXT PRIMARY KEY,
                    provider TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    cwd TEXT,
                    title TEXT,
                    started_at TEXT,
                    updated_at TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    modified_ns INTEGER NOT NULL,
                    indexed_messages INTEGER NOT NULL DEFAULT 0,
                    indexed_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS sessions_provider_updated
                    ON sessions(provider, updated_at DESC);

                CREATE VIRTUAL TABLE IF NOT EXISTS messages USING fts5(
                    text,
                    role UNINDEXED,
                    source_path UNINDEXED,
                    tokenize = 'unicode61 remove_diacritics 2 tokenchars ''._-/'''
                );
                "#,
            )
            .map_err(|error| error.to_string())?;
        let current_version = connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())?;
        if current_version < INDEX_VERSION {
            connection
                .execute_batch("DELETE FROM messages; DELETE FROM sessions;")
                .map_err(|error| error.to_string())?;
            connection
                .pragma_update(None, "user_version", INDEX_VERSION)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

fn configure_connection(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA foreign_keys = ON;
            "#,
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_an_outdated_index() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "chatfinder-state-test-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let database = directory.join("test.sqlite3");
        let state = AppState::new(database.clone()).unwrap();
        let connection = state.connect().unwrap();
        connection
            .execute_batch(
                r#"
                INSERT INTO sessions(
                    source_path, provider, session_id, updated_at, file_size,
                    modified_ns, indexed_messages, indexed_at
                ) VALUES ('/tmp/chat.jsonl', 'codex', 'chat-id', '2026-07-22', 1, 1, 1, '2026-07-22');
                INSERT INTO messages(text, role, source_path)
                VALUES ('message', 'user', '/tmp/chat.jsonl');
                PRAGMA user_version = 1;
                "#,
            )
            .unwrap();
        drop(connection);

        let upgraded = AppState::new(database).unwrap();
        let connection = upgraded.connect().unwrap();
        let sessions: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        let messages: i64 = connection
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!((sessions, messages, version), (0, 0, INDEX_VERSION));
        drop(connection);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
