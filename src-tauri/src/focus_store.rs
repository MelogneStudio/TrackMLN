use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::SharedDb;
use crate::focus::{FocusEngine, FocusEvent, FocusLog, FocusPhase, FocusPreset, FocusSnapshot};
use crate::recurrence::parse_date;
use crate::AppState;

const MAX_STATS_DAYS: i64 = 366;
const PRESET_COLUMNS: &str = "id, name, focus_minutes, short_break_minutes, long_break_minutes, \
                              sessions_until_long_break, total_sessions, auto_start_breaks, \
                              auto_start_focus, is_default";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DayFocus {
    pub date: String,
    pub seconds: i64,
    pub completed_sessions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoFocus {
    pub todo_id: i64,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusStats {
    pub start_date: String,
    pub end_date: String,
    pub total_focus_seconds: i64,
    pub completed_sessions: i64,
    pub abandoned_sessions: i64,
    pub daily: Vec<DayFocus>,
    pub by_todo: Vec<TodoFocus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseEndedPayload {
    ended: FocusPhase,
    next: Option<FocusPhase>,
    completed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FinishedPayload {
    focus_sessions: i64,
}

fn db_err(err: rusqlite::Error) -> String {
    err.to_string()
}

fn builtin_presets() -> Vec<FocusPreset> {
    let make = |name: &str, focus, short, long, until_long, total, default| FocusPreset {
        id: None,
        name: name.into(),
        focus_minutes: focus,
        short_break_minutes: short,
        long_break_minutes: long,
        sessions_until_long_break: until_long,
        total_sessions: total,
        auto_start_breaks: true,
        auto_start_focus: false,
        is_default: default,
    };

    vec![
        make("Pomodoro", 25, 5, 15, 4, 8, true),
        make("Deep work", 50, 10, 30, 2, 4, false),
        make("Quick sprint", 15, 3, 10, 4, 4, false),
    ]
}

pub fn init_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS focus_presets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            focus_minutes INTEGER NOT NULL,
            short_break_minutes INTEGER NOT NULL,
            long_break_minutes INTEGER NOT NULL,
            sessions_until_long_break INTEGER NOT NULL,
            total_sessions INTEGER NOT NULL,
            auto_start_breaks INTEGER NOT NULL DEFAULT 1,
            auto_start_focus INTEGER NOT NULL DEFAULT 0,
            is_default INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS focus_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at INTEGER NOT NULL,
            ended_at INTEGER NOT NULL,
            planned_seconds INTEGER NOT NULL,
            actual_seconds INTEGER NOT NULL,
            completed INTEGER NOT NULL,
            preset_id INTEGER,
            preset_name TEXT NOT NULL DEFAULT '',
            todo_id INTEGER,
            entry_id INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_focus_sessions_started ON focus_sessions(started_at);
        CREATE INDEX IF NOT EXISTS idx_focus_sessions_todo ON focus_sessions(todo_id);
        ",
    )?;

    let preset_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM focus_presets", [], |row| row.get(0))?;
    if preset_count == 0 {
        for preset in builtin_presets() {
            insert_preset(connection, &preset).map_err(rusqlite::Error::InvalidParameterName)?;
        }
    }
    Ok(())
}

fn row_to_preset(row: &Row) -> rusqlite::Result<FocusPreset> {
    let auto_breaks: i64 = row.get(7)?;
    let auto_focus: i64 = row.get(8)?;
    let is_default: i64 = row.get(9)?;
    Ok(FocusPreset {
        id: row.get(0)?,
        name: row.get(1)?,
        focus_minutes: row.get(2)?,
        short_break_minutes: row.get(3)?,
        long_break_minutes: row.get(4)?,
        sessions_until_long_break: row.get(5)?,
        total_sessions: row.get(6)?,
        auto_start_breaks: auto_breaks != 0,
        auto_start_focus: auto_focus != 0,
        is_default: is_default != 0,
    })
}

pub fn list_presets(connection: &Connection) -> Result<Vec<FocusPreset>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {PRESET_COLUMNS} FROM focus_presets ORDER BY is_default DESC, name COLLATE NOCASE, id"
        ))
        .map_err(db_err)?;
    let presets = statement
        .query_map([], row_to_preset)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    Ok(presets)
}

pub fn get_preset(connection: &Connection, id: i64) -> Result<FocusPreset, String> {
    connection
        .query_row(
            &format!("SELECT {PRESET_COLUMNS} FROM focus_presets WHERE id = ?1"),
            params![id],
            row_to_preset,
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| "Focus preset not found".to_string())
}

/// The preset marked default, else the first one, else the built-in Pomodoro.
pub fn default_preset(connection: &Connection) -> Result<FocusPreset, String> {
    let presets = list_presets(connection)?;
    Ok(presets
        .into_iter()
        .next()
        .unwrap_or_else(|| builtin_presets().remove(0)))
}

fn insert_preset(connection: &Connection, preset: &FocusPreset) -> Result<i64, String> {
    connection
        .execute(
            "INSERT INTO focus_presets
                (name, focus_minutes, short_break_minutes, long_break_minutes,
                 sessions_until_long_break, total_sessions, auto_start_breaks,
                 auto_start_focus, is_default, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                preset.name,
                preset.focus_minutes,
                preset.short_break_minutes,
                preset.long_break_minutes,
                preset.sessions_until_long_break,
                preset.total_sessions,
                preset.auto_start_breaks as i64,
                preset.auto_start_focus as i64,
                preset.is_default as i64,
                Local::now().timestamp()
            ],
        )
        .map_err(db_err)?;
    Ok(connection.last_insert_rowid())
}

pub fn save_preset(connection: &Connection, preset: FocusPreset) -> Result<FocusPreset, String> {
    let preset = preset.validated()?;

    let clash: Option<i64> = connection
        .query_row(
            "SELECT id FROM focus_presets WHERE name = ?1 COLLATE NOCASE AND id != ?2",
            params![preset.name, preset.id.unwrap_or(-1)],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    if clash.is_some() {
        return Err("A preset with that name already exists".into());
    }

    let id = match preset.id {
        Some(id) => {
            let changed = connection
                .execute(
                    "UPDATE focus_presets
                     SET name = ?1, focus_minutes = ?2, short_break_minutes = ?3,
                         long_break_minutes = ?4, sessions_until_long_break = ?5,
                         total_sessions = ?6, auto_start_breaks = ?7, auto_start_focus = ?8,
                         is_default = ?9
                     WHERE id = ?10",
                    params![
                        preset.name,
                        preset.focus_minutes,
                        preset.short_break_minutes,
                        preset.long_break_minutes,
                        preset.sessions_until_long_break,
                        preset.total_sessions,
                        preset.auto_start_breaks as i64,
                        preset.auto_start_focus as i64,
                        preset.is_default as i64,
                        id
                    ],
                )
                .map_err(db_err)?;
            if changed == 0 {
                return Err("Focus preset not found".into());
            }
            id
        }
        None => insert_preset(connection, &preset)?,
    };

    if preset.is_default {
        connection
            .execute("UPDATE focus_presets SET is_default = 0 WHERE id != ?1", params![id])
            .map_err(db_err)?;
    }
    get_preset(connection, id)
}

/// Deleting a preset also stops timetable blocks from pointing at it.
pub fn delete_preset(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM focus_presets WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if changed == 0 {
        return Err("Focus preset not found".into());
    }
    connection
        .execute(
            "UPDATE timetable_entries SET preset_id = NULL, auto_start = 0 WHERE preset_id = ?1",
            params![id],
        )
        .map_err(db_err)?;
    Ok(())
}

pub fn insert_log(connection: &Connection, log: &FocusLog) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO focus_sessions
                (started_at, ended_at, planned_seconds, actual_seconds, completed,
                 preset_id, preset_name, todo_id, entry_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                log.started_at,
                log.ended_at,
                log.planned_seconds,
                log.actual_seconds,
                log.completed as i64,
                log.preset_id,
                log.preset_name,
                log.todo_id,
                log.entry_id
            ],
        )
        .map_err(db_err)?;
    Ok(())
}

pub fn recent_logs(connection: &Connection, limit: i64) -> Result<Vec<FocusLog>, String> {
    let mut statement = connection
        .prepare(
            "SELECT started_at, ended_at, planned_seconds, actual_seconds, completed,
                    preset_id, preset_name, todo_id, entry_id
             FROM focus_sessions ORDER BY started_at DESC, id DESC LIMIT ?1",
        )
        .map_err(db_err)?;
    let logs = statement
        .query_map(params![limit.clamp(1, 500)], |row| {
            let completed: i64 = row.get(4)?;
            Ok(FocusLog {
                started_at: row.get(0)?,
                ended_at: row.get(1)?,
                planned_seconds: row.get(2)?,
                actual_seconds: row.get(3)?,
                completed: completed != 0,
                preset_id: row.get(5)?,
                preset_name: row.get(6)?,
                todo_id: row.get(7)?,
                entry_id: row.get(8)?,
            })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    Ok(logs)
}

pub fn focus_stats(connection: &Connection, start: &str, end: &str) -> Result<FocusStats, String> {
    let start: NaiveDate = parse_date(start)?;
    let end: NaiveDate = parse_date(end)?;
    if end < start {
        return Err("End date is before the start date".into());
    }
    if (end - start).num_days() >= MAX_STATS_DAYS {
        return Err(format!("Range is limited to {MAX_STATS_DAYS} days"));
    }
    let (start_key, end_key) = (start.to_string(), end.to_string());

    let mut statement = connection
        .prepare(
            "SELECT date(started_at, 'unixepoch', 'localtime'), SUM(actual_seconds), SUM(completed)
             FROM focus_sessions
             WHERE date(started_at, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
             GROUP BY date(started_at, 'unixepoch', 'localtime')",
        )
        .map_err(db_err)?;
    let mut by_day: HashMap<String, (i64, i64)> = HashMap::new();
    let rows = statement
        .query_map(params![start_key, end_key], |row| {
            Ok((row.get::<_, String>(0)?, (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)))
        })
        .map_err(db_err)?;
    for row in rows {
        let (day, values) = row.map_err(db_err)?;
        by_day.insert(day, values);
    }

    let mut daily = Vec::new();
    let mut day = start;
    while day <= end {
        let key = day.to_string();
        let (seconds, completed_sessions) = by_day.get(&key).copied().unwrap_or((0, 0));
        daily.push(DayFocus { date: key, seconds, completed_sessions });
        day += Duration::days(1);
    }

    let (total_focus_seconds, completed_sessions, all_sessions): (i64, i64, i64) = connection
        .query_row(
            "SELECT COALESCE(SUM(actual_seconds), 0), COALESCE(SUM(completed), 0), COUNT(*)
             FROM focus_sessions
             WHERE date(started_at, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2",
            params![start_key, end_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(db_err)?;

    let mut todo_statement = connection
        .prepare(
            "SELECT todo_id, SUM(actual_seconds) FROM focus_sessions
             WHERE todo_id IS NOT NULL
               AND date(started_at, 'unixepoch', 'localtime') BETWEEN ?1 AND ?2
             GROUP BY todo_id ORDER BY 2 DESC",
        )
        .map_err(db_err)?;
    let by_todo = todo_statement
        .query_map(params![start_key, end_key], |row| {
            Ok(TodoFocus { todo_id: row.get(0)?, seconds: row.get(1)? })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    Ok(FocusStats {
        start_date: start_key,
        end_date: end_key,
        total_focus_seconds,
        completed_sessions,
        abandoned_sessions: all_sessions - completed_sessions,
        daily,
        by_todo,
    })
}

// ── Engine glue (shared by the commands and the scheduler) ───────────────────

pub fn now_unix() -> i64 {
    Local::now().timestamp()
}

pub fn emit_snapshot(app: &AppHandle, engine: &Arc<Mutex<FocusEngine>>) -> Result<FocusSnapshot, String> {
    let snapshot = engine
        .lock()
        .map_err(|err| err.to_string())?
        .snapshot(now_unix());
    let _ = app.emit("focus-update", snapshot.clone());
    Ok(snapshot)
}

/// Stores finished focus phases and tells the UI what happened.
pub fn apply_events(
    app: &AppHandle,
    db: &SharedDb,
    engine: &Arc<Mutex<FocusEngine>>,
    events: Vec<FocusEvent>,
) {
    for event in &events {
        match event {
            FocusEvent::Logged(log) => {
                if let Ok(connection) = db.lock() {
                    if let Err(err) = insert_log(&connection, log) {
                        eprintln!("failed to store focus session: {err}");
                    }
                }
            }
            FocusEvent::PhaseEnded { ended, next, completed } => {
                let _ = app.emit(
                    "focus-phase-ended",
                    PhaseEndedPayload { ended: *ended, next: *next, completed: *completed },
                );
            }
            FocusEvent::Finished { focus_sessions } => {
                let _ = app.emit("focus-finished", FinishedPayload { focus_sessions: *focus_sessions });
            }
        }
    }

    if let Err(err) = emit_snapshot(app, engine) {
        eprintln!("failed to publish focus state: {err}");
    }
}

pub fn tick_engine(app: &AppHandle, db: &SharedDb, engine: &Arc<Mutex<FocusEngine>>) {
    let events = match engine.lock() {
        Ok(mut guard) => guard.tick(now_unix()),
        Err(_) => return,
    };
    if !events.is_empty() {
        apply_events(app, db, engine, events);
    }
}

pub fn start_session(
    app: &AppHandle,
    db: &SharedDb,
    engine: &Arc<Mutex<FocusEngine>>,
    preset: FocusPreset,
    todo_id: Option<i64>,
    entry_id: Option<i64>,
) -> Result<FocusSnapshot, String> {
    if let Some(id) = todo_id {
        let connection = db.lock().map_err(|err| err.to_string())?;
        crate::todos::get_todo(&connection, id)?;
    }

    engine
        .lock()
        .map_err(|err| err.to_string())?
        .start(preset, todo_id, entry_id, now_unix())?;
    emit_snapshot(app, engine)
}

fn control<F>(app: &AppHandle, state: &AppState, action: F) -> Result<FocusSnapshot, String>
where
    F: FnOnce(&mut FocusEngine, i64) -> Result<Vec<FocusEvent>, String>,
{
    let events = {
        let mut engine = state.focus.lock().map_err(|err| err.to_string())?;
        action(&mut *engine, now_unix())?
    };
    apply_events(app, &state.db, &state.focus, events);
    emit_snapshot(app, &state.focus)
}

#[tauri::command]
pub fn get_focus_presets(state: State<AppState>) -> Result<Vec<FocusPreset>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    list_presets(&connection)
}

#[tauri::command]
pub fn save_focus_preset(state: State<AppState>, preset: FocusPreset) -> Result<FocusPreset, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    save_preset(&connection, preset)
}

#[tauri::command]
pub fn delete_focus_preset(state: State<AppState>, preset_id: i64) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    delete_preset(&connection, preset_id)
}

#[tauri::command]
pub fn get_focus_state(app: AppHandle, state: State<AppState>) -> Result<FocusSnapshot, String> {
    tick_engine(&app, &state.db, &state.focus);
    let snapshot = state
        .focus
        .lock()
        .map_err(|err| err.to_string())?
        .snapshot(now_unix());
    Ok(snapshot)
}

/// Starts a run. The preset is, in order: `preset` (a one-off, unsaved
/// configuration), the saved preset `preset_id`, the preset of the timetable
/// block `entry_id`, or the default preset.
#[tauri::command]
pub fn start_focus(
    app: AppHandle,
    state: State<AppState>,
    preset_id: Option<i64>,
    preset: Option<FocusPreset>,
    todo_id: Option<i64>,
    entry_id: Option<i64>,
) -> Result<FocusSnapshot, String> {
    let chosen = {
        let connection = state.db.lock().map_err(|err| err.to_string())?;
        match (preset, preset_id) {
            (Some(custom), _) => custom,
            (None, Some(id)) => get_preset(&connection, id)?,
            (None, None) => {
                let from_block = match entry_id {
                    Some(id) => crate::timetable::get_entry(&connection, id)?.preset_id,
                    None => None,
                };
                match from_block {
                    Some(id) => get_preset(&connection, id)?,
                    None => default_preset(&connection)?,
                }
            }
        }
    };

    start_session(&app, &state.db, &state.focus, chosen, todo_id, entry_id)
}

#[tauri::command]
pub fn pause_focus(app: AppHandle, state: State<AppState>) -> Result<FocusSnapshot, String> {
    control(&app, &state, |engine, now| engine.pause(now).map(|_| Vec::new()))
}

/// Continues a paused phase, or starts the next phase when it is waiting.
#[tauri::command]
pub fn resume_focus(app: AppHandle, state: State<AppState>) -> Result<FocusSnapshot, String> {
    control(&app, &state, |engine, now| engine.resume(now).map(|_| Vec::new()))
}

#[tauri::command]
pub fn skip_focus_phase(app: AppHandle, state: State<AppState>) -> Result<FocusSnapshot, String> {
    control(&app, &state, |engine, now| engine.skip(now))
}

#[tauri::command]
pub fn stop_focus(app: AppHandle, state: State<AppState>) -> Result<FocusSnapshot, String> {
    control(&app, &state, |engine, now| Ok(engine.stop(now)))
}

#[tauri::command]
pub fn add_focus_time(app: AppHandle, state: State<AppState>, seconds: i64) -> Result<FocusSnapshot, String> {
    control(&app, &state, |engine, now| engine.add_time(now, seconds).map(|_| Vec::new()))
}

#[tauri::command]
pub fn get_focus_stats(
    state: State<AppState>,
    start_date: String,
    end_date: String,
) -> Result<FocusStats, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    focus_stats(&connection, &start_date, &end_date)
}

#[tauri::command]
pub fn get_focus_history(state: State<AppState>, limit: Option<i64>) -> Result<Vec<FocusLog>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    recent_logs(&connection, limit.unwrap_or(50))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        crate::todos::init_schema(&connection).unwrap();
        crate::timetable::init_schema(&connection).unwrap();
        init_schema(&connection).unwrap();
        connection
    }

    fn custom(name: &str) -> FocusPreset {
        FocusPreset {
            id: None,
            name: name.into(),
            focus_minutes: 30,
            short_break_minutes: 5,
            long_break_minutes: 20,
            sessions_until_long_break: 3,
            total_sessions: 6,
            auto_start_breaks: true,
            auto_start_focus: true,
            is_default: false,
        }
    }

    #[test]
    fn seeds_default_presets_once() {
        let connection = connection();
        let presets = list_presets(&connection).unwrap();
        assert_eq!(presets.len(), 3);
        assert_eq!(presets[0].name, "Pomodoro");
        assert!(presets[0].is_default);

        init_schema(&connection).unwrap();
        assert_eq!(list_presets(&connection).unwrap().len(), 3);
        assert_eq!(default_preset(&connection).unwrap().name, "Pomodoro");
    }

    #[test]
    fn saves_edits_and_deletes_presets() {
        let connection = connection();

        let created = save_preset(&connection, custom("Study")).unwrap();
        assert_eq!(created.focus_minutes, 30);
        assert!(created.auto_start_focus);

        let mut edit = created.clone();
        edit.focus_minutes = 40;
        edit.is_default = true;
        let edited = save_preset(&connection, edit).unwrap();
        assert_eq!(edited.focus_minutes, 40);
        assert_eq!(default_preset(&connection).unwrap().name, "Study");
        assert_eq!(list_presets(&connection).unwrap().iter().filter(|p| p.is_default).count(), 1);

        assert!(save_preset(&connection, custom("study")).is_err());
        assert!(save_preset(&connection, FocusPreset { focus_minutes: 0, ..custom("Bad") }).is_err());

        delete_preset(&connection, created.id.unwrap()).unwrap();
        assert!(get_preset(&connection, created.id.unwrap()).is_err());
        assert!(delete_preset(&connection, created.id.unwrap()).is_err());
    }

    #[test]
    fn deleting_a_preset_detaches_timetable_blocks() {
        let connection = connection();
        let preset = save_preset(&connection, custom("Study")).unwrap();

        let entry = crate::timetable::save_entry(
            &connection,
            crate::timetable::TimetableEntryDraft {
                id: None,
                title: "Study block".into(),
                notes: String::new(),
                color: String::new(),
                date: Some("2026-01-05".into()),
                start_minute: 600,
                duration_minutes: 60,
                recurrence: None,
                todo_id: None,
                preset_id: preset.id,
                auto_start: true,
            },
        )
        .unwrap();
        assert!(entry.auto_start);

        delete_preset(&connection, preset.id.unwrap()).unwrap();
        let updated = crate::timetable::get_entry(&connection, entry.id).unwrap();
        assert_eq!((updated.preset_id, updated.auto_start), (None, false));
    }

    #[test]
    fn logs_sessions_and_summarises_them() {
        let connection = connection();
        let today = Local::now().date_naive();
        let noon = crate::db::local_date_to_unix(today, 12, 0, 0);

        let log = |offset: i64, actual: i64, completed: bool, todo: Option<i64>| FocusLog {
            started_at: noon + offset,
            ended_at: noon + offset + actual,
            planned_seconds: 1_500,
            actual_seconds: actual,
            completed,
            preset_id: None,
            preset_name: "Pomodoro".into(),
            todo_id: todo,
            entry_id: None,
        };
        insert_log(&connection, &log(0, 1_500, true, Some(4))).unwrap();
        insert_log(&connection, &log(2_000, 600, false, Some(4))).unwrap();
        insert_log(&connection, &log(4_000, 1_500, true, None)).unwrap();

        let stats = focus_stats(&connection, &(today - Duration::days(2)).to_string(), &today.to_string()).unwrap();
        assert_eq!(stats.total_focus_seconds, 3_600);
        assert_eq!(stats.completed_sessions, 2);
        assert_eq!(stats.abandoned_sessions, 1);
        assert_eq!(stats.daily.len(), 3);
        assert_eq!(stats.daily[0].seconds, 0);
        assert_eq!(stats.daily[2].seconds, 3_600);
        assert_eq!(stats.by_todo, vec![TodoFocus { todo_id: 4, seconds: 2_100 }]);

        let recent = recent_logs(&connection, 2).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].started_at, noon + 4_000);
        assert!(focus_stats(&connection, "2026-02-01", "2026-01-01").is_err());
    }
}
