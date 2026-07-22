use crate::{models::SearchResult, state::AppState};
use rusqlite::{params, Connection, OptionalExtension};
use std::{cmp::Ordering, collections::HashMap};

const MAX_TERMS: usize = 8;
const MAX_TERM_RESULTS: usize = 500;
const MAX_RESULTS: usize = 100;

#[derive(Clone)]
struct SessionRow {
    provider: String,
    session_id: String,
    source_path: String,
    cwd: Option<String>,
    title: Option<String>,
    started_at: Option<String>,
    updated_at: String,
    file_size: u64,
}

struct Hit {
    count: u64,
    rank: f64,
    snippet: Option<String>,
    role: Option<String>,
}

pub fn search_sessions(
    state: &AppState,
    query: &str,
    provider: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let connection = state.connect()?;
    let sessions = load_sessions(&connection, provider)?;
    let terms = search_terms(query);
    let effective_limit = limit.clamp(1, MAX_RESULTS);

    if terms.is_empty() {
        return Ok(sessions
            .into_iter()
            .take(effective_limit)
            .map(|session| to_result(session, 0, None, None))
            .collect());
    }

    let mut combined: Option<HashMap<String, Hit>> = None;
    for term in &terms {
        let current = query_term(&connection, term, provider)?;
        combined = Some(match combined {
            None => current,
            Some(mut existing) => {
                existing.retain(|path, hit| {
                    if let Some(next) = current.get(path) {
                        hit.count += next.count;
                        hit.rank += next.rank;
                        true
                    } else {
                        false
                    }
                });
                existing
            }
        });
    }

    let mut hits = combined.unwrap_or_default();
    let metadata_query = query.trim().to_lowercase();
    for session in &sessions {
        let metadata_matches = [
            Some(session.session_id.as_str()),
            Some(session.source_path.as_str()),
            session.cwd.as_deref(),
            session.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.to_lowercase().contains(&metadata_query));

        if metadata_matches {
            hits.entry(session.source_path.clone()).or_insert(Hit {
                count: 1,
                rank: -100.0,
                snippet: None,
                role: None,
            });
        }
    }

    let session_map: HashMap<String, SessionRow> = sessions
        .into_iter()
        .map(|session| (session.source_path.clone(), session))
        .collect();
    let mut ranked: Vec<(SessionRow, Hit)> = hits
        .into_iter()
        .filter_map(|(path, hit)| {
            session_map
                .get(&path)
                .cloned()
                .map(|session| (session, hit))
        })
        .collect();

    ranked.sort_by(|(left_session, left_hit), (right_session, right_hit)| {
        left_hit
            .rank
            .partial_cmp(&right_hit.rank)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right_session.updated_at.cmp(&left_session.updated_at))
    });

    let mut results = Vec::new();
    for (session, mut hit) in ranked.into_iter().take(effective_limit) {
        if hit.rank > -50.0 {
            let (snippet, role) = query_best_snippet(&connection, &terms[0], &session.source_path)?;
            hit.snippet = snippet;
            hit.role = role;
        }
        results.push(to_result(session, hit.count, hit.snippet, hit.role));
    }
    Ok(results)
}

fn load_sessions(connection: &Connection, provider: &str) -> Result<Vec<SessionRow>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT provider, session_id, source_path, cwd, title, started_at, updated_at, file_size
            FROM sessions
            WHERE (?1 = 'all' OR provider = ?1)
            ORDER BY updated_at DESC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([provider], |row| {
            Ok(SessionRow {
                provider: row.get(0)?,
                session_id: row.get(1)?,
                source_path: row.get(2)?,
                cwd: row.get(3)?,
                title: row.get(4)?,
                started_at: row.get(5)?,
                updated_at: row.get(6)?,
                file_size: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

fn query_term(
    connection: &Connection,
    term: &str,
    provider: &str,
) -> Result<HashMap<String, Hit>, String> {
    let fts_query = format!("\"{}\"", term.replace('"', "\"\""));
    let mut statement = connection
        .prepare(
            r#"
            SELECT
                messages.source_path,
                COUNT(*) AS hit_count,
                MIN(messages.rank) AS best_rank
            FROM messages
            JOIN sessions ON sessions.source_path = messages.source_path
            WHERE messages MATCH ?1
              AND (?2 = 'all' OR sessions.provider = ?2)
            GROUP BY messages.source_path
            ORDER BY best_rank ASC
            LIMIT ?3
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![fts_query, provider, MAX_TERM_RESULTS], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Hit {
                    count: row.get(1)?,
                    rank: row.get(2)?,
                    snippet: None,
                    role: None,
                },
            ))
        })
        .map_err(|error| error.to_string())?;

    let mut hits = HashMap::new();
    for row in rows {
        let (path, hit) = row.map_err(|error| error.to_string())?;
        hits.insert(path, hit);
    }
    Ok(hits)
}

fn query_best_snippet(
    connection: &Connection,
    term: &str,
    source_path: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let fts_query = format!("\"{}\"", term.replace('"', "\"\""));
    connection
        .query_row(
            r#"
            SELECT snippet(messages, 0, '[[[', ']]]', ' … ', 22), role
            FROM messages
            WHERE messages MATCH ?1 AND source_path = ?2
            ORDER BY rank ASC
            LIMIT 1
            "#,
            params![fts_query, source_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map(|value| value.unwrap_or((None, None)))
        .map_err(|error| error.to_string())
}

fn search_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| term.trim_matches('"').trim())
        .filter(|term| !term.is_empty())
        .take(MAX_TERMS)
        .map(str::to_lowercase)
        .collect()
}

fn to_result(
    session: SessionRow,
    match_count: u64,
    snippet: Option<String>,
    snippet_role: Option<String>,
) -> SearchResult {
    SearchResult {
        provider: session.provider,
        session_id: session.session_id,
        source_path: session.source_path,
        cwd: session.cwd,
        title: session.title,
        started_at: session.started_at,
        updated_at: session.updated_at,
        file_size: session.file_size,
        match_count,
        snippet,
        snippet_role,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_limits_search_terms() {
        let terms = search_terms("  db.ts  auth route api test one two three nine ");
        assert_eq!(terms.len(), MAX_TERMS);
        assert_eq!(terms[0], "db.ts");
    }
}
