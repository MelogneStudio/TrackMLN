use chrono::{Duration, Local};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::recurrence::{local_clock, parse_date, Recurrence};
use crate::AppState;

const MAX_TITLE_CHARS: usize = 200;
const TODO_COLUMNS: &str = "id, title, notes, priority, due_date, due_minute, estimate_minutes, \
                            created_at, completed_at, template_id, occurrence_date";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub notes: String,
    /// 0 = none, 1 = low, 2 = medium, 3 = high
    pub priority: i64,
    /// Local `YYYY-MM-DD`; `None` means no deadline.
    pub due_date: Option<String>,
    /// Minutes after midnight; `None` means "by end of day".
    pub due_minute: Option<i64>,
    pub estimate_minutes: Option<i64>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub completed: bool,
    pub overdue: bool,
    /// Set when this todo is one occurrence of a recurring todo.
    pub template_id: Option<i64>,
    pub occurrence_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoDraft {
    #[serde(default)]
    pub id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub due_minute: Option<i64>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoSummary {
    pub open: i64,
    pub overdue: i64,
    pub due_today: i64,
    pub completed_today: i64,
}

fn db_err(err: rusqlite::Error) -> String {
    err.to_string()
}

pub fn init_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 0,
            due_date TEXT,
            due_minute INTEGER,
            estimate_minutes INTEGER,
            created_at INTEGER NOT NULL,
            completed_at INTEGER,
            template_id INTEGER,
            occurrence_date TEXT,
            skipped INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS todo_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 0,
            due_minute INTEGER,
            estimate_minutes INTEGER,
            recurrence TEXT NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_todos_due ON todos(due_date);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_todos_occurrence
            ON todos(template_id, occurrence_date) WHERE template_id IS NOT NULL;
        ",
    )
}

pub fn is_overdue(due_date: Option<&str>, due_minute: Option<i64>, today: &str, now_minute: i64) -> bool {
    match due_date {
        None => false,
        Some(date) if date < today => true,
        Some(date) if date == today => due_minute.map_or(false, |minute| minute < now_minute),
        Some(_) => false,
    }
}

fn row_to_todo(row: &Row, today: &str, now_minute: i64) -> rusqlite::Result<Todo> {
    let due_date: Option<String> = row.get(4)?;
    let due_minute: Option<i64> = row.get(5)?;
    let completed_at: Option<i64> = row.get(8)?;
    let overdue = completed_at.is_none() && is_overdue(due_date.as_deref(), due_minute, today, now_minute);

    Ok(Todo {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        priority: row.get(3)?,
        due_date,
        due_minute,
        estimate_minutes: row.get(6)?,
        created_at: row.get(7)?,
        completed: completed_at.is_some(),
        completed_at,
        overdue,
        template_id: row.get(9)?,
        occurrence_date: row.get(10)?,
    })
}

pub fn normalize_draft(mut draft: TodoDraft) -> Result<TodoDraft, String> {
    draft.title = draft.title.trim().to_string();
    if draft.title.is_empty() {
        return Err("Give the todo a title".into());
    }
    if draft.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("Title is limited to {MAX_TITLE_CHARS} characters"));
    }

    draft.notes = draft.notes.trim().to_string();
    draft.priority = draft.priority.clamp(0, 3);

    draft.due_date = match draft.due_date.take() {
        Some(raw) if !raw.trim().is_empty() => Some(parse_date(&raw)?.to_string()),
        _ => None,
    };

    draft.due_minute = match (draft.due_date.as_ref(), draft.due_minute) {
        (None, _) | (_, None) => None,
        (Some(_), Some(minute)) if (0..1440).contains(&minute) => Some(minute),
        (Some(_), Some(_)) => return Err("Due time must be between 00:00 and 23:59".into()),
    };

    draft.estimate_minutes = draft
        .estimate_minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| minutes.min(24 * 60));

    Ok(draft)
}

pub fn get_todo(connection: &Connection, id: i64) -> Result<Todo, String> {
    let (today, now_minute) = local_clock();
    let today = today.to_string();
    connection
        .query_row(
            &format!("SELECT {TODO_COLUMNS} FROM todos WHERE id = ?1 AND skipped = 0"),
            params![id],
            |row| row_to_todo(row, &today, now_minute),
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| "Todo not found".to_string())
}

/// `status`: open | completed | overdue | today | all. The optional date bounds
/// filter on the due date (todos without a deadline are left out when set).
pub fn list_todos(
    connection: &Connection,
    status: &str,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<Vec<Todo>, String> {
    let (status_clause, order) = match status {
        "open" => ("completed_at IS NULL", "(due_date IS NULL), due_date, COALESCE(due_minute, 1440), priority DESC, id"),
        "completed" => ("completed_at IS NOT NULL", "completed_at DESC, id DESC"),
        "overdue" => (
            "completed_at IS NULL AND due_date IS NOT NULL
             AND (due_date < ?1 OR (due_date = ?1 AND due_minute IS NOT NULL AND due_minute < ?2))",
            "due_date, COALESCE(due_minute, 1440), priority DESC, id",
        ),
        "today" => (
            "due_date = ?1",
            "(completed_at IS NOT NULL), COALESCE(due_minute, 1440), priority DESC, id",
        ),
        "all" => (
            "1 = 1",
            "(completed_at IS NOT NULL), (due_date IS NULL), due_date, COALESCE(due_minute, 1440), priority DESC, id",
        ),
        other => return Err(format!("Unknown todo status '{other}'")),
    };

    let date_from = date_from.filter(|value| !value.trim().is_empty()).map(|value| parse_date(value).map(|d| d.to_string())).transpose()?;
    let date_to = date_to.filter(|value| !value.trim().is_empty()).map(|value| parse_date(value).map(|d| d.to_string())).transpose()?;

    let (today, now_minute) = local_clock();
    let today = today.to_string();
    let sql = format!(
        "SELECT {TODO_COLUMNS} FROM todos
         WHERE skipped = 0 AND {status_clause}
           AND (?3 IS NULL OR due_date >= ?3)
           AND (?4 IS NULL OR due_date <= ?4)
         ORDER BY {order}"
    );

    let mut statement = connection.prepare(&sql).map_err(db_err)?;
    let todos = statement
        .query_map(params![today, now_minute, date_from, date_to], |row| {
            row_to_todo(row, &today, now_minute)
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    Ok(todos)
}

pub fn save_todo(connection: &Connection, draft: TodoDraft) -> Result<Todo, String> {
    let draft = normalize_draft(draft)?;

    let id = match draft.id {
        Some(id) => {
            let changed = connection
                .execute(
                    "UPDATE todos
                     SET title = ?1, notes = ?2, priority = ?3, due_date = ?4,
                         due_minute = ?5, estimate_minutes = ?6
                     WHERE id = ?7 AND skipped = 0",
                    params![
                        draft.title,
                        draft.notes,
                        draft.priority,
                        draft.due_date,
                        draft.due_minute,
                        draft.estimate_minutes,
                        id
                    ],
                )
                .map_err(db_err)?;
            if changed == 0 {
                return Err("Todo not found".into());
            }
            id
        }
        None => {
            connection
                .execute(
                    "INSERT INTO todos
                        (title, notes, priority, due_date, due_minute, estimate_minutes, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        draft.title,
                        draft.notes,
                        draft.priority,
                        draft.due_date,
                        draft.due_minute,
                        draft.estimate_minutes,
                        Local::now().timestamp()
                    ],
                )
                .map_err(db_err)?;
            connection.last_insert_rowid()
        }
    };

    get_todo(connection, id)
}

pub fn set_completed(connection: &Connection, id: i64, completed: bool) -> Result<Todo, String> {
    let completed_at = if completed { Some(Local::now().timestamp()) } else { None };
    let changed = connection
        .execute(
            "UPDATE todos SET completed_at = ?1 WHERE id = ?2 AND skipped = 0",
            params![completed_at, id],
        )
        .map_err(db_err)?;
    if changed == 0 {
        return Err("Todo not found".into());
    }
    get_todo(connection, id)
}

/// One-off todos are deleted. A single occurrence of a recurring todo is only
/// hidden, so it is not regenerated the next time the series is expanded.
pub fn delete_todo(connection: &Connection, id: i64) -> Result<(), String> {
    let template_id: Option<Option<i64>> = connection
        .query_row("SELECT template_id FROM todos WHERE id = ?1", params![id], |row| row.get(0))
        .optional()
        .map_err(db_err)?;

    match template_id {
        None => Err("Todo not found".into()),
        Some(Some(_)) => {
            connection
                .execute("UPDATE todos SET skipped = 1 WHERE id = ?1", params![id])
                .map_err(db_err)?;
            Ok(())
        }
        Some(None) => {
            connection
                .execute("DELETE FROM todos WHERE id = ?1", params![id])
                .map_err(db_err)?;
            Ok(())
        }
    }
}

pub fn summary(connection: &Connection) -> Result<TodoSummary, String> {
    let (today, now_minute) = local_clock();
    let today = today.to_string();

    let count = |sql: &str| -> Result<i64, String> {
        connection
            .query_row(sql, params![today, now_minute], |row| row.get(0))
            .map_err(db_err)
    };

    Ok(TodoSummary {
        open: count("SELECT COUNT(*) FROM todos WHERE skipped = 0 AND completed_at IS NULL AND (?1 IS NOT NULL OR ?2 IS NOT NULL)")?,
        overdue: count(
            "SELECT COUNT(*) FROM todos
             WHERE skipped = 0 AND completed_at IS NULL AND due_date IS NOT NULL
               AND (due_date < ?1 OR (due_date = ?1 AND due_minute IS NOT NULL AND due_minute < ?2))",
        )?,
        due_today: count(
            "SELECT COUNT(*) FROM todos
             WHERE skipped = 0 AND completed_at IS NULL AND due_date = ?1 AND ?2 IS NOT NULL",
        )?,
        completed_today: count(
            "SELECT COUNT(*) FROM todos
             WHERE skipped = 0 AND completed_at IS NOT NULL
               AND date(completed_at, 'unixepoch', 'localtime') = ?1 AND ?2 IS NOT NULL",
        )?,
    })
}

// ── Recurring todos ──────────────────────────────────────────────────────────

/// Occurrences of recurring todos are created this many days ahead of today.
pub const MATERIALIZE_DAYS_AHEAD: i64 = 14;

const TEMPLATE_COLUMNS: &str =
    "id, title, notes, priority, due_minute, estimate_minutes, recurrence, active, created_at";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoTemplate {
    pub id: i64,
    pub title: String,
    pub notes: String,
    pub priority: i64,
    pub due_minute: Option<i64>,
    pub estimate_minutes: Option<i64>,
    pub recurrence: Recurrence,
    pub active: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoTemplateDraft {
    #[serde(default)]
    pub id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub due_minute: Option<i64>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
    pub recurrence: Recurrence,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

fn row_to_template(row: &Row) -> rusqlite::Result<TodoTemplate> {
    let raw: String = row.get(6)?;
    let recurrence = serde_json::from_str::<Recurrence>(&raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let active: i64 = row.get(7)?;

    Ok(TodoTemplate {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        priority: row.get(3)?,
        due_minute: row.get(4)?,
        estimate_minutes: row.get(5)?,
        recurrence,
        active: active != 0,
        created_at: row.get(8)?,
    })
}

pub fn list_templates(connection: &Connection) -> Result<Vec<TodoTemplate>, String> {
    let mut statement = connection
        .prepare(&format!("SELECT {TEMPLATE_COLUMNS} FROM todo_templates ORDER BY title COLLATE NOCASE, id"))
        .map_err(db_err)?;
    let templates = statement
        .query_map([], row_to_template)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    Ok(templates)
}

fn get_template(connection: &Connection, id: i64) -> Result<TodoTemplate, String> {
    connection
        .query_row(
            &format!("SELECT {TEMPLATE_COLUMNS} FROM todo_templates WHERE id = ?1"),
            params![id],
            row_to_template,
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| "Recurring todo not found".to_string())
}

/// Creates the missing occurrences of every active recurring todo from today
/// through `MATERIALIZE_DAYS_AHEAD`. Safe to call repeatedly: occurrences that
/// already exist (done, edited, or removed) are left alone.
pub fn materialize(connection: &Connection) -> Result<usize, String> {
    let (today, _) = local_clock();
    let until = today + Duration::days(MATERIALIZE_DAYS_AHEAD);
    let now = Local::now().timestamp();
    let mut created = 0;

    for template in list_templates(connection)?.iter().filter(|template| template.active) {
        for date in template.recurrence.occurrences_between(today, until) {
            created += connection
                .execute(
                    "INSERT OR IGNORE INTO todos
                        (title, notes, priority, due_date, due_minute, estimate_minutes,
                         created_at, template_id, occurrence_date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?4)",
                    params![
                        template.title,
                        template.notes,
                        template.priority,
                        date.to_string(),
                        template.due_minute,
                        template.estimate_minutes,
                        now,
                        template.id
                    ],
                )
                .map_err(db_err)?;
        }
    }
    Ok(created)
}

/// Drops the upcoming, untouched occurrences of a series so they can be rebuilt
/// from the current rule. Completed and individually removed ones stay.
fn clear_upcoming_occurrences(connection: &Connection, template_id: i64) -> Result<(), String> {
    let (today, _) = local_clock();
    connection
        .execute(
            "DELETE FROM todos
             WHERE template_id = ?1 AND completed_at IS NULL AND skipped = 0 AND due_date >= ?2",
            params![template_id, today.to_string()],
        )
        .map_err(db_err)?;
    Ok(())
}

pub fn save_template(connection: &Connection, draft: TodoTemplateDraft) -> Result<TodoTemplate, String> {
    let recurrence = draft.recurrence.validated()?;
    let checked = normalize_draft(TodoDraft {
        id: None,
        title: draft.title,
        notes: draft.notes,
        priority: draft.priority,
        due_date: Some(recurrence.start_date.clone()),
        due_minute: draft.due_minute,
        estimate_minutes: draft.estimate_minutes,
    })?;
    let recurrence_json = serde_json::to_string(&recurrence).map_err(|err| err.to_string())?;

    let id = match draft.id {
        Some(id) => {
            let changed = connection
                .execute(
                    "UPDATE todo_templates
                     SET title = ?1, notes = ?2, priority = ?3, due_minute = ?4,
                         estimate_minutes = ?5, recurrence = ?6, active = ?7
                     WHERE id = ?8",
                    params![
                        checked.title,
                        checked.notes,
                        checked.priority,
                        checked.due_minute,
                        checked.estimate_minutes,
                        recurrence_json,
                        draft.active as i64,
                        id
                    ],
                )
                .map_err(db_err)?;
            if changed == 0 {
                return Err("Recurring todo not found".into());
            }
            id
        }
        None => {
            connection
                .execute(
                    "INSERT INTO todo_templates
                        (title, notes, priority, due_minute, estimate_minutes, recurrence, active, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        checked.title,
                        checked.notes,
                        checked.priority,
                        checked.due_minute,
                        checked.estimate_minutes,
                        recurrence_json,
                        draft.active as i64,
                        Local::now().timestamp()
                    ],
                )
                .map_err(db_err)?;
            connection.last_insert_rowid()
        }
    };

    clear_upcoming_occurrences(connection, id)?;
    materialize(connection)?;
    get_template(connection, id)
}

pub fn set_template_active(connection: &Connection, id: i64, active: bool) -> Result<TodoTemplate, String> {
    let changed = connection
        .execute(
            "UPDATE todo_templates SET active = ?1 WHERE id = ?2",
            params![active as i64, id],
        )
        .map_err(db_err)?;
    if changed == 0 {
        return Err("Recurring todo not found".into());
    }

    if !active {
        clear_upcoming_occurrences(connection, id)?;
    }
    materialize(connection)?;
    get_template(connection, id)
}

/// Removes the series and every occurrence not yet completed. Completed
/// occurrences stay as history.
pub fn delete_template(connection: &Connection, id: i64) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM todos WHERE template_id = ?1 AND completed_at IS NULL",
            params![id],
        )
        .map_err(db_err)?;
    let changed = connection
        .execute("DELETE FROM todo_templates WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if changed == 0 {
        return Err("Recurring todo not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn get_todo_templates(state: State<AppState>) -> Result<Vec<TodoTemplate>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    list_templates(&connection)
}

#[tauri::command]
pub fn save_todo_template(state: State<AppState>, draft: TodoTemplateDraft) -> Result<TodoTemplate, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    save_template(&connection, draft)
}

#[tauri::command]
pub fn set_todo_template_active(state: State<AppState>, template_id: i64, active: bool) -> Result<TodoTemplate, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    set_template_active(&connection, template_id, active)
}

#[tauri::command]
pub fn delete_todo_template(state: State<AppState>, template_id: i64) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    delete_template(&connection, template_id)
}

#[tauri::command]
pub fn get_todos(
    state: State<AppState>,
    status: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<Vec<Todo>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    materialize(&connection)?;
    list_todos(
        &connection,
        status.as_deref().unwrap_or("open"),
        date_from.as_deref(),
        date_to.as_deref(),
    )
}

#[tauri::command]
pub fn save_todo_item(state: State<AppState>, draft: TodoDraft) -> Result<Todo, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    save_todo(&connection, draft)
}

#[tauri::command]
pub fn set_todo_completed(state: State<AppState>, todo_id: i64, completed: bool) -> Result<Todo, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    set_completed(&connection, todo_id, completed)
}

#[tauri::command]
pub fn delete_todo_item(state: State<AppState>, todo_id: i64) -> Result<(), String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    delete_todo(&connection, todo_id)
}

#[tauri::command]
pub fn get_todo_summary(state: State<AppState>) -> Result<TodoSummary, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    materialize(&connection)?;
    summary(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        init_schema(&connection).unwrap();
        connection
    }

    fn draft(title: &str) -> TodoDraft {
        TodoDraft {
            id: None,
            title: title.into(),
            notes: String::new(),
            priority: 0,
            due_date: None,
            due_minute: None,
            estimate_minutes: None,
        }
    }

    #[test]
    fn overdue_rules() {
        assert!(!is_overdue(None, None, "2026-05-10", 600));
        assert!(is_overdue(Some("2026-05-09"), None, "2026-05-10", 600));
        assert!(!is_overdue(Some("2026-05-10"), None, "2026-05-10", 1439));
        assert!(is_overdue(Some("2026-05-10"), Some(540), "2026-05-10", 600));
        assert!(!is_overdue(Some("2026-05-10"), Some(700), "2026-05-10", 600));
        assert!(!is_overdue(Some("2026-05-11"), Some(0), "2026-05-10", 600));
    }

    #[test]
    fn validates_drafts() {
        assert!(normalize_draft(draft("   ")).is_err());

        let mut bad_date = draft("x");
        bad_date.due_date = Some("tomorrow".into());
        assert!(normalize_draft(bad_date).is_err());

        let mut bad_time = draft("x");
        bad_time.due_date = Some("2026-05-10".into());
        bad_time.due_minute = Some(1440);
        assert!(normalize_draft(bad_time).is_err());

        let mut no_date = draft("  Write report ");
        no_date.priority = 9;
        no_date.due_minute = Some(600);
        let cleaned = normalize_draft(no_date).unwrap();
        assert_eq!(cleaned.title, "Write report");
        assert_eq!(cleaned.priority, 3);
        assert_eq!(cleaned.due_minute, None);
    }

    #[test]
    fn create_edit_complete_and_delete() {
        let connection = connection();

        let mut new_todo = draft("Pay rent");
        new_todo.due_date = Some("2000-01-01".into());
        let created = save_todo(&connection, new_todo).unwrap();
        assert!(created.overdue);
        assert!(!created.completed);

        let mut edit = draft("Pay rent now");
        edit.id = Some(created.id);
        edit.due_date = Some("2999-01-01".into());
        let edited = save_todo(&connection, edit).unwrap();
        assert_eq!(edited.title, "Pay rent now");
        assert!(!edited.overdue);

        let done = set_completed(&connection, created.id, true).unwrap();
        assert!(done.completed);
        assert_eq!(list_todos(&connection, "open", None, None).unwrap().len(), 0);
        assert_eq!(list_todos(&connection, "completed", None, None).unwrap().len(), 1);

        let undone = set_completed(&connection, created.id, false).unwrap();
        assert!(!undone.completed);

        delete_todo(&connection, created.id).unwrap();
        assert!(get_todo(&connection, created.id).is_err());
        assert!(delete_todo(&connection, created.id).is_err());
    }

    #[test]
    fn lists_by_status_and_date_range() {
        let connection = connection();

        for (title, date) in [("a", "2000-01-01"), ("b", "2999-01-01"), ("c", "2999-06-01")] {
            let mut item = draft(title);
            item.due_date = Some(date.into());
            save_todo(&connection, item).unwrap();
        }
        save_todo(&connection, draft("no deadline")).unwrap();

        assert_eq!(list_todos(&connection, "overdue", None, None).unwrap().len(), 1);
        assert_eq!(list_todos(&connection, "all", None, None).unwrap().len(), 4);
        let ranged = list_todos(&connection, "open", Some("2999-01-01"), Some("2999-03-01")).unwrap();
        assert_eq!(ranged.len(), 1);
        assert_eq!(ranged[0].title, "b");
        assert!(list_todos(&connection, "bogus", None, None).is_err());

        let counts = summary(&connection).unwrap();
        assert_eq!(counts.open, 4);
        assert_eq!(counts.overdue, 1);
    }
}

#[cfg(test)]
mod template_tests {
    use super::*;
    use crate::recurrence::RecurrenceKind;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        init_schema(&connection).unwrap();
        connection
    }

    fn daily_draft(title: &str) -> TodoTemplateDraft {
        let (today, _) = local_clock();
        TodoTemplateDraft {
            id: None,
            title: title.into(),
            notes: String::new(),
            priority: 1,
            due_minute: Some(9 * 60),
            estimate_minutes: None,
            recurrence: Recurrence {
                kind: RecurrenceKind::Daily,
                interval: 1,
                weekdays: vec![],
                month_day: None,
                start_date: today.to_string(),
                end_date: None,
            },
            active: true,
        }
    }

    #[test]
    fn materializes_a_rolling_window_without_duplicates() {
        let connection = connection();
        save_template(&connection, daily_draft("Stretch")).unwrap();

        let all = list_todos(&connection, "all", None, None).unwrap();
        assert_eq!(all.len() as i64, MATERIALIZE_DAYS_AHEAD + 1);
        assert!(all.iter().all(|todo| todo.template_id.is_some() && todo.due_minute == Some(540)));

        assert_eq!(materialize(&connection).unwrap(), 0);
    }

    #[test]
    fn deleting_one_occurrence_does_not_bring_it_back() {
        let connection = connection();
        save_template(&connection, daily_draft("Stretch")).unwrap();

        let first = list_todos(&connection, "open", None, None).unwrap()[0].clone();
        delete_todo(&connection, first.id).unwrap();
        assert_eq!(materialize(&connection).unwrap(), 0);

        let remaining = list_todos(&connection, "all", None, None).unwrap();
        assert_eq!(remaining.len() as i64, MATERIALIZE_DAYS_AHEAD);
        assert!(remaining.iter().all(|todo| todo.id != first.id));
    }

    #[test]
    fn editing_the_series_keeps_completed_occurrences() {
        let connection = connection();
        let template = save_template(&connection, daily_draft("Stretch")).unwrap();

        let first = list_todos(&connection, "open", None, None).unwrap()[0].clone();
        set_completed(&connection, first.id, true).unwrap();

        let mut edit = daily_draft("Stretch longer");
        edit.id = Some(template.id);
        save_template(&connection, edit).unwrap();

        let all = list_todos(&connection, "all", None, None).unwrap();
        assert_eq!(all.len() as i64, MATERIALIZE_DAYS_AHEAD + 1);
        let done: Vec<_> = all.iter().filter(|todo| todo.completed).collect();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].title, "Stretch");
        assert!(all.iter().filter(|todo| !todo.completed).all(|todo| todo.title == "Stretch longer"));
    }

    #[test]
    fn pausing_and_deleting_a_series() {
        let connection = connection();
        let template = save_template(&connection, daily_draft("Stretch")).unwrap();

        set_template_active(&connection, template.id, false).unwrap();
        assert_eq!(list_todos(&connection, "open", None, None).unwrap().len(), 0);

        set_template_active(&connection, template.id, true).unwrap();
        assert_eq!(list_todos(&connection, "open", None, None).unwrap().len() as i64, MATERIALIZE_DAYS_AHEAD + 1);

        delete_template(&connection, template.id).unwrap();
        assert_eq!(list_todos(&connection, "all", None, None).unwrap().len(), 0);
        assert!(list_templates(&connection).unwrap().is_empty());
        assert!(delete_template(&connection, template.id).is_err());
    }
}
