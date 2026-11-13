//! Local SQLite persistence.
//!
//! Migrations are append-only: never edit a released entry in `MIGRATIONS`, add a new one.
//!
//! - v1: `settings`.
//! - v2: `benchmark_results` (benchmark history) and `diagnostic_reports` (latest 20).
//! - v3: `assistant_events` (audit log of the "Risolvi" assistant, latest 1000; sizes
//!   and token counts only, never the API key or payload content).
//! - v4: `control_documents` (control centers: profiles, selections and the original
//!   system state to restore; one JSON document per key).
//!
//! History rows store the full contract as JSON plus a few indexed columns. A row whose
//! JSON no longer deserializes is skipped with a warning, never fatal.

#![forbid(unsafe_code)]

use std::path::Path;

use models::benchmark::BenchmarkKind;
use models::benchmark_run::BenchmarkResult;
use models::diagnostics::DiagnosticReport;
use models::settings::AppSettings;
use models::{AppError, ErrorCode};
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};
use serde::{Deserialize, Serialize};

const MIGRATIONS_SLICE: &[M<'static>] = &[
    // v1 (released): never edit.
    M::up(
        "CREATE TABLE settings (
        key        TEXT PRIMARY KEY NOT NULL,
        value      TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    ) STRICT;",
    ),
    // v2: benchmark history and diagnostic reports.
    M::up(
        "CREATE TABLE benchmark_results (
        run_id              TEXT PRIMARY KEY NOT NULL,
        kind                TEXT NOT NULL,
        state               TEXT NOT NULL,
        methodology_version INTEGER NOT NULL,
        started_at          INTEGER NOT NULL,
        json                TEXT NOT NULL
    ) STRICT;
    CREATE INDEX benchmark_results_kind_started
        ON benchmark_results (kind, started_at DESC);
    CREATE TABLE diagnostic_reports (
        id           TEXT PRIMARY KEY NOT NULL,
        generated_at INTEGER NOT NULL,
        json         TEXT NOT NULL
    ) STRICT;",
    ),
    // v3: "Risolvi" assistant audit log. Sizes and token counts only: never the API key
    // or the payload content.
    M::up(
        "CREATE TABLE assistant_events (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        at            INTEGER NOT NULL,
        session_id    TEXT,
        rule_id       TEXT,
        model         TEXT,
        kind          TEXT NOT NULL
                      CHECK (kind IN ('preview', 'request', 'plan', 'step', 'error')),
        bytes_sent    INTEGER,
        input_tokens  INTEGER,
        output_tokens INTEGER,
        step_id       TEXT,
        ok            INTEGER CHECK (ok IN (0, 1)),
        message       TEXT
    ) STRICT;
    CREATE INDEX assistant_events_at ON assistant_events (at DESC, id DESC);
    CREATE INDEX assistant_events_session ON assistant_events (session_id, at DESC);",
    ),
    // v4: control centers (Performance, Cooling, Lighting, Gaming). One JSON document per
    // key, owned by the center whose name prefixes the key.
    M::up(
        "CREATE TABLE control_documents (
        key        TEXT PRIMARY KEY NOT NULL,
        json       TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    ) STRICT;",
    ),
];
const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATIONS_SLICE);

const SETTINGS_KEY: &str = "app";

/// Upper bound for the `limit` of `list_benchmark_results` (lower bound is 1).
pub const BENCHMARK_LIST_LIMIT_MAX: u32 = 500;
/// Number of diagnostic reports kept; older ones are pruned on save.
pub const DIAGNOSTIC_REPORTS_KEPT: u32 = 20;
/// Assistant events kept; older ones are pruned on insert.
pub const ASSISTANT_EVENTS_KEPT: u32 = 1_000;
/// Upper bound for the `limit` of `list_assistant_events`.
pub const ASSISTANT_EVENTS_LIST_MAX: u32 = 500;
const ASSISTANT_MESSAGE_MAX_CHARS: usize = 500;
const ASSISTANT_TEXT_MAX_CHARS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantEventKind {
    /// A preview was prepared (`bytes_sent` = payload size that would be sent).
    Preview,
    /// The payload was sent (`bytes_sent` = payload size).
    Request,
    /// A validated plan was received (token counts set).
    Plan,
    /// A step was executed (`step_id`, `ok`, `message`).
    Step,
    /// A request or step failed (`message` = error summary).
    Error,
}

/// One row of the assistant audit log. Never holds the API key or payload content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEvent {
    /// Assigned by the database; ignored on insert.
    pub id: i64,
    /// Unix ms.
    pub at: u64,
    pub session_id: Option<String>,
    pub rule_id: Option<String>,
    pub model: Option<String>,
    pub kind: AssistantEventKind,
    pub bytes_sent: Option<u64>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub step_id: Option<String>,
    pub ok: Option<bool>,
    /// Short plain-text summary (clipped to 500 chars; API-key-like tokens removed).
    pub message: Option<String>,
}

impl AssistantEvent {
    pub fn new(kind: AssistantEventKind, at: u64) -> Self {
        Self {
            id: 0,
            at,
            session_id: None,
            rule_id: None,
            model: None,
            kind,
            bytes_sent: None,
            input_tokens: None,
            output_tokens: None,
            step_id: None,
            ok: None,
            message: None,
        }
    }
}

fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Defense in depth: drops anything that looks like an Anthropic key (`sk-ant-…`).
fn strip_secrets(s: &str) -> String {
    s.split_inclusive(char::is_whitespace)
        .map(|w| {
            if w.trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
                .starts_with("sk-ant-")
            {
                let ws: String = w.chars().filter(|c| c.is_whitespace()).collect();
                format!("<chiave rimossa>{ws}")
            } else {
                w.to_owned()
            }
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Invalid(AppError),
}

impl From<PersistenceError> for AppError {
    fn from(err: PersistenceError) -> Self {
        match err {
            PersistenceError::Invalid(app) => app,
            other => AppError::new(ErrorCode::Database, "local database operation failed")
                .with_detail(other.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens (or creates) the database file and applies pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Returns stored settings, or defaults when none are stored. A corrupted row is
    /// logged and replaced by defaults rather than blocking app startup.
    pub fn load_settings(&self) -> Result<AppSettings> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![SETTINGS_KEY],
                |r| r.get(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(AppSettings::default());
        };
        match serde_json::from_str::<AppSettings>(&raw) {
            Ok(s) if s.validate().is_ok() => Ok(s),
            Ok(_) | Err(_) => {
                tracing::warn!("stored settings invalid, using defaults");
                Ok(AppSettings::default())
            }
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        settings.validate().map_err(PersistenceError::Invalid)?;
        let json = serde_json::to_string(settings)?;
        let now = i64::try_from(models::time::now_unix_ms()).unwrap_or(i64::MAX);
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![SETTINGS_KEY, json, now],
        )?;
        Ok(())
    }

    /// Inserts the result, or replaces the stored one with the same `run_id`.
    pub fn save_benchmark_result(&self, r: &BenchmarkResult) -> Result<()> {
        if r.run_id.is_empty() {
            return Err(PersistenceError::Invalid(AppError::invalid_input(
                "benchmark result runId must not be empty",
            )));
        }
        let json = serde_json::to_string(r)?;
        self.conn.execute(
            "INSERT INTO benchmark_results (run_id, kind, state, methodology_version, started_at, json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(run_id) DO UPDATE SET
                kind = excluded.kind,
                state = excluded.state,
                methodology_version = excluded.methodology_version,
                started_at = excluded.started_at,
                json = excluded.json",
            params![
                r.run_id,
                enum_key(r.kind)?,
                enum_key(r.state)?,
                i64::from(r.methodology_version),
                to_i64(r.started_at),
                json
            ],
        )?;
        Ok(())
    }

    /// Newest first (`started_at` descending, then `run_id` descending), optionally
    /// filtered by kind. `limit` is clamped to `1..=BENCHMARK_LIST_LIMIT_MAX`. Corrupt
    /// rows are skipped with a warning (so fewer than `limit` rows may be returned).
    pub fn list_benchmark_results(
        &self,
        kind: Option<BenchmarkKind>,
        limit: u32,
    ) -> Result<Vec<BenchmarkResult>> {
        let limit = i64::from(limit.clamp(1, BENCHMARK_LIST_LIMIT_MAX));
        let kind = kind.map(enum_key).transpose()?;
        let mut stmt = self.conn.prepare(
            "SELECT run_id, json FROM benchmark_results
             WHERE ?1 IS NULL OR kind = ?1
             ORDER BY started_at DESC, run_id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![kind, limit], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, json)| parse_row("benchmark_results", &id, &json))
            .collect())
    }

    /// `None` when absent, or when the stored JSON is corrupt (logged).
    pub fn get_benchmark_result(&self, run_id: &str) -> Result<Option<BenchmarkResult>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM benchmark_results WHERE run_id = ?1",
                params![run_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.and_then(|j| parse_row("benchmark_results", run_id, &j)))
    }

    /// Inserts the report (or replaces the one with the same id), then keeps only the
    /// newest `DIAGNOSTIC_REPORTS_KEPT` reports, in one transaction.
    pub fn save_diagnostic_report(&self, r: &DiagnosticReport) -> Result<()> {
        if r.id.is_empty() {
            return Err(PersistenceError::Invalid(AppError::invalid_input(
                "diagnostic report id must not be empty",
            )));
        }
        let json = serde_json::to_string(r)?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO diagnostic_reports (id, generated_at, json) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET generated_at = excluded.generated_at, json = excluded.json",
            params![r.id, to_i64(r.generated_at), json],
        )?;
        tx.execute(
            "DELETE FROM diagnostic_reports WHERE id NOT IN (
                SELECT id FROM diagnostic_reports ORDER BY generated_at DESC, id DESC LIMIT ?1
             )",
            params![i64::from(DIAGNOSTIC_REPORTS_KEPT)],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The newest report that deserializes; corrupt rows are skipped with a warning.
    pub fn latest_diagnostic_report(&self) -> Result<Option<DiagnosticReport>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, json FROM diagnostic_reports ORDER BY generated_at DESC, id DESC",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let json: String = row.get(1)?;
            if let Some(r) = parse_row("diagnostic_reports", &id, &json) {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }

    /// Appends an event (its `id` is ignored) and prunes to the newest
    /// `ASSISTANT_EVENTS_KEPT`, in one transaction. Returns the new row id.
    pub fn insert_assistant_event(&self, e: &AssistantEvent) -> Result<i64> {
        let text = |v: &Option<String>| {
            v.as_deref()
                .map(|s| clip_chars(s, ASSISTANT_TEXT_MAX_CHARS))
        };
        let message = e
            .message
            .as_deref()
            .map(|m| clip_chars(&strip_secrets(m), ASSISTANT_MESSAGE_MAX_CHARS));
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO assistant_events
                (at, session_id, rule_id, model, kind, bytes_sent, input_tokens, output_tokens,
                 step_id, ok, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                to_i64(e.at),
                text(&e.session_id),
                text(&e.rule_id),
                text(&e.model),
                enum_key(e.kind)?,
                e.bytes_sent.map(to_i64),
                e.input_tokens.map(i64::from),
                e.output_tokens.map(i64::from),
                text(&e.step_id),
                e.ok,
                message,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "DELETE FROM assistant_events WHERE id NOT IN (
                SELECT id FROM assistant_events ORDER BY at DESC, id DESC LIMIT ?1
             )",
            params![i64::from(ASSISTANT_EVENTS_KEPT)],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Newest first, optionally for one session. `limit` is clamped to
    /// `1..=ASSISTANT_EVENTS_LIST_MAX`. Rows with an unknown kind are skipped.
    pub fn list_assistant_events(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<AssistantEvent>> {
        let limit = i64::from(limit.clamp(1, ASSISTANT_EVENTS_LIST_MAX));
        let mut stmt = self.conn.prepare(
            "SELECT id, at, session_id, rule_id, model, kind, bytes_sent, input_tokens,
                    output_tokens, step_id, ok, message
             FROM assistant_events
             WHERE ?1 IS NULL OR session_id = ?1
             ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![session_id, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<bool>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let to_u64 = |v: i64| u64::try_from(v).unwrap_or(0);
        let to_u32 = |v: i64| u32::try_from(v).unwrap_or(0);
        Ok(rows
            .into_iter()
            .filter_map(|(id, at, session_id, rule_id, model, kind, bytes, it, ot, step_id, ok, message)| {
                let kind = match serde_json::from_value(serde_json::Value::String(kind)) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!(table = "assistant_events", id, error = %e, "skipping corrupt row");
                        return None;
                    }
                };
                Some(AssistantEvent {
                    id,
                    at: to_u64(at),
                    session_id,
                    rule_id,
                    model,
                    kind,
                    bytes_sent: bytes.map(to_u64),
                    input_tokens: it.map(to_u32),
                    output_tokens: ot.map(to_u32),
                    step_id,
                    ok,
                    message,
                })
            })
            .collect())
    }

    /// A control-center document, or `None` if never saved.
    pub fn load_control_document(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT json FROM control_documents WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Inserts or replaces a control-center document. Keys are 1–64 ASCII characters.
    pub fn save_control_document(&self, key: &str, json: &str) -> Result<()> {
        validate_control_key(key)?;
        let now = i64::try_from(models::time::now_unix_ms()).unwrap_or(i64::MAX);
        self.conn.execute(
            "INSERT INTO control_documents (key, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![key, json, now],
        )?;
        Ok(())
    }

    pub fn delete_control_document(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM control_documents WHERE key = ?1", params![key])?;
        Ok(())
    }
}

fn validate_control_key(key: &str) -> Result<()> {
    let ok = !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(PersistenceError::Invalid(AppError::invalid_input(
            "invalid control document key",
        )))
    }
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// The serde name of a unit enum variant (e.g. `cpuSustained`), stored in indexed columns.
fn enum_key<T: serde::Serialize>(v: T) -> Result<String> {
    match serde_json::to_value(v)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(PersistenceError::Invalid(AppError::internal(format!(
            "expected a string enum key, got {other}"
        )))),
    }
}

fn parse_row<T: serde::de::DeserializeOwned>(table: &str, id: &str, json: &str) -> Option<T> {
    match serde_json::from_str(json) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(table, id, error = %e, "skipping corrupt row");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_documents_round_trip() {
        let db = Database::open_in_memory().expect("open");
        assert_eq!(
            db.load_control_document("performance.profiles")
                .expect("load"),
            None
        );
        db.save_control_document("performance.profiles", "[1]")
            .expect("save");
        db.save_control_document("performance.profiles", "[2]")
            .expect("replace");
        assert_eq!(
            db.load_control_document("performance.profiles")
                .expect("load")
                .as_deref(),
            Some("[2]")
        );
        db.delete_control_document("performance.profiles")
            .expect("delete");
        assert_eq!(
            db.load_control_document("performance.profiles")
                .expect("load"),
            None
        );
        assert!(db.save_control_document("bad key", "{}").is_err());
        assert!(db.save_control_document("", "{}").is_err());
    }
    use models::benchmark::BenchmarkState;
    use models::benchmark_run::{BenchmarkContext, BenchmarkRequest, PowerSource, ResultMetric};
    use models::component::ComponentKind;
    use models::diagnostics::{CoverageItem, CoverageStatus};
    use models::settings::ThemePreference;
    use models::{Metric, Unavailability, Unit};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pcdiag-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn bench(run_id: &str, kind: BenchmarkKind, started_at: u64) -> BenchmarkResult {
        BenchmarkResult {
            run_id: run_id.into(),
            kind,
            state: BenchmarkState::Completed,
            methodology_version: 1,
            app_version: "0.1.0".into(),
            build_profile: "release".into(),
            started_at,
            finished_at: started_at + 10_000,
            duration_ms: 10_000,
            request: BenchmarkRequest {
                kind,
                duration_s: Some(10),
                threads: None,
                storage_volume: None,
                storage_file_mib: None,
                storage_write_consent: false,
                gpu_id: None,
            },
            context: BenchmarkContext {
                os_version: Some("11".into()),
                cpu_brand: Some("Test CPU".into()),
                logical_processors: 8,
                total_memory_bytes: 16 << 30,
                power_source: PowerSource::Ac,
                power_plan: None,
                background_cpu_percent: Metric::unavailable(Unavailability::NotImplemented),
                available_memory_bytes: Metric::unavailable(Unavailability::NotImplemented),
                cpu_limit_mhz: Metric::unavailable(Unavailability::NotImplemented),
                cpu_max_mhz: Metric::unavailable(Unavailability::NotImplemented),
                storage_model: None,
                gpu_name: None,
                gpu_driver_version: None,
            },
            metrics: vec![ResultMetric {
                id: "throughput".into(),
                label: "Throughput".into(),
                value: 1000.0,
                unit: Unit::OperationsPerSecond,
                higher_is_better: true,
            }],
            series: vec![],
            warnings: vec![],
            notes: Vec::new(),
            safety_stop: None,
            error: None,
        }
    }

    fn report(id: &str, generated_at: u64) -> DiagnosticReport {
        DiagnosticReport {
            id: id.into(),
            generated_at,
            app_version: "0.1.0".into(),
            rules_version: 1,
            observed_window_s: 60,
            findings: vec![],
            coverage: vec![CoverageItem {
                component: ComponentKind::Cpu,
                status: CoverageStatus::Partial,
                note: "n".into(),
            }],
            limitations: vec![],
        }
    }

    #[test]
    fn migrations_are_valid() {
        assert!(MIGRATIONS.validate().is_ok());
    }

    #[test]
    fn fresh_db_is_at_latest_schema() {
        let db = Database::open_in_memory().expect("open");
        assert_eq!(
            db.schema_version().expect("version"),
            MIGRATIONS_SLICE.len() as u32
        );
        assert_eq!(db.schema_version().expect("version"), 4);
    }

    #[test]
    fn missing_settings_yield_defaults() {
        let db = Database::open_in_memory().expect("open");
        assert_eq!(db.load_settings().expect("load"), AppSettings::default());
    }

    #[test]
    fn settings_roundtrip_and_upsert() {
        let db = Database::open_in_memory().expect("open");
        let mut s = AppSettings {
            theme: ThemePreference::Dark,
            ..AppSettings::default()
        };
        db.save_settings(&s).expect("save");
        s.sampling_interval_ms = 2_000;
        db.save_settings(&s).expect("save again");
        assert_eq!(db.load_settings().expect("load"), s);
    }

    #[test]
    fn invalid_settings_are_rejected_with_input_error() {
        let db = Database::open_in_memory().expect("open");
        let s = AppSettings {
            sampling_interval_ms: 1,
            ..AppSettings::default()
        };
        let err: AppError = db.save_settings(&s).expect_err("must reject").into();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert_eq!(db.load_settings().expect("load"), AppSettings::default());
    }

    #[test]
    fn corrupted_row_falls_back_to_defaults() {
        let db = Database::open_in_memory().expect("open");
        db.conn
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('app', 'not json', 0)",
                [],
            )
            .expect("insert");
        assert_eq!(db.load_settings().expect("load"), AppSettings::default());
    }

    #[test]
    fn file_database_persists_across_reopen() {
        let dir = temp_dir("reopen");
        let path = dir.join("t.sqlite3");
        {
            let db = Database::open(&path).expect("open");
            let s = AppSettings {
                sidebar_collapsed: true,
                ..AppSettings::default()
            };
            db.save_settings(&s).expect("save");
            db.save_benchmark_result(&bench("r1", BenchmarkKind::CpuMultiCore, 5))
                .expect("save bench");
        }
        let db = Database::open(&path).expect("reopen");
        assert!(db.load_settings().expect("load").sidebar_collapsed);
        assert!(db.get_benchmark_result("r1").expect("get").is_some());
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_v1_database_file_and_keeps_settings() {
        let dir = temp_dir("v1");
        let path = dir.join("v1.sqlite3");
        {
            // Exactly the released v1 schema, as an old app version left it.
            let conn = Connection::open(&path).expect("open raw");
            conn.execute_batch(
                "CREATE TABLE settings (
                    key        TEXT PRIMARY KEY NOT NULL,
                    value      TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                ) STRICT;
                PRAGMA user_version = 1;",
            )
            .expect("v1 schema");
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('app', ?1, 1)",
                params![r#"{"theme":"light","samplingIntervalMs":3000}"#],
            )
            .expect("insert v1 settings");
        }
        let db = Database::open(&path).expect("migrate");
        assert_eq!(db.schema_version().expect("version"), 4);
        let s = db.load_settings().expect("load");
        assert_eq!(s.theme, ThemePreference::Light);
        assert_eq!(s.sampling_interval_ms, 3000);
        db.save_benchmark_result(&bench("r1", BenchmarkKind::CpuSustained, 1))
            .expect("v2 table usable");
        db.save_diagnostic_report(&report("d1", 1))
            .expect("v2 table usable");
        db.insert_assistant_event(&AssistantEvent::new(AssistantEventKind::Preview, 1))
            .expect("v3 table usable");
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn benchmark_roundtrip_and_upsert() {
        let db = Database::open_in_memory().expect("open");
        let mut r = bench("run-1", BenchmarkKind::CpuSingleCore, 100);
        db.save_benchmark_result(&r).expect("save");
        assert_eq!(
            db.get_benchmark_result("run-1").expect("get"),
            Some(r.clone())
        );
        r.state = BenchmarkState::Failed;
        r.metrics.clear();
        db.save_benchmark_result(&r).expect("upsert");
        assert_eq!(db.get_benchmark_result("run-1").expect("get"), Some(r));
        assert_eq!(
            db.list_benchmark_results(None, 10).expect("list").len(),
            1,
            "upsert must not duplicate"
        );
        assert_eq!(db.get_benchmark_result("missing").expect("get"), None);
    }

    #[test]
    fn benchmark_empty_run_id_is_rejected() {
        let db = Database::open_in_memory().expect("open");
        let err: AppError = db
            .save_benchmark_result(&bench("", BenchmarkKind::CpuSingleCore, 1))
            .expect_err("reject")
            .into();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn benchmark_list_is_newest_first_filtered_and_clamped() {
        let db = Database::open_in_memory().expect("open");
        for i in 0..5u64 {
            db.save_benchmark_result(&bench(
                &format!("a{i}"),
                BenchmarkKind::CpuMultiCore,
                i * 10,
            ))
            .expect("save");
        }
        db.save_benchmark_result(&bench("s", BenchmarkKind::StorageRandom, 25))
            .expect("save");
        let all = db.list_benchmark_results(None, 100).expect("list");
        let ids: Vec<_> = all.iter().map(|r| r.run_id.as_str()).collect();
        assert_eq!(ids, ["a4", "a3", "s", "a2", "a1", "a0"]);

        let cpu = db
            .list_benchmark_results(Some(BenchmarkKind::CpuMultiCore), 2)
            .expect("list");
        let ids: Vec<_> = cpu.iter().map(|r| r.run_id.as_str()).collect();
        assert_eq!(ids, ["a4", "a3"]);

        // limit 0 is clamped to 1.
        assert_eq!(db.list_benchmark_results(None, 0).expect("list").len(), 1);
        // limits above the max are clamped, not rejected.
        assert_eq!(
            db.list_benchmark_results(None, u32::MAX)
                .expect("list")
                .len(),
            6
        );
        assert!(
            db.list_benchmark_results(Some(BenchmarkKind::GpuCompute), 10)
                .expect("list")
                .is_empty()
        );
    }

    #[test]
    fn benchmark_list_clamps_at_500() {
        let db = Database::open_in_memory().expect("open");
        let tx = db.conn.unchecked_transaction().expect("tx");
        for i in 0..510u64 {
            db.save_benchmark_result(&bench(&format!("r{i:04}"), BenchmarkKind::CpuSingleCore, i))
                .expect("save");
        }
        tx.commit().expect("commit");
        assert_eq!(
            db.list_benchmark_results(None, 1000).expect("list").len(),
            BENCHMARK_LIST_LIMIT_MAX as usize
        );
    }

    #[test]
    fn corrupt_benchmark_rows_are_skipped() {
        let db = Database::open_in_memory().expect("open");
        db.save_benchmark_result(&bench("good", BenchmarkKind::CpuMultiCore, 1))
            .expect("save");
        db.conn
            .execute(
                "INSERT INTO benchmark_results (run_id, kind, state, methodology_version, started_at, json)
                 VALUES ('bad', 'cpuMultiCore', 'completed', 1, 2, '{not json')",
                [],
            )
            .expect("insert");
        let list = db.list_benchmark_results(None, 10).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].run_id, "good");
        assert_eq!(db.get_benchmark_result("bad").expect("get"), None);
    }

    #[test]
    fn diagnostic_report_roundtrip_and_latest() {
        let db = Database::open_in_memory().expect("open");
        assert_eq!(db.latest_diagnostic_report().expect("latest"), None);
        db.save_diagnostic_report(&report("d1", 10)).expect("save");
        db.save_diagnostic_report(&report("d2", 20)).expect("save");
        db.save_diagnostic_report(&report("d0", 5)).expect("save");
        assert_eq!(
            db.latest_diagnostic_report().expect("latest"),
            Some(report("d2", 20))
        );
    }

    #[test]
    fn diagnostic_reports_are_pruned_to_latest_20() {
        let db = Database::open_in_memory().expect("open");
        for i in 0..25u64 {
            db.save_diagnostic_report(&report(&format!("d{i:02}"), i))
                .expect("save");
        }
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM diagnostic_reports", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, i64::from(DIAGNOSTIC_REPORTS_KEPT));
        let oldest: i64 = db
            .conn
            .query_row(
                "SELECT MIN(generated_at) FROM diagnostic_reports",
                [],
                |r| r.get(0),
            )
            .expect("min");
        assert_eq!(oldest, 5);
        assert_eq!(
            db.latest_diagnostic_report().expect("latest").map(|r| r.id),
            Some("d24".into())
        );
    }

    #[test]
    fn corrupt_newest_report_falls_back_to_previous() {
        let db = Database::open_in_memory().expect("open");
        db.save_diagnostic_report(&report("ok", 1)).expect("save");
        db.conn
            .execute(
                "INSERT INTO diagnostic_reports (id, generated_at, json) VALUES ('bad', 99, 'x')",
                [],
            )
            .expect("insert");
        assert_eq!(
            db.latest_diagnostic_report().expect("latest").map(|r| r.id),
            Some("ok".into())
        );
    }

    fn event(kind: AssistantEventKind, at: u64, session: &str) -> AssistantEvent {
        AssistantEvent {
            session_id: Some(session.into()),
            rule_id: Some("storage.volume.low_space".into()),
            model: Some("claude-sonnet-5-5".into()),
            ..AssistantEvent::new(kind, at)
        }
    }

    #[test]
    fn migrates_v2_database_to_v3() {
        let mut conn = Connection::open_in_memory().expect("open");
        MIGRATIONS.to_version(&mut conn, 2).expect("v2");
        conn.execute(
            "INSERT INTO diagnostic_reports (id, generated_at, json) VALUES ('d', 1, '{}')",
            [],
        )
        .expect("v2 row");
        let db = Database::init(conn).expect("migrate");
        assert_eq!(db.schema_version().expect("version"), 4);
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM diagnostic_reports", [], |r| r.get(0))
            .expect("count");
        assert_eq!(n, 1, "v2 data kept");
        assert!(db.list_assistant_events(None, 10).expect("list").is_empty());
    }

    #[test]
    fn assistant_events_roundtrip_newest_first_and_filtered() {
        let db = Database::open_in_memory().expect("open");
        let mut req = event(AssistantEventKind::Request, 20, "s1");
        req.bytes_sent = Some(1234);
        let id = db.insert_assistant_event(&req).expect("insert");
        assert!(id > 0);
        let mut plan = event(AssistantEventKind::Plan, 30, "s1");
        plan.input_tokens = Some(1500);
        plan.output_tokens = Some(900);
        db.insert_assistant_event(&plan).expect("insert");
        let mut step = event(AssistantEventKind::Step, 40, "s1");
        step.step_id = Some("s1-1".into());
        step.ok = Some(true);
        step.message = Some("Pulizia disco avviata".into());
        db.insert_assistant_event(&step).expect("insert");
        db.insert_assistant_event(&event(AssistantEventKind::Error, 50, "s2"))
            .expect("insert");

        let all = db.list_assistant_events(None, 100).expect("list");
        let kinds: Vec<_> = all.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                AssistantEventKind::Error,
                AssistantEventKind::Step,
                AssistantEventKind::Plan,
                AssistantEventKind::Request
            ]
        );
        let s1 = db.list_assistant_events(Some("s1"), 100).expect("list");
        assert_eq!(s1.len(), 3);
        assert_eq!(s1[0].ok, Some(true));
        assert_eq!(s1[0].step_id.as_deref(), Some("s1-1"));
        assert_eq!(s1[1].input_tokens, Some(1500));
        assert_eq!(s1[1].output_tokens, Some(900));
        assert_eq!(s1[2].bytes_sent, Some(1234));
        assert_eq!(s1[2].id, id);
        assert_eq!(s1[2].ok, None);
        assert_eq!(db.list_assistant_events(None, 0).expect("list").len(), 1);
    }

    #[test]
    fn assistant_event_message_is_clipped_and_keys_removed() {
        let db = Database::open_in_memory().expect("open");
        let mut e = event(AssistantEventKind::Error, 1, "s");
        e.message = Some(format!(
            "chiave sk-ant-api03-SECRET rifiutata {}",
            "x".repeat(2000)
        ));
        db.insert_assistant_event(&e).expect("insert");
        let got = db.list_assistant_events(None, 1).expect("list");
        let m = got[0].message.as_deref().expect("message");
        assert!(!m.contains("sk-ant"), "{m}");
        assert!(!m.contains("SECRET"), "{m}");
        assert!(m.starts_with("chiave <chiave rimossa> rifiutata"));
        assert_eq!(m.chars().count(), ASSISTANT_MESSAGE_MAX_CHARS);
    }

    #[test]
    fn assistant_events_are_pruned_and_kind_is_checked() {
        let db = Database::open_in_memory().expect("open");
        for i in 0..(u64::from(ASSISTANT_EVENTS_KEPT) + 5) {
            db.insert_assistant_event(&event(AssistantEventKind::Preview, i, "s"))
                .expect("insert");
        }
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM assistant_events", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, i64::from(ASSISTANT_EVENTS_KEPT));
        let min: i64 = db
            .conn
            .query_row("SELECT MIN(at) FROM assistant_events", [], |r| r.get(0))
            .expect("min");
        assert_eq!(min, 5);
        assert!(
            db.conn
                .execute(
                    "INSERT INTO assistant_events (at, kind) VALUES (1, 'apiKey')",
                    [],
                )
                .is_err(),
            "CHECK constraint on kind"
        );
    }
}
