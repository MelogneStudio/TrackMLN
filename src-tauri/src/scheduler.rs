use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::SharedDb;
use crate::focus::FocusEngine;
use crate::focus_store;
use crate::recurrence::local_clock;
use crate::timetable::{self, TimetableOccurrence};

const LOOP_INTERVAL: Duration = Duration::from_secs(2);
/// A block still counts as "just started" this many minutes after its start
/// time, so waking from sleep or launching the app a moment late still fires it.
const START_WINDOW_MINUTES: i64 = 2;
const MATERIALIZE_EVERY_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockStartedPayload {
    block: TimetableOccurrence,
    auto_started: bool,
}

#[derive(Default)]
struct SchedulerState {
    fired: HashSet<(i64, String)>,
    fired_day: String,
    last_materialized: i64,
}

fn just_started(now_minute: i64, start_minute: i64) -> bool {
    now_minute >= start_minute && now_minute < start_minute + START_WINDOW_MINUTES
}

/// Background loop: advances the focus timer, announces timetable blocks as
/// they begin (optionally auto-starting their focus preset), and keeps
/// recurring todos topped up as days roll over.
pub fn start_scheduler(db: SharedDb, focus: Arc<Mutex<FocusEngine>>, app: AppHandle) {
    thread::spawn(move || {
        let mut state = SchedulerState::default();
        loop {
            run_once(&app, &db, &focus, &mut state);
            thread::sleep(LOOP_INTERVAL);
        }
    });
}

fn run_once(app: &AppHandle, db: &SharedDb, focus: &Arc<Mutex<FocusEngine>>, state: &mut SchedulerState) {
    focus_store::tick_engine(app, db, focus);

    let now = focus_store::now_unix();
    if now - state.last_materialized >= MATERIALIZE_EVERY_SECONDS {
        state.last_materialized = now;
        if let Ok(connection) = db.lock() {
            if let Err(err) = crate::todos::materialize(&connection) {
                eprintln!("failed to expand recurring todos: {err}");
            }
        }
    }

    fire_started_blocks(app, db, focus, state);
}

fn fire_started_blocks(
    app: &AppHandle,
    db: &SharedDb,
    focus: &Arc<Mutex<FocusEngine>>,
    state: &mut SchedulerState,
) {
    let (today, now_minute) = local_clock();
    let day_key = today.to_string();
    if state.fired_day != day_key {
        state.fired.clear();
        state.fired_day = day_key.clone();
    }

    let blocks = {
        let Ok(connection) = db.lock() else {
            return;
        };
        match timetable::occurrences(&connection, &day_key, &day_key) {
            Ok(blocks) => blocks,
            Err(err) => {
                eprintln!("failed to read timetable: {err}");
                return;
            }
        }
    };

    for block in blocks {
        if !just_started(now_minute, block.start_minute) || block.todo_completed == Some(true) {
            continue;
        }
        if !state.fired.insert((block.entry_id, block.date.clone())) {
            continue;
        }

        let auto_started = block.auto_start && try_auto_start(app, db, focus, &block);
        let _ = app.emit(
            "timetable-block-started",
            BlockStartedPayload { block, auto_started },
        );
    }
}

/// Starts the block's focus preset unless a run is already going.
fn try_auto_start(
    app: &AppHandle,
    db: &SharedDb,
    focus: &Arc<Mutex<FocusEngine>>,
    block: &TimetableOccurrence,
) -> bool {
    let Some(preset_id) = block.preset_id else {
        return false;
    };
    if focus.lock().map(|engine| engine.is_active()).unwrap_or(true) {
        return false;
    }

    let preset = {
        let Ok(connection) = db.lock() else {
            return false;
        };
        match focus_store::get_preset(&connection, preset_id) {
            Ok(preset) => preset,
            Err(_) => return false,
        }
    };

    focus_store::start_session(app, db, focus, preset, block.todo_id, Some(block.entry_id)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_counts_as_started_only_in_a_short_window() {
        assert!(!just_started(539, 540));
        assert!(just_started(540, 540));
        assert!(just_started(541, 540));
        assert!(!just_started(542, 540));
    }
}
