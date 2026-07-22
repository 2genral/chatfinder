use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub provider: Provider,
    pub path: PathBuf,
    pub size: u64,
    pub modified_ns: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRoot {
    pub provider: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub indexed_sessions: u64,
    pub indexed_messages: u64,
    pub source_files: u64,
    pub source_bytes: u64,
    pub last_indexed_at: Option<String>,
    pub is_indexing: bool,
    pub roots: Vec<SourceRoot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub phase: String,
    pub processed: usize,
    pub total: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub current_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSummary {
    pub discovered: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub messages: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub provider: String,
    pub session_id: String,
    pub source_path: String,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub file_size: u64,
    pub match_count: u64,
    pub snippet: Option<String>,
    pub snippet_role: Option<String>,
}
