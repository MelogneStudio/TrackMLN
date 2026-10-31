use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Local, LocalResult, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{AppTotal, Goal, GoalCandidate, GoalDraft, HourlyData, WeekData, WeekDay};

pub const TOTAL_GOAL_TARGET: &str = "__total__";

pub type SharedDb = Arc<Mutex<Connection>>;

pub fn default_db_path(base_dir: impl AsRef<Path>) -> PathBuf {
    base_dir.as_ref().join("trackmln.db")
}

pub fn open_shared_database(path: impl AsRef<Path>) -> Result<SharedDb, String> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let connection = Connection::open(path).map_err(|err| err.to_string())?;
    // WAL + NORMAL sync survives a crash or power cut without losing committed
    // rows, and the busy timeout stops brief lock contention from erroring out.
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;",
        )
        .map_err(|err| err.to_string())?;
    init_schema(&connection).map_err(|err| err.to_string())?;
    Ok(Arc::new(Mutex::new(connection)))
}

pub fn init_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_identity TEXT,
            exe_name TEXT,
            app_name TEXT NOT NULL,
            start INTEGER NOT NULL,
            end INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS goals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name TEXT NOT NULL UNIQUE,
            daily_limit INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_start ON sessions(start);
        CREATE INDEX IF NOT EXISTS idx_sessions_app_name ON sessions(app_name);
        ",
    )?;

    ensure_column(connection, "sessions", "app_identity", "TEXT")?;
    ensure_column(connection, "sessions", "exe_name", "TEXT")?;
    ensure_column(connection, "goals", "target_kind", "TEXT NOT NULL DEFAULT 'app'")?;
    ensure_column(connection, "goals", "warn_seconds", "INTEGER")?;
    ensure_column(connection, "goals", "annoy_seconds", "INTEGER")?;
    crate::todos::init_schema(connection)?;
    Ok(())
}

/// Rows for the same app that touch within this many seconds are merged, so the
/// periodic heartbeat flush does not litter the table with one-minute rows.
const MERGE_GAP_SECONDS: i64 = 2;

/// Writes a usage session. Sessions that cross local midnight are split so each
/// stored row belongs to exactly one calendar day, which keeps every per-day
/// query (`date(start, 'localtime')`) correct.
pub fn log_session(
    connection: &Connection,
    app_identity: Option<&str>,
    exe_name: Option<&str>,
    app_name: &str,
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> rusqlite::Result<()> {
    let end_ts = end.timestamp();
    let mut cursor = start.timestamp();
    if end_ts - cursor < 1 {
        return Ok(());
    }

    while cursor < end_ts {
        let segment_end = end_ts.min(next_local_midnight(cursor));
        write_segment(connection, app_identity, exe_name, app_name, cursor, segment_end)?;
        cursor = segment_end;
    }
    Ok(())
}

fn write_segment(
    connection: &Connection,
    app_identity: Option<&str>,
    exe_name: Option<&str>,
    app_name: &str,
    start: i64,
    end: i64,
) -> rusqlite::Result<()> {
    let last = connection
        .query_row(
            "SELECT id, app_identity, app_name, end FROM sessions ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;

    if let Some((id, last_identity, last_name, last_end)) = last {
        let same_app = match (app_identity, last_identity.as_deref()) {
            (Some(current), Some(previous)) => current == previous,
            (None, None) => last_name == app_name,
            _ => false,
        };
        let touching = start >= last_end && start - last_end <= MERGE_GAP_SECONDS;
        // Never extend a row across midnight.
        let same_day = next_local_midnight(last_end - 1) >= end;

        if same_app && touching && same_day {
            connection.execute("UPDATE sessions SET end = ?1 WHERE id = ?2", params![end, id])?;
            return Ok(());
        }
    }

    connection.execute(
        "INSERT INTO sessions (app_identity, exe_name, app_name, start, end) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![app_identity, exe_name, app_name, start, end],
    )?;
    Ok(())
}

/// Unix timestamp of the first local midnight strictly after `timestamp`.
pub fn next_local_midnight(timestamp: i64) -> i64 {
    let fallback = timestamp + 86_400;
    let Some(utc) = DateTime::from_timestamp(timestamp, 0) else {
        return fallback;
    };
    let Some(next_day) = utc.with_timezone(&Local).date_naive().succ_opt() else {
        return fallback;
    };
    let naive = next_day.and_hms_opt(0, 0, 0).expect("midnight is a valid time");
    let boundary = match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp(),
        LocalResult::Ambiguous(dt, _) => dt.timestamp(),
        LocalResult::None => fallback,
    };
    if boundary > timestamp {
        boundary
    } else {
        fallback
    }
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;

    while let Some(row) = rows.next()? {
        let existing: String = row.get(1)?;
        if existing == column {
            return Ok(());
        }
    }

    connection.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

pub fn get_today_totals(connection: &Connection) -> rusqlite::Result<Vec<AppTotal>> {
    let mut statement = connection.prepare(
        "
        SELECT CASE
                   WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                   ELSE LOWER(app_name)
               END AS app_identity,
               MIN(app_name) AS app_name,
               COALESCE(SUM(end - start), 0) AS total
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') = date('now', 'localtime')
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        GROUP BY app_identity
        ORDER BY total DESC, app_identity ASC
        ",
    )?;

    let totals = statement
        .query_map([], |row| {
            Ok(AppTotal {
                app_identity: row.get(0)?,
                app_name: row.get(1)?,
                total: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(totals)
}

pub fn get_hourly_today(connection: &Connection) -> rusqlite::Result<Vec<HourlyData>> {
    let mut buckets: Vec<HourlyData> = (0..24)
        .map(|hour| HourlyData { hour, total: 0 })
        .collect();

    let today = Local::now().date_naive();
    let day_start = local_date_to_unix(today, 0, 0, 0);
    let day_end = local_date_to_unix(today, 23, 59, 59);

    let mut statement = connection.prepare(
        "
        SELECT start, end FROM sessions
        WHERE end >= ?1 AND start <= ?2
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        ",
    )?;

    for row in statement.query_map(params![day_start, day_end], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (raw_start, raw_end) = row?;

        // clamp to today's boundaries
        let start = raw_start.max(day_start);
        let end = raw_end.min(day_end + 1);

        // split across hour buckets
        let start_hour = ((start - day_start) / 3600) as usize;
        let end_hour = ((end - day_start - 1).max(0) / 3600) as usize;

        for hour in start_hour..=end_hour {
            if hour >= 24 { break; }
            let bucket_start = day_start + (hour as i64 * 3600);
            let bucket_end = bucket_start + 3600;
            let overlap = end.min(bucket_end) - start.max(bucket_start);
            if overlap > 0 {
                buckets[hour].total += overlap;
            }
        }
    }

    Ok(buckets)
}

pub fn get_goals(connection: &Connection) -> rusqlite::Result<Vec<Goal>> {
    let mut statement = connection.prepare(
        "
        SELECT id,
               target_kind,
               app_name,
               warn_seconds,
               annoy_seconds,
               daily_limit
        FROM goals
        ORDER BY target_kind ASC, app_name COLLATE NOCASE ASC
        ",
    )?;

    let goals = statement
        .query_map([], |row| {
            let target_kind: String = row.get(1)?;
            let target_value: String = row.get(2)?;
            let warn_seconds: Option<i64> = row.get(3)?;
            let annoy_seconds: Option<i64> = row.get(4)?;
            let legacy_limit: i64 = row.get(5)?;
            Ok(Goal {
                id: row.get(0)?,
                label: target_value.clone(),
                target_kind,
                target_value,
                warn_seconds,
                annoy_seconds: annoy_seconds.or(Some(legacy_limit).filter(|value| *value > 0)),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(goals)
}

pub fn upsert_goal(connection: &Connection, draft: &GoalDraft) -> rusqlite::Result<()> {
    let warn_seconds = draft.warn_seconds.filter(|value| *value > 0);
    let annoy_seconds = draft.annoy_seconds.filter(|value| *value > 0);
    let legacy_limit = annoy_seconds.unwrap_or(0);

    match draft.id {
        Some(id) => {
            connection.execute(
                "
                UPDATE goals
                SET target_kind = ?1,
                    app_name = ?2,
                    warn_seconds = ?3,
                    annoy_seconds = ?4,
                    daily_limit = ?5
                WHERE id = ?6
                ",
                params![
                    draft.target_kind,
                    draft.target_value,
                    warn_seconds,
                    annoy_seconds,
                    legacy_limit,
                    id
                ],
            )?;
        }
        None => {
            connection.execute(
                "
                INSERT INTO goals (target_kind, app_name, warn_seconds, annoy_seconds, daily_limit)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(app_name) DO UPDATE SET
                    target_kind = excluded.target_kind,
                    warn_seconds = excluded.warn_seconds,
                    annoy_seconds = excluded.annoy_seconds,
                    daily_limit = excluded.daily_limit
                ",
                params![
                    draft.target_kind,
                    draft.target_value,
                    warn_seconds,
                    annoy_seconds,
                    legacy_limit
                ],
            )?;
        }
    }

    Ok(())
}

pub fn delete_goal(connection: &Connection, id: i64) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM goals WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_goal_candidates(connection: &Connection) -> rusqlite::Result<Vec<GoalCandidate>> {
    let mut statement = connection.prepare(
        "
        SELECT CASE
                   WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                   ELSE LOWER(app_name)
               END AS app_identity,
               MIN(app_name) AS app_name,
               COALESCE(SUM(end - start), 0) AS total
        FROM sessions
        WHERE CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        GROUP BY app_identity
        ORDER BY total DESC, app_identity ASC
        ",
    )?;

    let candidates = statement
        .query_map([], |row| {
            Ok(GoalCandidate {
                app_identity: row.get(0)?,
                app_name: row.get(1)?,
                total_seconds: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(candidates)
}

pub fn get_today_total_for_identity(connection: &Connection, app_identity: &str) -> rusqlite::Result<i64> {
    connection.query_row(
        "
        SELECT COALESCE(SUM(end - start), 0)
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') = date('now', 'localtime')
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END = ?1
        ",
        params![app_identity],
        |row| row.get(0),
    )
}

pub fn get_today_total(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row(
        "
        SELECT COALESCE(SUM(end - start), 0)
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') = date('now', 'localtime')
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        ",
        [],
        |row| row.get(0),
    )
}

pub fn get_week_dashboard(connection: &Connection) -> rusqlite::Result<WeekData> {
    let today = Local::now().date_naive();
    let week_days: Vec<NaiveDate> = (0..7)
        .map(|offset| today - Duration::days((6 - offset) as i64))
        .collect();
    let week_start = week_days.first().copied().unwrap();
    let week_end = week_days.last().copied().unwrap();
    let last_week_start = week_start - Duration::days(7);
    let last_week_end = week_end - Duration::days(7);

    let mut daily_totals = HashMap::<String, i64>::new();
    let mut daily_apps = HashMap::<String, Vec<AppTotal>>::new();

    let mut day_totals_statement = connection.prepare(
        "
        SELECT date(start, 'unixepoch', 'localtime') AS day,
               COALESCE(SUM(end - start), 0) AS total
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        GROUP BY day
        ORDER BY day ASC
        ",
    )?;

    for row in day_totals_statement.query_map(
        params![week_start.to_string(), week_end.to_string()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )? {
        let (day, total) = row?;
        daily_totals.insert(day, total);
    }

    let mut day_apps_statement = connection.prepare(
        "
        SELECT date(start, 'unixepoch', 'localtime') AS day,
               CASE
                   WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                   ELSE LOWER(app_name)
               END AS app_identity,
               MIN(app_name) AS app_name,
               COALESCE(SUM(end - start), 0) AS total
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        GROUP BY day, app_identity
        ORDER BY day ASC, total DESC, app_identity ASC
        ",
    )?;

    for row in day_apps_statement.query_map(
        params![week_start.to_string(), week_end.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                AppTotal {
                    app_identity: row.get(1)?,
                    app_name: row.get(2)?,
                    total: row.get(3)?,
                },
            ))
        },
    )? {
        let (day, app) = row?;
        daily_apps.entry(day).or_default().push(app);
    }

    let mut week_apps_statement = connection.prepare(
        "
        SELECT CASE
                   WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                   ELSE LOWER(app_name)
               END AS app_identity,
               MIN(app_name) AS app_name,
               COALESCE(SUM(end - start), 0) AS total
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        GROUP BY app_identity
        ORDER BY total DESC, app_identity ASC
        ",
    )?;

    let week_apps: Vec<AppTotal> = week_apps_statement
        .query_map(
            params![week_start.to_string(), week_end.to_string()],
            |row| {
                Ok(AppTotal {
                    app_identity: row.get(0)?,
                    app_name: row.get(1)?,
                    total: row.get(2)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let previous_week_total: i64 = connection.query_row(
        "
        SELECT COALESCE(SUM(end - start), 0)
        FROM sessions
        WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
          AND CASE
                  WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity
                  ELSE LOWER(app_name)
              END NOT IN ('idle', 'unknown')
        ",
        params![last_week_start.to_string(), last_week_end.to_string()],
        |row| row.get(0),
    )?;

    let days: Vec<WeekDay> = week_days
        .iter()
        .map(|day| {
            let key = day.to_string();
            WeekDay {
                date: key.clone(),
                total: *daily_totals.get(&key).unwrap_or(&0),
                apps: daily_apps.remove(&key).unwrap_or_default(),
            }
        })
        .collect();

    let week_total = days.iter().map(|day| day.total).sum::<i64>();

    Ok(WeekData {
        days,
        apps: week_apps.clone(),
        week_total,
        current_week_average: week_total as f64 / 7.0,
        previous_week_average: previous_week_total as f64 / 7.0,
        top_app: week_apps.first().cloned(),
    })
}

#[cfg(test)]
pub fn unix_to_local(timestamp: i64) -> Option<DateTime<Local>> {
    Some(DateTime::from_timestamp(timestamp, 0)?.with_timezone(&Local))
}

pub fn local_date_to_unix(date: NaiveDate, hour: u32, minute: u32, second: u32) -> i64 {
    let naive = date
        .and_hms_opt(hour, minute, second)
        .expect("valid test timestamp");
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp(),
        LocalResult::Ambiguous(dt, _) => dt.timestamp(),
        LocalResult::None => panic!("invalid local datetime for test data"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory db");
        init_schema(&connection).expect("schema init");
        connection
    }

    #[test]
    fn ignores_subsecond_sessions() {
        let connection = test_connection();
        let start = Local::now();
        let end = start + Duration::milliseconds(500);

        log_session(&connection, None, None, "Chrome", start, end).expect("session write");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .expect("count rows");
        assert_eq!(count, 0);
    }

    #[test]
    fn splits_sessions_that_cross_midnight() {
        let connection = test_connection();
        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let start = unix_to_local(local_date_to_unix(yesterday, 23, 0, 0)).unwrap();
        let end = unix_to_local(local_date_to_unix(today, 1, 0, 0)).unwrap();

        log_session(&connection, None, None, "Game", start, end).expect("session write");

        let mut statement = connection
            .prepare(
                "SELECT date(start, 'unixepoch', 'localtime'), SUM(end - start)
                 FROM sessions GROUP BY 1 ORDER BY 1",
            )
            .expect("prepare");
        let rows: Vec<(String, i64)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, yesterday.to_string());
        assert_eq!(rows[1].0, today.to_string());
        assert_eq!(rows[0].1 + rows[1].1, end.timestamp() - start.timestamp());
    }

    #[test]
    fn merges_back_to_back_sessions_for_the_same_app() {
        let connection = test_connection();
        let today = Local::now().date_naive();
        let t = |h, m, s| unix_to_local(local_date_to_unix(today, h, m, s)).unwrap();

        log_session(&connection, Some("chrome"), None, "Chrome", t(9, 0, 0), t(9, 1, 0)).unwrap();
        log_session(&connection, Some("chrome"), None, "Chrome", t(9, 1, 0), t(9, 2, 0)).unwrap();
        log_session(&connection, Some("code"), None, "Code", t(9, 2, 0), t(9, 3, 0)).unwrap();
        log_session(&connection, Some("chrome"), None, "Chrome", t(9, 3, 0), t(9, 4, 0)).unwrap();

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);

        let first_end: i64 = connection
            .query_row("SELECT end FROM sessions ORDER BY id ASC LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(first_end, t(9, 2, 0).timestamp());
    }

    #[test]
    fn computes_today_totals_and_hourly_buckets() {
        let connection = test_connection();
        let today = Local::now().date_naive();
        let start = unix_to_local(local_date_to_unix(today, 9, 0, 0)).unwrap();
        let end = unix_to_local(local_date_to_unix(today, 9, 30, 0)).unwrap();
        let start_2 = unix_to_local(local_date_to_unix(today, 10, 0, 0)).unwrap();
        let end_2 = unix_to_local(local_date_to_unix(today, 10, 15, 0)).unwrap();

        log_session(&connection, None, None, "Chrome", start, end).expect("session write");
        log_session(&connection, None, None, "Chrome", start_2, end_2).expect("session write");

        let totals = get_today_totals(&connection).expect("today totals");
        assert_eq!(
            totals,
            vec![AppTotal {
                app_identity: "chrome".into(),
                app_name: "Chrome".into(),
                total: 2_700
            }]
        );

        let hourly = get_hourly_today(&connection).expect("hourly totals");
        assert_eq!(hourly[9].total, 1_800);
        assert_eq!(hourly[10].total, 900);
    }

    #[test]
    fn computes_week_dashboard() {
        let connection = test_connection();
        let today = Local::now().date_naive();
        let this_week_day = today - Duration::days(1);
        let last_week_day = today - Duration::days(8);

        let this_start = unix_to_local(local_date_to_unix(this_week_day, 14, 0, 0)).unwrap();
        let this_end = unix_to_local(local_date_to_unix(this_week_day, 15, 0, 0)).unwrap();
        let last_start = unix_to_local(local_date_to_unix(last_week_day, 14, 0, 0)).unwrap();
        let last_end = unix_to_local(local_date_to_unix(last_week_day, 14, 30, 0)).unwrap();

        log_session(&connection, None, None, "VS Code", this_start, this_end).expect("session write");
        log_session(&connection, None, None, "VS Code", last_start, last_end).expect("session write");

        let dashboard = get_week_dashboard(&connection).expect("week dashboard");
        assert_eq!(dashboard.week_total, 3_600);
        assert_eq!(dashboard.previous_week_average, 1800.0 / 7.0);
        assert_eq!(
            dashboard.top_app,
            Some(AppTotal {
                app_identity: "vs code".into(),
                app_name: "VS Code".into(),
                total: 3_600
            })
        );
    }
}
