//! Storage — **ephemeral by default; opt-in local `SQLite`** only
//! (`docs/PROJECT.md` §4/§8; D-001). A persistent-store-as-product is
//! the kill-zone; this is a local file the user explicitly asks for
//! with `--save`, never a server, never default-on.

use std::path::Path;

use rusqlite::Connection;

use crate::timeline::Timeline;

/// Where a captured timeline goes. `Ephemeral` is the default and a
/// no-op (the `Timeline` already lives in memory for rendering).
#[derive(Debug, Clone)]
pub enum Sink {
    Ephemeral,
    Sqlite(std::path::PathBuf),
}

/// The opt-in local schema. Kept as one constant so it can be
/// snapshot-tested (the schema is a contract).
pub const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS session (
    id          INTEGER PRIMARY KEY,
    created_at  TEXT NOT NULL,
    ctx_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS step (
    session_id    INTEGER NOT NULL REFERENCES session(id),
    idx           INTEGER NOT NULL,
    provider      TEXT,
    req_method    TEXT NOT NULL,
    req_path      TEXT NOT NULL,
    status        INTEGER,
    prompt_tokens INTEGER NOT NULL,
    PRIMARY KEY (session_id, idx)
);
CREATE TABLE IF NOT EXISTS capture (
    session_id INTEGER NOT NULL REFERENCES session(id),
    idx        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    body       BLOB NOT NULL,
    PRIMARY KEY (session_id, idx, kind)
);";

/// Persist a timeline to the sink. `Ephemeral` does nothing.
///
/// # Errors
/// Returns [`crate::Error::Persistence`] on any `SQLite` failure.
pub fn persist(timeline: &Timeline, sink: &Sink) -> crate::Result<()> {
    let path = match sink {
        Sink::Ephemeral => return Ok(()),
        Sink::Sqlite(p) => p,
    };
    let conn = Connection::open(path)?;
    write_into(&conn, timeline)
}

fn write_into(conn: &Connection, timeline: &Timeline) -> crate::Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT INTO session (created_at, ctx_version) VALUES (?1, ?2)",
        rusqlite::params![now_epoch_secs(), env!("CARGO_PKG_VERSION")],
    )?;
    let session_id = conn.last_insert_rowid();
    for step in &timeline.steps {
        let provider = step.provider.map(|p| match p {
            crate::adapter::Provider::Anthropic => "anthropic",
            crate::adapter::Provider::OpenAiCompat => "open_ai_compat",
        });
        let status = step.response.as_ref().map(|r| i64::from(r.status));
        conn.execute(
            "INSERT INTO step (session_id, idx, provider, req_method, req_path, status, prompt_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                session_id,
                i64::try_from(step.index).unwrap_or(i64::MAX),
                provider,
                step.request.method,
                step.request.path,
                status,
                i64::try_from(step.prompt_tokens).unwrap_or(i64::MAX),
            ],
        )?;
        conn.execute(
            "INSERT INTO capture (session_id, idx, kind, body) VALUES (?1, ?2, 'request', ?3)",
            rusqlite::params![
                session_id,
                i64::try_from(step.index).unwrap_or(i64::MAX),
                step.request.body.as_bytes(),
            ],
        )?;
        if let Some(resp) = &step.response {
            conn.execute(
                "INSERT INTO capture (session_id, idx, kind, body) VALUES (?1, ?2, 'response', ?3)",
                rusqlite::params![
                    session_id,
                    i64::try_from(step.index).unwrap_or(i64::MAX),
                    resp.body.as_bytes(),
                ],
            )?;
        }
    }
    Ok(())
}

/// Load the most recent saved session back into a `Timeline` (the
/// post-hoc `ctx open` path). Round-trips the schema contract.
///
/// # Errors
/// Returns [`crate::Error::Persistence`] on any `SQLite` failure.
pub fn load(path: &Path) -> crate::Result<Timeline> {
    let conn = Connection::open(path)?;
    let session_id: i64 =
        conn.query_row("SELECT id FROM session ORDER BY id DESC LIMIT 1", [], |r| {
            r.get(0)
        })?;

    let mut timeline = Timeline::new();
    // provider + prompt_tokens are intentionally re-derived from the
    // stored verbatim body on read (consistent round-trip), so only the
    // transport facts are selected here.
    let mut stmt = conn.prepare(
        "SELECT idx, req_method, req_path, status
         FROM step WHERE session_id = ?1 ORDER BY idx",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;

    for row in rows {
        let (idx, method, path, status) = row?;
        // The request capture is `BLOB NOT NULL` and always written by
        // `write_into`; its absence means a corrupt/foreign DB — the
        // exact thing `ctx open` must surface, not silently paper over.
        let body: Vec<u8> = conn.query_row(
            "SELECT body FROM capture WHERE session_id=?1 AND idx=?2 AND kind='request'",
            rusqlite::params![session_id, idx],
            |r| r.get(0),
        )?;
        let resp_body: Option<Vec<u8>> = conn
            .query_row(
                "SELECT body FROM capture WHERE session_id=?1 AND idx=?2 AND kind='response'",
                rusqlite::params![session_id, idx],
                |r| r.get(0),
            )
            .ok();

        let i = timeline.record_request(&method, &path, &[], &body);
        if let Some(rb) = resp_body {
            let st = u16::try_from(status.unwrap_or(0)).unwrap_or(0);
            timeline.record_response(i, st, &[], &rb);
        }
    }
    Ok(timeline)
}

fn now_epoch_secs() -> String {
    // Avoid pulling `chrono` for one timestamp: seconds since epoch is a
    // stable, dependency-free, snapshot-redactable value (the column is
    // `created_at TEXT`, format `epoch:<secs>` — name says what it is).
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("epoch:{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_is_noop() {
        let t = Timeline::new();
        assert!(persist(&t, &Sink::Ephemeral).is_ok());
    }

    #[test]
    fn sqlite_roundtrip_persists_steps() {
        let mut t = Timeline::new();
        t.record_request("POST", "/v1/messages", &[], b"{\"messages\":[]}");
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("s.sqlite");
        persist(&t, &Sink::Sqlite(db.clone())).unwrap();

        let conn = Connection::open(&db).unwrap();
        let steps: i64 = conn
            .query_row("SELECT COUNT(*) FROM step", [], |r| r.get(0))
            .unwrap();
        let caps: i64 = conn
            .query_row("SELECT COUNT(*) FROM capture", [], |r| r.get(0))
            .unwrap();
        assert_eq!(steps, 1);
        assert_eq!(caps, 1);

        // Pins the `created_at` contract (kills the `now_epoch_secs`
        // -> ""/"xyzzy" mutants — the timestamp must be the documented
        // `epoch:<secs>` form, not empty/arbitrary).
        let created_at: String = conn
            .query_row("SELECT created_at FROM session", [], |r| r.get(0))
            .unwrap();
        assert!(
            created_at.starts_with("epoch:"),
            "created_at must be epoch:<secs>, got {created_at:?}"
        );
        assert!(created_at["epoch:".len()..].parse::<u64>().is_ok());
    }

    #[test]
    fn load_roundtrips_a_saved_session() {
        let mut t = Timeline::new();
        let i = t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"s","messages":[{"role":"user","content":"hi"}]}"#,
        );
        t.record_response(i, 201, &[], b"{\"id\":\"x\"}");

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("s.sqlite");
        persist(&t, &Sink::Sqlite(db.clone())).unwrap();

        let back = load(&db).unwrap();
        assert_eq!(back.steps.len(), 1);
        let s = &back.steps[0];
        assert_eq!(s.request.method, "POST");
        assert_eq!(s.request.path, "/v1/messages");
        assert_eq!(s.request.body, t.steps[0].request.body);
        assert_eq!(s.provider, Some(crate::adapter::Provider::Anthropic));
        assert_eq!(s.response.as_ref().unwrap().status, 201);
        assert_eq!(s.prompt_tokens, t.steps[0].prompt_tokens);
    }

    #[test]
    fn load_missing_session_is_persistence_error() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.sqlite");
        Connection::open(&empty)
            .unwrap()
            .execute_batch(SCHEMA)
            .unwrap();
        assert!(matches!(load(&empty), Err(crate::Error::Persistence(_))));
    }

    #[test]
    fn schema_is_stable_contract() {
        // Guards the on-disk contract; the full text is snapshot-tested
        // in tests/snapshots.
        assert!(SCHEMA.contains("CREATE TABLE IF NOT EXISTS session"));
        assert!(SCHEMA.contains("CREATE TABLE IF NOT EXISTS step"));
        assert!(SCHEMA.contains("CREATE TABLE IF NOT EXISTS capture"));
    }
}
