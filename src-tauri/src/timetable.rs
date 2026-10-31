use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::recurrence::{parse_date, Recurrence};
use crate::AppState;

const MINUTES_PER_DAY: i64 = 1440;
const MAX_RANGE_DAYS: i64 = 92;
const MAX_TITLE_CHARS: usize = 200;
const MAX_COLOR_CHARS: usize = 32;
const DEFAULT_TODO_BLOCK_MINUTES: i64 = 30;

const ENTRY_COLUMNS: &str =
    "id, title, notes, color, date, start_minute, duration_minutes, recurrence, todo_id, preset_id, auto_start";

/// A timetable block definition: either a one-off on `date`, or a recurring
/// series described by `recurrence`. Times are minutes after local midnight.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimetableEntry {
    pub id: i64,
    /// Empty when the block follows the title of its linked todo.
    pub title: String,
    pub notes: String,
    pub color: String,
    pub date: Option<String>,
    pub start_minute: i64,
    pub duration_minutes: i64,
    pub recurrence: Option<Recurrence>,
    pub todo_id: Option<i64>,
    /// Focus preset to run during this block.
    pub preset_id: Option<i64>,
    /// Start the focus preset automatically when the block begins.
    pub auto_start: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimetableEntryDraft {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub date: Option<String>,
    pub start_minute: i64,
    pub duration_minutes: i64,
    #[serde(default)]
    pub recurrence: Option<Recurrence>,
    #[serde(default)]
    pub todo_id: Option<i64>,
    #[serde(default)]
    pub preset_id: Option<i64>,
    #[serde(default)]
    pub auto_start: bool,
}

/// One concrete block on one day, after applying per-day edits and removals.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimetableOccurrence {
    pub entry_id: i64,
    pub date: String,
    pub title: String,
    pub notes: String,
    pub color: String,
    pub start_minute: i64,
    pub end_minute: i64,
    pub duration_minutes: i64,
    pub todo_id: Option<i64>,
    pub todo_completed: Option<bool>,
    pub preset_id: Option<i64>,
    pub auto_start: bool,
    pub recurring: bool,
    /// This day differs from the series (moved, resized or renamed).
    pub modified: bool,
    /// Overlaps another block on the same day.
    pub overlaps: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OccurrencePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub start_minute: Option<i64>,
    #[serde(default)]
    pub duration_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FreeSlot {
    pub start_minute: i64,
    pub end_minute: i64,
}

#[derive(Debug, Clone)]
struct DayException {
    deleted: bool,
    title: Option<String>,
    start_minute: Option<i64>,
    duration_minutes: Option<i64>,
}

fn db_err(err: rusqlite::Error) -> String {
    err.to_string()
}

pub fn init_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS timetable_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '',
            date TEXT,
            start_minute INTEGER NOT NULL,
            duration_minutes INTEGER NOT NULL,
            recurrence TEXT,
            todo_id INTEGER,
            preset_id INTEGER,
            auto_start INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS timetable_exceptions (
            entry_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            deleted INTEGER NOT NULL DEFAULT 0,
            title TEXT,
            start_minute INTEGER,
            duration_minutes INTEGER,
            PRIMARY KEY (entry_id, date)
        );

        CREATE INDEX IF NOT EXISTS idx_timetable_entries_date ON timetable_entries(date);
        CREATE INDEX IF NOT EXISTS idx_timetable_entries_todo ON timetable_entries(todo_id);
        ",
    )
}

fn row_to_entry(row: &Row) -> rusqlite::Result<TimetableEntry> {
    let raw: Option<String> = row.get(7)?;
    let recurrence = match raw {
        Some(json) => Some(serde_json::from_str::<Recurrence>(&json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?),
        None => None,
    };
    let auto_start: i64 = row.get(10)?;

    Ok(TimetableEntry {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        color: row.get(3)?,
        date: row.get(4)?,
        start_minute: row.get(5)?,
        duration_minutes: row.get(6)?,
        recurrence,
        todo_id: row.get(8)?,
        preset_id: row.get(9)?,
        auto_start: auto_start != 0,
    })
}

fn check_block(start_minute: i64, duration_minutes: i64) -> Result<(), String> {
    if !(0..MINUTES_PER_DAY).contains(&start_minute) {
        return Err("Start time must be between 00:00 and 23:59".into());
    }
    if duration_minutes < 1 {
        return Err("Duration must be at least 1 minute".into());
    }
    if start_minute + duration_minutes > MINUTES_PER_DAY {
        return Err("A block cannot run past midnight; split it across two days".into());
    }
    Ok(())
}

pub fn normalize_draft(mut draft: TimetableEntryDraft) -> Result<TimetableEntryDraft, String> {
    draft.title = draft.title.trim().to_string();
    if draft.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("Title is limited to {MAX_TITLE_CHARS} characters"));
    }
    if draft.title.is_empty() && draft.todo_id.is_none() {
        return Err("Give the block a title or link it to a todo".into());
    }

    draft.notes = draft.notes.trim().to_string();
    draft.color = draft.color.trim().to_string();
    if draft.color.chars().count() > MAX_COLOR_CHARS {
        return Err("Color value is too long".into());
    }

    check_block(draft.start_minute, draft.duration_minutes)?;

    match draft.recurrence.take() {
        Some(rule) => {
            draft.recurrence = Some(rule.validated()?);
            draft.date = None;
        }
        None => {
            let raw = draft
                .date
                .take()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Pick a date for this block".to_string())?;
            draft.date = Some(parse_date(&raw)?.to_string());
        }
    }

    if draft.auto_start && draft.preset_id.is_none() {
        return Err("Auto-start needs a focus preset".into());
    }
    Ok(draft)
}

pub fn get_entry(connection: &Connection, id: i64) -> Result<TimetableEntry, String> {
    connection
        .query_row(
            &format!("SELECT {ENTRY_COLUMNS} FROM timetable_entries WHERE id = ?1"),
            params![id],
            row_to_entry,
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| "Timetable entry not found".to_string())
}

pub fn list_entries(connection: &Connection) -> Result<Vec<TimetableEntry>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM timetable_entries ORDER BY start_minute, id"
        ))
        .map_err(db_err)?;
    let entries = statement
        .query_map([], row_to_entry)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    Ok(entries)
}

pub fn save_entry(connection: &Connection, draft: TimetableEntryDraft) -> Result<TimetableEntry, String> {
    let draft = normalize_draft(draft)?;
    let recurrence_json = draft
        .recurrence
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| err.to_string())?;

    let id = match draft.id {
        Some(id) => {
            let changed = connection
                .execute(
                    "UPDATE timetable_entries
                     SET title = ?1, notes = ?2, color = ?3, date = ?4, start_minute = ?5,
                         duration_minutes = ?6, recurrence = ?7, todo_id = ?8, preset_id = ?9,
                         auto_start = ?10
                     WHERE id = ?11",
                    params![
                        draft.title,
                        draft.notes,
                        draft.color,
                        draft.date,
                        draft.start_minute,
                        draft.duration_minutes,
                        recurrence_json,
                        draft.todo_id,
                        draft.preset_id,
                        draft.auto_start as i64,
                        id
                    ],
                )
                .map_err(db_err)?;
            if changed == 0 {
                return Err("Timetable entry not found".into());
            }
            id
        }
        None => {
            connection
                .execute(
                    "INSERT INTO timetable_entries
                        (title, notes, color, date, start_minute, duration_minutes, recurrence,
                         todo_id, preset_id, auto_start, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        draft.title,
                        draft.notes,
                        draft.color,
                        draft.date,
                        draft.start_minute,
                        draft.duration_minutes,
                        recurrence_json,
                        draft.todo_id,
                        draft.preset_id,
                        draft.auto_start as i64,
                        Local::now().timestamp()
                    ],
                )
                .map_err(db_err)?;
            connection.last_insert_rowid()
        }
    };

    get_entry(connection, id)
}

/// Deletes the whole block or series, including its per-day edits.
pub fn delete_entry(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM timetable_entries WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if changed == 0 {
        return Err("Timetable entry not found".into());
    }
    connection
        .execute("DELETE FROM timetable_exceptions WHERE entry_id = ?1", params![id])
        .map_err(db_err)?;
    Ok(())
}

/// Removes just one day of a recurring series (a one-off block is deleted).
pub fn skip_occurrence(connection: &Connection, entry_id: i64, date: &str) -> Result<(), String> {
    let entry = get_entry(connection, entry_id)?;
    let date = parse_date(date)?;

    match entry.recurrence {
        None => delete_entry(connection, entry_id),
        Some(rule) => {
            if !rule.matches(date) {
                return Err("That day is not part of the series".into());
            }
            connection
                .execute(
                    "INSERT INTO timetable_exceptions (entry_id, date, deleted) VALUES (?1, ?2, 1)
                     ON CONFLICT(entry_id, date) DO UPDATE SET deleted = 1",
                    params![entry_id, date.to_string()],
                )
                .map_err(db_err)?;
            Ok(())
        }
    }
}

/// Changes one day of a recurring series (a one-off block is edited directly).
pub fn edit_occurrence(
    connection: &Connection,
    entry_id: i64,
    date: &str,
    patch: OccurrencePatch,
) -> Result<(), String> {
    let entry = get_entry(connection, entry_id)?;
    let date = parse_date(date)?;

    let title = patch
        .title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if title.as_ref().map_or(false, |value| value.chars().count() > MAX_TITLE_CHARS) {
        return Err(format!("Title is limited to {MAX_TITLE_CHARS} characters"));
    }
    check_block(
        patch.start_minute.unwrap_or(entry.start_minute),
        patch.duration_minutes.unwrap_or(entry.duration_minutes),
    )?;

    match entry.recurrence {
        None => {
            connection
                .execute(
                    "UPDATE timetable_entries
                     SET title = COALESCE(?1, title),
                         start_minute = COALESCE(?2, start_minute),
                         duration_minutes = COALESCE(?3, duration_minutes)
                     WHERE id = ?4",
                    params![title, patch.start_minute, patch.duration_minutes, entry_id],
                )
                .map_err(db_err)?;
            Ok(())
        }
        Some(rule) => {
            if !rule.matches(date) {
                return Err("That day is not part of the series".into());
            }
            connection
                .execute(
                    "INSERT INTO timetable_exceptions
                        (entry_id, date, deleted, title, start_minute, duration_minutes)
                     VALUES (?1, ?2, 0, ?3, ?4, ?5)
                     ON CONFLICT(entry_id, date) DO UPDATE SET
                        deleted = 0,
                        title = COALESCE(excluded.title, timetable_exceptions.title),
                        start_minute = COALESCE(excluded.start_minute, timetable_exceptions.start_minute),
                        duration_minutes = COALESCE(excluded.duration_minutes, timetable_exceptions.duration_minutes)",
                    params![entry_id, date.to_string(), title, patch.start_minute, patch.duration_minutes],
                )
                .map_err(db_err)?;
            Ok(())
        }
    }
}

/// Undoes a skip or per-day edit, putting that day back to the series default.
pub fn restore_occurrence(connection: &Connection, entry_id: i64, date: &str) -> Result<(), String> {
    let date = parse_date(date)?;
    connection
        .execute(
            "DELETE FROM timetable_exceptions WHERE entry_id = ?1 AND date = ?2",
            params![entry_id, date.to_string()],
        )
        .map_err(db_err)?;
    Ok(())
}

fn load_exceptions(connection: &Connection) -> Result<HashMap<(i64, String), DayException>, String> {
    let mut statement = connection
        .prepare("SELECT entry_id, date, deleted, title, start_minute, duration_minutes FROM timetable_exceptions")
        .map_err(db_err)?;
    let rows = statement
        .query_map([], |row| {
            let deleted: i64 = row.get(2)?;
            Ok((
                (row.get::<_, i64>(0)?, row.get::<_, String>(1)?),
                DayException {
                    deleted: deleted != 0,
                    title: row.get(3)?,
                    start_minute: row.get(4)?,
                    duration_minutes: row.get(5)?,
                },
            ))
        })
        .map_err(db_err)?;

    let mut exceptions = HashMap::new();
    for row in rows {
        let (key, exception) = row.map_err(db_err)?;
        exceptions.insert(key, exception);
    }
    Ok(exceptions)
}

/// id -> (title, completed) for every todo a timetable entry points at.
fn load_linked_todos(connection: &Connection) -> Result<HashMap<i64, (String, bool)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, completed_at IS NOT NULL FROM todos
             WHERE skipped = 0 AND id IN (SELECT todo_id FROM timetable_entries WHERE todo_id IS NOT NULL)",
        )
        .map_err(db_err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, (row.get::<_, String>(1)?, row.get::<_, bool>(2)?)))
        })
        .map_err(db_err)?;

    let mut todos = HashMap::new();
    for row in rows {
        let (id, info) = row.map_err(db_err)?;
        todos.insert(id, info);
    }
    Ok(todos)
}

fn parse_range(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate), String> {
    let from = parse_date(from)?;
    let to = parse_date(to)?;
    if to < from {
        return Err("End date is before the start date".into());
    }
    if (to - from).num_days() >= MAX_RANGE_DAYS {
        return Err(format!("Range is limited to {MAX_RANGE_DAYS} days"));
    }
    Ok((from, to))
}

/// Every block between `from` and `to` (inclusive), sorted by day then start time.
pub fn occurrences(connection: &Connection, from: &str, to: &str) -> Result<Vec<TimetableOccurrence>, String> {
    let (from, to) = parse_range(from, to)?;
    let entries = list_entries(connection)?;
    let exceptions = load_exceptions(connection)?;
    let linked_todos = load_linked_todos(connection)?;
    let mut result: Vec<TimetableOccurrence> = Vec::new();

    for entry in &entries {
        let linked = entry.todo_id.and_then(|id| linked_todos.get(&id));
        if entry.todo_id.is_some() && linked.is_none() && entry.title.is_empty() {
            // The todo this block was following no longer exists.
            continue;
        }

        let dates: Vec<NaiveDate> = match (&entry.recurrence, &entry.date) {
            (Some(rule), _) => rule.occurrences_between(from, to),
            (None, Some(raw)) => match parse_date(raw) {
                Ok(day) if day >= from && day <= to => vec![day],
                _ => Vec::new(),
            },
            (None, None) => Vec::new(),
        };

        for day in dates {
            let day_key = day.to_string();
            let exception = exceptions.get(&(entry.id, day_key.clone()));
            if exception.map_or(false, |item| item.deleted) {
                continue;
            }

            let start_minute = exception
                .and_then(|item| item.start_minute)
                .unwrap_or(entry.start_minute);
            let duration_minutes = exception
                .and_then(|item| item.duration_minutes)
                .unwrap_or(entry.duration_minutes);
            let base_title = if !entry.title.is_empty() {
                entry.title.clone()
            } else {
                linked
                    .map(|(title, _)| title.clone())
                    .unwrap_or_else(|| "Untitled".to_string())
            };
            let title = exception
                .and_then(|item| item.title.clone())
                .unwrap_or(base_title);

            result.push(TimetableOccurrence {
                entry_id: entry.id,
                date: day_key,
                title,
                notes: entry.notes.clone(),
                color: entry.color.clone(),
                start_minute,
                end_minute: (start_minute + duration_minutes).min(MINUTES_PER_DAY),
                duration_minutes,
                todo_id: entry.todo_id.filter(|_| linked.is_some()),
                todo_completed: linked.map(|(_, completed)| *completed),
                preset_id: entry.preset_id,
                auto_start: entry.auto_start,
                recurring: entry.recurrence.is_some(),
                modified: exception.is_some(),
                overlaps: false,
            });
        }
    }

    result.sort_by(|a, b| {
        (a.date.as_str(), a.start_minute, a.entry_id).cmp(&(b.date.as_str(), b.start_minute, b.entry_id))
    });

    // Sorted by start within a day, so a later block overlaps an earlier one
    // exactly when it starts before the earlier one ends.
    let count = result.len();
    for i in 0..count {
        for j in (i + 1)..count {
            if result[j].date != result[i].date {
                break;
            }
            if result[j].start_minute < result[i].end_minute {
                result[i].overlaps = true;
                result[j].overlaps = true;
            }
        }
    }

    Ok(result)
}

/// Gaps of at least `min_minutes` inside `[day_start, day_end]` not covered by a block.
pub fn free_slots(
    connection: &Connection,
    date: &str,
    min_minutes: i64,
    day_start: i64,
    day_end: i64,
) -> Result<Vec<FreeSlot>, String> {
    if day_start < 0 || day_end > MINUTES_PER_DAY || day_start >= day_end {
        return Err("Day window must satisfy 0 <= start < end <= 24:00".into());
    }
    let min_minutes = min_minutes.max(1);
    let blocks = occurrences(connection, date, date)?;

    let mut slots = Vec::new();
    let mut cursor = day_start;
    for block in &blocks {
        if block.end_minute <= day_start || block.start_minute >= day_end {
            continue;
        }
        let busy_start = block.start_minute.max(day_start);
        let busy_end = block.end_minute.min(day_end);
        if busy_start - cursor >= min_minutes {
            slots.push(FreeSlot { start_minute: cursor, end_minute: busy_start });
        }
        cursor = cursor.max(busy_end);
    }
    if day_end - cursor >= min_minutes {
        slots.push(FreeSlot { start_minute: cursor, end_minute: day_end });
    }
    Ok(slots)
}

/// Puts a todo on the timetable. The block follows the todo's title and shows
/// whether it is done; the length defaults to the todo's estimate.
pub fn schedule_todo(
    connection: &Connection,
    todo_id: i64,
    date: &str,
    start_minute: i64,
    duration_minutes: Option<i64>,
    preset_id: Option<i64>,
    auto_start: bool,
) -> Result<TimetableEntry, String> {
    let todo = crate::todos::get_todo(connection, todo_id)?;
    let duration = duration_minutes
        .or(todo.estimate_minutes)
        .unwrap_or(DEFAULT_TODO_BLOCK_MINUTES);

    save_entry(
        connection,
        TimetableEntryDraft {
            id: None,
            title: String::new(),
            notes: String::new(),
            color: String::new(),
            date: Some(date.to_string()),
            start_minute,
            duration_minutes: duration,
            recurrence: None,
            todo_id: Some(todo_id),
            preset_id,
            auto_start,
        },
    )
}

/// Removes every timetable block linked to a todo; returns how many.
pub fn unschedule_todo(connection: &Connection, todo_id: i64) -> Result<usize, String> {
    let ids: Vec<i64> = {
        let mut statement = connection
            .prepare("SELECT id FROM timetable_entries WHERE todo_id = ?1")
            .map_err(db_err)?;
        let rows = statement
            .query_map(params![todo_id], |row| row.get::<_, i64>(0))
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        rows
    };

    for id in &ids {
        delete_entry(connection, *id)?;
    }
    Ok(ids.len())
}

/// Called when a todo is deleted: blocks that only existed to show the todo go
/// away, blocks with their own title keep their slot but lose the link.
pub fn detach_todo(connection: &Connection, todo_id: i64) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM timetable_exceptions WHERE entry_id IN
                (SELECT id FROM timetable_entries WHERE todo_id = ?1 AND title = '')",
            params![todo_id],
        )
        .map_err(db_err)?;
    connection
        .execute(
            "DELETE FROM timetable_entries WHERE todo_id = ?1 AND title = ''",
            params![todo_id],
        )
        .map_err(db_err)?;
    connection
        .execute(
            "UPDATE timetable_entries SET todo_id = NULL WHERE todo_id = ?1",
            params![todo_id],
        )
        .map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_timetable(
    state: State<AppState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<TimetableOccurrence>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    occurrences(&connection, &start_date, &end_date)
}

#[tauri::command]
pub fn get_timetable_entries(state: State<AppState>) -> Result<Vec<TimetableEntry>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    list_entries(&connection)
}

#[tauri::command]
pub fn save_timetable_entry(
    state: State<AppState>,
    draft: TimetableEntryDraft,
) -> Result<TimetableEntry, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    save_entry(&connection, draft)
}

#[tauri::command]
pub fn delete_timetable_entry(state: State<AppState>, entry_id: i64) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    delete_entry(&connection, entry_id)
}

#[tauri::command]
pub fn skip_timetable_occurrence(
    state: State<AppState>,
    entry_id: i64,
    date: String,
) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    skip_occurrence(&connection, entry_id, &date)
}

#[tauri::command]
pub fn edit_timetable_occurrence(
    state: State<AppState>,
    entry_id: i64,
    date: String,
    patch: OccurrencePatch,
) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    edit_occurrence(&connection, entry_id, &date, patch)
}

#[tauri::command]
pub fn restore_timetable_occurrence(
    state: State<AppState>,
    entry_id: i64,
    date: String,
) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    restore_occurrence(&connection, entry_id, &date)
}

#[tauri::command]
pub fn schedule_todo_block(
    state: State<AppState>,
    todo_id: i64,
    date: String,
    start_minute: i64,
    duration_minutes: Option<i64>,
    preset_id: Option<i64>,
    auto_start: Option<bool>,
) -> Result<TimetableEntry, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    schedule_todo(
        &connection,
        todo_id,
        &date,
        start_minute,
        duration_minutes,
        preset_id,
        auto_start.unwrap_or(false),
    )
}

#[tauri::command]
pub fn unschedule_todo_blocks(state: State<AppState>, todo_id: i64) -> Result<usize, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    unschedule_todo(&connection, todo_id)
}

#[tauri::command]
pub fn get_free_slots(
    state: State<AppState>,
    date: String,
    min_minutes: Option<i64>,
    day_start_minute: Option<i64>,
    day_end_minute: Option<i64>,
) -> Result<Vec<FreeSlot>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    free_slots(
        &connection,
        &date,
        min_minutes.unwrap_or(15),
        day_start_minute.unwrap_or(8 * 60),
        day_end_minute.unwrap_or(22 * 60),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recurrence::RecurrenceKind;
    use crate::todos::{self, TodoDraft};

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        todos::init_schema(&connection).unwrap();
        init_schema(&connection).unwrap();
        connection
    }

    fn one_off(title: &str, date: &str, start: i64, duration: i64) -> TimetableEntryDraft {
        TimetableEntryDraft {
            id: None,
            title: title.into(),
            notes: String::new(),
            color: String::new(),
            date: Some(date.into()),
            start_minute: start,
            duration_minutes: duration,
            recurrence: None,
            todo_id: None,
            preset_id: None,
            auto_start: false,
        }
    }

    fn weekly(title: &str, weekdays: Vec<u8>, start: i64, duration: i64) -> TimetableEntryDraft {
        let mut draft = one_off(title, "2026-01-05", start, duration);
        draft.date = None;
        draft.recurrence = Some(Recurrence {
            kind: RecurrenceKind::Weekly,
            interval: 1,
            weekdays,
            month_day: None,
            start_date: "2026-01-05".into(),
            end_date: None,
        });
        draft
    }

    #[test]
    fn expands_recurring_blocks_and_applies_day_exceptions() {
        let connection = connection();
        let entry = save_entry(&connection, weekly("Study", vec![0, 2], 540, 60)).unwrap();

        let all = occurrences(&connection, "2026-01-05", "2026-01-18").unwrap();
        let dates: Vec<_> = all.iter().map(|item| item.date.as_str()).collect();
        assert_eq!(dates, vec!["2026-01-05", "2026-01-07", "2026-01-12", "2026-01-14"]);
        assert!(all.iter().all(|item| item.recurring && !item.modified));

        skip_occurrence(&connection, entry.id, "2026-01-07").unwrap();
        assert_eq!(occurrences(&connection, "2026-01-05", "2026-01-18").unwrap().len(), 3);
        assert!(skip_occurrence(&connection, entry.id, "2026-01-06").is_err());

        edit_occurrence(
            &connection,
            entry.id,
            "2026-01-12",
            OccurrencePatch { title: Some("Study (moved)".into()), start_minute: Some(600), duration_minutes: None },
        )
        .unwrap();
        let moved = occurrences(&connection, "2026-01-12", "2026-01-12").unwrap();
        assert_eq!(moved[0].title, "Study (moved)");
        assert_eq!(moved[0].start_minute, 600);
        assert_eq!(moved[0].end_minute, 660);
        assert!(moved[0].modified);

        restore_occurrence(&connection, entry.id, "2026-01-12").unwrap();
        restore_occurrence(&connection, entry.id, "2026-01-07").unwrap();
        let restored = occurrences(&connection, "2026-01-05", "2026-01-18").unwrap();
        assert_eq!(restored.len(), 4);
        assert!(restored.iter().all(|item| !item.modified && item.title == "Study"));

        delete_entry(&connection, entry.id).unwrap();
        assert!(occurrences(&connection, "2026-01-05", "2026-01-18").unwrap().is_empty());
    }

    #[test]
    fn editing_a_one_off_changes_the_entry_itself() {
        let connection = connection();
        let entry = save_entry(&connection, one_off("Gym", "2026-01-05", 480, 60)).unwrap();

        edit_occurrence(
            &connection,
            entry.id,
            "2026-01-05",
            OccurrencePatch { title: None, start_minute: Some(500), duration_minutes: Some(45) },
        )
        .unwrap();
        let updated = get_entry(&connection, entry.id).unwrap();
        assert_eq!((updated.start_minute, updated.duration_minutes), (500, 45));

        skip_occurrence(&connection, entry.id, "2026-01-05").unwrap();
        assert!(get_entry(&connection, entry.id).is_err());
    }

    #[test]
    fn flags_overlaps_and_finds_free_slots() {
        let connection = connection();
        save_entry(&connection, one_off("A", "2026-01-05", 540, 60)).unwrap();
        save_entry(&connection, one_off("B", "2026-01-05", 570, 60)).unwrap();
        save_entry(&connection, one_off("C", "2026-01-05", 720, 30)).unwrap();

        let day = occurrences(&connection, "2026-01-05", "2026-01-05").unwrap();
        let flags: Vec<_> = day.iter().map(|item| item.overlaps).collect();
        assert_eq!(flags, vec![true, true, false]);

        let slots = free_slots(&connection, "2026-01-05", 15, 480, 1320).unwrap();
        assert_eq!(
            slots,
            vec![
                FreeSlot { start_minute: 480, end_minute: 540 },
                FreeSlot { start_minute: 630, end_minute: 720 },
                FreeSlot { start_minute: 750, end_minute: 1320 },
            ]
        );
        assert!(free_slots(&connection, "2026-01-05", 15, 900, 900).is_err());
    }

    #[test]
    fn rejects_invalid_blocks() {
        let connection = connection();

        assert!(save_entry(&connection, one_off("Late", "2026-01-05", 1400, 60)).is_err());
        assert!(save_entry(&connection, one_off("Zero", "2026-01-05", 600, 0)).is_err());
        assert!(save_entry(&connection, one_off("", "2026-01-05", 600, 30)).is_err());

        let mut no_date = one_off("No date", "2026-01-05", 600, 30);
        no_date.date = None;
        assert!(save_entry(&connection, no_date).is_err());

        let mut auto = one_off("Focus", "2026-01-05", 600, 30);
        auto.auto_start = true;
        assert!(save_entry(&connection, auto.clone()).is_err());
        auto.preset_id = Some(1);
        assert!(save_entry(&connection, auto).is_ok());
    }

    #[test]
    fn linked_todo_drives_title_completion_and_cleanup() {
        let connection = connection();
        let todo = todos::save_todo(
            &connection,
            TodoDraft {
                id: None,
                title: "Write essay".into(),
                notes: String::new(),
                priority: 0,
                due_date: None,
                due_minute: None,
                estimate_minutes: Some(90),
            },
        )
        .unwrap();

        let entry = schedule_todo(&connection, todo.id, "2026-01-05", 600, None, None, false).unwrap();
        assert_eq!(entry.duration_minutes, 90);

        let blocks = occurrences(&connection, "2026-01-05", "2026-01-05").unwrap();
        assert_eq!(blocks[0].title, "Write essay");
        assert_eq!(blocks[0].todo_completed, Some(false));

        todos::set_completed(&connection, todo.id, true).unwrap();
        let blocks = occurrences(&connection, "2026-01-05", "2026-01-05").unwrap();
        assert_eq!(blocks[0].todo_completed, Some(true));

        todos::delete_todo(&connection, todo.id).unwrap();
        assert!(occurrences(&connection, "2026-01-05", "2026-01-05").unwrap().is_empty());
        assert!(get_entry(&connection, entry.id).is_err());
    }

    #[test]
    fn unschedule_removes_every_linked_block() {
        let connection = connection();
        let todo = todos::save_todo(
            &connection,
            TodoDraft {
                id: None,
                title: "Read".into(),
                notes: String::new(),
                priority: 0,
                due_date: None,
                due_minute: None,
                estimate_minutes: None,
            },
        )
        .unwrap();

        schedule_todo(&connection, todo.id, "2026-01-05", 600, None, None, false).unwrap();
        schedule_todo(&connection, todo.id, "2026-01-06", 600, Some(20), None, false).unwrap();
        assert_eq!(unschedule_todo(&connection, todo.id).unwrap(), 2);
        assert!(list_entries(&connection).unwrap().is_empty());
    }
}
