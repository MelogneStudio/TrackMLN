use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::models::AppTotal;
use crate::recurrence::parse_date;
use crate::AppState;

const MAX_RANGE_DAYS: i64 = 366;

const IDENTITY_EXPR: &str =
    "CASE WHEN COALESCE(TRIM(app_identity), '') != '' THEN app_identity ELSE LOWER(app_name) END";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DayTotal {
    pub date: String,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageRange {
    pub start_date: String,
    pub end_date: String,
    pub total_seconds: i64,
    pub average_per_day: f64,
    pub daily: Vec<DayTotal>,
    pub apps: Vec<AppTotal>,
    pub busiest_day: Option<DayTotal>,
    pub longest_session_seconds: i64,
    pub session_count: i64,
}

fn db_err(err: rusqlite::Error) -> String {
    err.to_string()
}

fn parse_range(start: &str, end: &str) -> Result<(NaiveDate, NaiveDate), String> {
    let start = parse_date(start)?;
    let end = parse_date(end)?;
    if end < start {
        return Err("End date is before the start date".into());
    }
    if (end - start).num_days() >= MAX_RANGE_DAYS {
        return Err(format!("Range is limited to {MAX_RANGE_DAYS} days"));
    }
    Ok((start, end))
}

/// Per-day totals for every day in `[start, end]` (zero-filled). With `identity`
/// set only that app is counted, otherwise everything except idle/unknown.
fn day_totals(
    connection: &Connection,
    start: NaiveDate,
    end: NaiveDate,
    identity: Option<&str>,
) -> Result<Vec<DayTotal>, String> {
    let sql = format!(
        "SELECT date(start, 'unixepoch', 'localtime') AS day,
                COALESCE(SUM(end - start), 0)
         FROM sessions
         WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
           AND ((?3 IS NULL AND {IDENTITY_EXPR} NOT IN ('idle', 'unknown'))
                OR {IDENTITY_EXPR} = ?3)
         GROUP BY day"
    );

    let mut statement = connection.prepare(&sql).map_err(db_err)?;
    let mut by_day: HashMap<String, i64> = HashMap::new();
    let rows = statement
        .query_map(params![start.to_string(), end.to_string(), identity], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(db_err)?;
    for row in rows {
        let (day, total) = row.map_err(db_err)?;
        by_day.insert(day, total);
    }

    let mut days = Vec::new();
    let mut day = start;
    while day <= end {
        let key = day.to_string();
        days.push(DayTotal {
            total: by_day.get(&key).copied().unwrap_or(0),
            date: key,
        });
        day += Duration::days(1);
    }
    Ok(days)
}

pub fn usage_range(connection: &Connection, start: &str, end: &str) -> Result<UsageRange, String> {
    let (start, end) = parse_range(start, end)?;
    let daily = day_totals(connection, start, end, None)?;
    let total_seconds: i64 = daily.iter().map(|day| day.total).sum();
    let busiest_day = daily
        .iter()
        .filter(|day| day.total > 0)
        .max_by_key(|day| day.total)
        .cloned();

    let apps_sql = format!(
        "SELECT {IDENTITY_EXPR} AS app_identity,
                MIN(app_name) AS app_name,
                COALESCE(SUM(end - start), 0) AS total
         FROM sessions
         WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
           AND {IDENTITY_EXPR} NOT IN ('idle', 'unknown')
         GROUP BY app_identity
         ORDER BY total DESC, app_identity ASC"
    );
    let mut statement = connection.prepare(&apps_sql).map_err(db_err)?;
    let apps = statement
        .query_map(params![start.to_string(), end.to_string()], |row| {
            Ok(AppTotal {
                app_identity: row.get(0)?,
                app_name: row.get(1)?,
                total: row.get(2)?,
            })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    let sessions_sql = format!(
        "SELECT COALESCE(MAX(end - start), 0), COUNT(*)
         FROM sessions
         WHERE date(start, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
           AND {IDENTITY_EXPR} NOT IN ('idle', 'unknown')"
    );
    let (longest_session_seconds, session_count) = connection
        .query_row(
            &sessions_sql,
            params![start.to_string(), end.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(db_err)?;

    let day_count = daily.len().max(1) as f64;
    Ok(UsageRange {
        start_date: start.to_string(),
        end_date: end.to_string(),
        total_seconds,
        average_per_day: total_seconds as f64 / day_count,
        daily,
        apps,
        busiest_day,
        longest_session_seconds,
        session_count,
    })
}

pub fn app_history(
    connection: &Connection,
    app_identity: &str,
    start: &str,
    end: &str,
) -> Result<Vec<DayTotal>, String> {
    let (start, end) = parse_range(start, end)?;
    day_totals(connection, start, end, Some(app_identity))
}

#[tauri::command]
pub fn get_usage_range(
    state: State<AppState>,
    start_date: String,
    end_date: String,
) -> Result<UsageRange, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    let mut range = usage_range(&connection, &start_date, &end_date)?;
    drop(connection);

    let mut resolver = state
        .app_name_resolver
        .lock()
        .map_err(|err| err.to_string())?;
    range.apps = range
        .apps
        .into_iter()
        .map(|app| crate::commands::present_app_total(&mut resolver, app))
        .collect();
    Ok(range)
}

#[tauri::command]
pub fn get_app_history(
    state: State<AppState>,
    app_identity: String,
    start_date: String,
    end_date: String,
) -> Result<Vec<DayTotal>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    app_history(&connection, &app_identity, &start_date, &end_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_schema, local_date_to_unix, log_session};
    use chrono::{DateTime, Local};

    fn at(date: NaiveDate, hour: u32, minute: u32) -> DateTime<Local> {
        DateTime::from_timestamp(local_date_to_unix(date, hour, minute, 0), 0)
            .unwrap()
            .with_timezone(&Local)
    }

    #[test]
    fn zero_fills_days_and_summarises_the_range() {
        let connection = Connection::open_in_memory().unwrap();
        init_schema(&connection).unwrap();

        let today = Local::now().date_naive();
        let two_days_ago = today - Duration::days(2);

        log_session(&connection, Some("chrome"), None, "Chrome", at(two_days_ago, 9, 0), at(two_days_ago, 10, 0)).unwrap();
        log_session(&connection, Some("code"), None, "Code", at(today, 9, 0), at(today, 9, 30)).unwrap();
        log_session(&connection, Some("idle"), None, "Idle", at(today, 12, 0), at(today, 13, 0)).unwrap();

        let range = usage_range(&connection, &two_days_ago.to_string(), &today.to_string()).unwrap();

        assert_eq!(range.daily.len(), 3);
        assert_eq!(range.daily[1].total, 0);
        assert_eq!(range.total_seconds, 3_600 + 1_800);
        assert_eq!(range.longest_session_seconds, 3_600);
        assert_eq!(range.session_count, 2);
        assert_eq!(range.apps.len(), 2);
        assert_eq!(range.busiest_day.unwrap().date, two_days_ago.to_string());

        let history = app_history(&connection, "code", &two_days_ago.to_string(), &today.to_string()).unwrap();
        assert_eq!(history.iter().map(|day| day.total).collect::<Vec<_>>(), vec![0, 0, 1_800]);
    }

    #[test]
    fn rejects_backwards_and_oversized_ranges() {
        assert!(parse_range("2026-02-01", "2026-01-01").is_err());
        assert!(parse_range("2024-01-01", "2026-01-01").is_err());
    }
}
