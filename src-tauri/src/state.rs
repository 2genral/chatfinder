use rusqlite::Connection;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

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
