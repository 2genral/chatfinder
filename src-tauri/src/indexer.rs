use crate::{
    models::{AppStatus, IndexProgress, IndexSummary, Provider, SourceFile, SourceRoot},
    state::AppState,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    fs::File,
    hash::{Hash, Hasher},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::UNIX_EPOCH,
};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;
const MAX_FRAGMENT_BYTES: usize = 512 * 1024;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Default)]
struct SessionMetadata {
    session_id: String,
    cwd: Option<String>,
    title: Option<String>,
    started_at: Option<String>,
}

struct IndexedMessage {
    role: String,
    text: String,
}

pub fn source_roots() -> Vec<(Provider, PathBuf)> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    vec![
        (Provider::Claude, home.join(".claude").join("projects")),
        (Provider::Codex, home.join(".codex").join("sessions")),
        (
            Provider::Codex,
            home.join(".codex").join("archived_sessions"),
        ),
    ]
}

pub fn discover_sources() -> Vec<SourceFile> {
    let mut sources = Vec::new();

    for (provider, root) in source_roots() {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
            {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
            let modified_ns = modified
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
                .unwrap_or_default();
            let updated_at: DateTime<Utc> = modified.into();

            sources.push(SourceFile {
                provider,
                path: entry.path().to_path_buf(),
                size: metadata.len(),
                modified_ns,
                updated_at: updated_at.to_rfc3339(),
            });
        }
    }

    sources.sort_by_key(|source| std::cmp::Reverse(source.modified_ns));
    sources
}

pub fn load_status(state: &AppState) -> Result<AppStatus, String> {
    let sources = discover_sources();
    let connection = state.connect()?;
    let (indexed_sessions, indexed_messages, last_indexed_at) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(indexed_messages), 0), MAX(indexed_at) FROM sessions",
            [],
            |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;

    Ok(AppStatus {
        indexed_sessions,
        indexed_messages,
        source_files: sources.len() as u64,
        source_bytes: sources.iter().map(|source| source.size).sum(),
        last_indexed_at,
        is_indexing: state.indexing.load(Ordering::SeqCst),
        roots: source_roots()
            .into_iter()
            .map(|(provider, path)| SourceRoot {
                provider: provider.as_str().to_string(),
                exists: path.exists(),
                path: path.to_string_lossy().into_owned(),
            })
            .collect(),
    })
}

pub fn sync_index(app: &AppHandle, state: &AppState) -> Result<IndexSummary, String> {
    if state.indexing.swap(true, Ordering::SeqCst) {
        return Err("Indexing is already running".to_string());
    }

    let result = sync_index_inner(app, state);
    state.indexing.store(false, Ordering::SeqCst);
    result
}

fn sync_index_inner(app: &AppHandle, state: &AppState) -> Result<IndexSummary, String> {
    let sources = discover_sources();
    let total = sources.len();
    let mut connection = state.connect()?;
    let existing = existing_sources(&connection)?;
    let discovered_paths: HashSet<String> = sources
        .iter()
        .map(|source| source.path.to_string_lossy().into_owned())
        .collect();
    let titles = codex_titles();

    let mut indexed = 0;
    let mut skipped = 0;
    let mut messages = 0;

    emit_progress(
        app,
        IndexProgress {
            phase: "indexing".to_string(),
            processed: 0,
            total,
            indexed,
            skipped,
            current_path: None,
        },
    );

    for (position, source) in sources.iter().enumerate() {
        let source_path = source.path.to_string_lossy().into_owned();
        let unchanged = existing.get(&source_path).is_some_and(|(size, modified)| {
            *size == source.size && *modified == source.modified_ns
        });

        if unchanged {
            skipped += 1;
        } else {
            match index_source(&mut connection, source, &titles) {
                Ok(indexed_messages) => {
                    indexed += 1;
                    messages += indexed_messages;
                }
                Err(_) => skipped += 1,
            }
        }

        emit_progress(
            app,
            IndexProgress {
                phase: "indexing".to_string(),
                processed: position + 1,
                total,
                indexed,
                skipped,
                current_path: Some(source_path),
            },
        );
    }

    let removed = remove_missing_sources(&mut connection, &existing, &discovered_paths)?;
    emit_progress(
        app,
        IndexProgress {
            phase: "complete".to_string(),
            processed: total,
            total,
            indexed,
            skipped,
            current_path: None,
        },
    );

    Ok(IndexSummary {
        discovered: total,
        indexed,
        skipped,
        removed,
        messages,
    })
}

fn existing_sources(connection: &Connection) -> Result<HashMap<String, (u64, i64)>, String> {
    let mut statement = connection
        .prepare("SELECT source_path, file_size, modified_ns FROM sessions")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut sources = HashMap::new();
    for row in rows {
        let (path, size, modified) = row.map_err(|error| error.to_string())?;
        sources.insert(path, (size, modified));
    }
    Ok(sources)
}

fn index_source(
    connection: &mut Connection,
    source: &SourceFile,
    titles: &HashMap<String, String>,
) -> Result<usize, String> {
    let source_path = source.path.to_string_lossy().into_owned();
    let mut metadata = SessionMetadata {
        session_id: fallback_session_id(&source.path),
        ..SessionMetadata::default()
    };
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM messages WHERE source_path = ?1",
            [&source_path],
        )
        .map_err(|error| error.to_string())?;

    let file = File::open(&source.path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::with_capacity(128 * 1024, file);
    let mut buffer = Vec::with_capacity(32 * 1024);
    let mut seen = HashSet::new();
    let mut message_count = 0;

    {
        let mut insert = transaction
            .prepare("INSERT INTO messages(text, role, source_path) VALUES (?1, ?2, ?3)")
            .map_err(|error| error.to_string())?;

        while let Some(within_limit) = read_capped_line(&mut reader, &mut buffer, MAX_LINE_BYTES)
            .map_err(|error| error.to_string())?
        {
            if !within_limit || buffer.is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_slice::<Value>(&buffer) else {
                continue;
            };

            update_metadata(source.provider, &record, &mut metadata);
            for message in extract_messages(source.provider, &record) {
                if message.text.is_empty() || message.text.len() > MAX_MESSAGE_BYTES {
                    continue;
                }
                let mut hasher = DefaultHasher::new();
                message.role.hash(&mut hasher);
                message.text.hash(&mut hasher);
                if !seen.insert(hasher.finish()) {
                    continue;
                }
                if metadata.title.is_none() && message.role == "user" {
                    metadata.title = title_from_text(&message.text);
                }
                insert
                    .execute(params![message.text, message.role, source_path])
                    .map_err(|error| error.to_string())?;
                message_count += 1;
            }
        }
    }

    if let Some(index_title) = titles.get(&metadata.session_id) {
        metadata.title = Some(index_title.clone());
    }

    transaction
        .execute(
            r#"
            INSERT INTO sessions(
                source_path, provider, session_id, cwd, title, started_at, updated_at,
                file_size, modified_ns, indexed_messages, indexed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(source_path) DO UPDATE SET
                provider = excluded.provider,
                session_id = excluded.session_id,
                cwd = excluded.cwd,
                title = excluded.title,
                started_at = excluded.started_at,
                updated_at = excluded.updated_at,
                file_size = excluded.file_size,
                modified_ns = excluded.modified_ns,
                indexed_messages = excluded.indexed_messages,
                indexed_at = excluded.indexed_at
            "#,
            params![
                source_path,
                source.provider.as_str(),
                metadata.session_id,
                metadata.cwd,
                metadata.title,
                metadata.started_at,
                source.updated_at,
                source.size,
                source.modified_ns,
                message_count,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(message_count)
}

fn remove_missing_sources(
    connection: &mut Connection,
    existing: &HashMap<String, (u64, i64)>,
    discovered: &HashSet<String>,
) -> Result<usize, String> {
    let missing: Vec<&String> = existing
        .keys()
        .filter(|path| !discovered.contains(*path))
        .collect();
    if missing.is_empty() {
        return Ok(0);
    }

    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for path in &missing {
        transaction
            .execute(
                "DELETE FROM messages WHERE source_path = ?1",
                [path.as_str()],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM sessions WHERE source_path = ?1",
                [path.as_str()],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(missing.len())
}

fn update_metadata(provider: Provider, record: &Value, metadata: &mut SessionMetadata) {
    match provider {
        Provider::Claude => {
            if let Some(session_id) = record.get("sessionId").and_then(Value::as_str) {
                metadata.session_id = session_id.to_string();
            }
            if metadata.cwd.is_none() {
                metadata.cwd = record
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if metadata.started_at.is_none() {
                metadata.started_at = record
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
        }
        Provider::Codex => {
            if record.get("type").and_then(Value::as_str) != Some("session_meta") {
                return;
            }
            let Some(payload) = record.get("payload") else {
                return;
            };
            if let Some(session_id) = payload.get("id").and_then(Value::as_str) {
                metadata.session_id = session_id.to_string();
            }
            metadata.cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string);
            metadata.started_at = payload
                .get("timestamp")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
}

fn extract_messages(provider: Provider, record: &Value) -> Vec<IndexedMessage> {
    match provider {
        Provider::Claude => extract_claude_messages(record),
        Provider::Codex => extract_codex_messages(record),
    }
}

fn extract_claude_messages(record: &Value) -> Vec<IndexedMessage> {
    let record_type = record
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(record_type, "user" | "assistant") {
        return Vec::new();
    }
    let Some(message) = record.get("message") else {
        return Vec::new();
    };
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or(record_type);
    let Some(content) = message.get("content") else {
        return Vec::new();
    };

    let mut visible = Vec::new();
    let mut tools = Vec::new();
    extract_claude_content(content, &mut visible, &mut tools);
    let mut messages = Vec::new();
    if let Some(text) = join_fragments(visible) {
        messages.push(IndexedMessage {
            role: role.to_string(),
            text,
        });
    }
    if let Some(text) = join_fragments(tools) {
        messages.push(IndexedMessage {
            role: "tool".to_string(),
            text,
        });
    }
    messages
}

fn extract_claude_content(content: &Value, visible: &mut Vec<String>, tools: &mut Vec<String>) {
    match content {
        Value::String(text) => push_fragment(visible, text),
        Value::Array(blocks) => {
            for block in blocks {
                let block_type = block
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            push_fragment(visible, text);
                        }
                    }
                    "tool_use" => {
                        if let Some(name) = block.get("name").and_then(Value::as_str) {
                            push_fragment(tools, name);
                        }
                        if let Some(input) = block.get("input") {
                            collect_strings(input, tools);
                        }
                    }
                    "tool_result" => {
                        if let Some(result) = block.get("content") {
                            collect_strings(result, tools);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn extract_codex_messages(record: &Value) -> Vec<IndexedMessage> {
    let record_type = record
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(payload) = record.get("payload") else {
        return Vec::new();
    };
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut fragments = Vec::new();

    match (record_type, payload_type) {
        ("response_item", "message") => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            if let Some(content) = payload.get("content") {
                extract_codex_content(content, &mut fragments);
            }
            join_fragments(fragments)
                .map(|text| {
                    vec![IndexedMessage {
                        role: role.to_string(),
                        text,
                    }]
                })
                .unwrap_or_default()
        }
        ("response_item", "function_call") | ("response_item", "custom_tool_call") => {
            if let Some(name) = payload.get("name").and_then(Value::as_str) {
                push_fragment(&mut fragments, name);
            }
            for key in ["arguments", "input"] {
                if let Some(value) = payload.get(key) {
                    collect_strings(value, &mut fragments);
                }
            }
            join_fragments(fragments)
                .map(|text| {
                    vec![IndexedMessage {
                        role: "tool".to_string(),
                        text,
                    }]
                })
                .unwrap_or_default()
        }
        ("response_item", "function_call_output")
        | ("response_item", "custom_tool_call_output") => {
            if let Some(output) = payload.get("output") {
                collect_strings(output, &mut fragments);
            }
            join_fragments(fragments)
                .map(|text| {
                    vec![IndexedMessage {
                        role: "tool".to_string(),
                        text,
                    }]
                })
                .unwrap_or_default()
        }
        ("event_msg", "user_message") => event_message(payload, "user"),
        ("event_msg", "agent_message") => event_message(payload, "assistant"),
        _ => Vec::new(),
    }
}

fn event_message(payload: &Value, role: &str) -> Vec<IndexedMessage> {
    let Some(message) = payload.get("message").and_then(Value::as_str) else {
        return Vec::new();
    };
    let text = normalize_text(message);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![IndexedMessage {
            role: role.to_string(),
            text,
        }]
    }
}

fn extract_codex_content(content: &Value, fragments: &mut Vec<String>) {
    let Value::Array(items) = content else {
        collect_strings(content, fragments);
        return;
    };
    for item in items {
        for key in ["text", "input_text", "output_text"] {
            if let Some(text) = item.get(key).and_then(Value::as_str) {
                push_fragment(fragments, text);
            }
        }
    }
}

fn collect_strings(value: &Value, fragments: &mut Vec<String>) {
    match value {
        Value::String(text) => push_fragment(fragments, text),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, fragments);
            }
        }
        Value::Object(object) => {
            for (key, item) in object {
                if matches!(
                    key.as_str(),
                    "encrypted_content" | "image_url" | "data" | "blob" | "base64"
                ) {
                    continue;
                }
                collect_strings(item, fragments);
            }
        }
        _ => {}
    }
}

fn push_fragment(fragments: &mut Vec<String>, text: &str) {
    if text.is_empty() || text.len() > MAX_FRAGMENT_BYTES || text.starts_with("data:") {
        return;
    }
    let normalized = normalize_text(text);
    if !normalized.is_empty() {
        fragments.push(normalized);
    }
}

fn join_fragments(fragments: Vec<String>) -> Option<String> {
    if fragments.is_empty() {
        return None;
    }
    let mut joined = String::new();
    for fragment in fragments {
        if joined.len() + fragment.len() + 1 > MAX_MESSAGE_BYTES {
            break;
        }
        if !joined.is_empty() {
            joined.push_str(" · ");
        }
        joined.push_str(&fragment);
    }
    (!joined.is_empty()).then_some(joined)
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len().min(MAX_MESSAGE_BYTES));
    let mut previous_whitespace = false;
    for character in text.chars() {
        if character.is_whitespace() {
            if !previous_whitespace && !normalized.is_empty() {
                normalized.push(' ');
            }
            previous_whitespace = true;
        } else {
            normalized.push(character);
            previous_whitespace = false;
        }
    }
    normalized.trim().to_string()
}

fn title_from_text(text: &str) -> Option<String> {
    let title: String = text.chars().take(96).collect();
    let title = title.trim();
    (!title.is_empty()).then(|| title.to_string())
}

fn fallback_session_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .rsplit_once('-')
        .filter(|(_, suffix)| suffix.len() == 36)
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
}

fn codex_titles() -> HashMap<String, String> {
    let Some(path) = dirs::home_dir().map(|home| home.join(".codex").join("session_index.jsonl"))
    else {
        return HashMap::new();
    };
    let Ok(file) = File::open(path) else {
        return HashMap::new();
    };
    let reader = BufReader::new(file);
    let mut titles = HashMap::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(title) = value.get("thread_name").and_then(Value::as_str) else {
            continue;
        };
        titles.insert(id.to_string(), title.to_string());
    }
    titles
}

fn read_capped_line<R: BufRead>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
    limit: usize,
) -> io::Result<Option<bool>> {
    buffer.clear();
    let mut skipped = false;
    let mut read_any = false;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if read_any {
                Ok(Some(!skipped))
            } else {
                Ok(None)
            };
        }
        read_any = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        let content_len = newline.unwrap_or(available.len());

        if !skipped {
            if buffer.len() + content_len <= limit {
                buffer.extend_from_slice(&available[..content_len]);
            } else {
                skipped = true;
                buffer.clear();
            }
        }
        reader.consume(consumed);

        if newline.is_some() {
            return Ok(Some(!skipped));
        }
    }
}

fn emit_progress(app: &AppHandle, progress: IndexProgress) {
    let _ = app.emit("index-progress", progress);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_claude_text_and_tool_input() {
        let record = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Found the file"},
                    {"type": "tool_use", "name": "Read", "input": {"file_path": "src/db.ts"}}
                ]
            }
        });
        let messages = extract_claude_messages(&record);
        assert_eq!(messages.len(), 2);
        assert!(messages
            .iter()
            .any(|message| message.text.contains("src/db.ts")));
    }

    #[test]
    fn extracts_codex_visible_message() {
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "find db.ts"}]
            }
        });
        let messages = extract_codex_messages(&record);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "find db.ts");
    }

    #[test]
    fn capped_reader_skips_oversized_lines() {
        let data = b"short\nthis line is too long\nok\n";
        let mut reader = BufReader::new(&data[..]);
        let mut buffer = Vec::new();
        assert_eq!(
            read_capped_line(&mut reader, &mut buffer, 8).unwrap(),
            Some(true)
        );
        assert_eq!(buffer, b"short");
        assert_eq!(
            read_capped_line(&mut reader, &mut buffer, 8).unwrap(),
            Some(false)
        );
        assert!(buffer.is_empty());
        assert_eq!(
            read_capped_line(&mut reader, &mut buffer, 8).unwrap(),
            Some(true)
        );
        assert_eq!(buffer, b"ok");
    }

    #[test]
    fn indexes_and_finds_a_keyword() {
        let nonce = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "chatfinder-index-test-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("test-session.jsonl");
        let records = [
            serde_json::json!({
                "type": "user",
                "sessionId": "test-session",
                "cwd": "/work/example",
                "timestamp": "2026-07-22T00:00:00Z",
                "message": {"role": "user", "content": "Where is db.ts?"}
            }),
            serde_json::json!({
                "type": "assistant",
                "sessionId": "test-session",
                "message": {"role": "assistant", "content": [{"type": "text", "text": "It is under src/db.ts"}]}
            }),
        ];
        let contents = records
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        std::fs::write(&source_path, contents).unwrap();
        let file_metadata = std::fs::metadata(&source_path).unwrap();
        let source = SourceFile {
            provider: Provider::Claude,
            path: source_path,
            size: file_metadata.len(),
            modified_ns: 1,
            updated_at: "2026-07-22T00:00:00Z".to_string(),
        };
        let state = AppState::new(directory.join("test.sqlite3")).unwrap();
        let mut connection = state.connect().unwrap();
        assert_eq!(
            index_source(&mut connection, &source, &HashMap::new()).unwrap(),
            2
        );
        drop(connection);

        let results = crate::search::search_sessions(&state, "db.ts", "all", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "test-session");
        assert!(results[0]
            .snippet
            .as_deref()
            .unwrap()
            .contains("[[[db.ts]]]"));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
