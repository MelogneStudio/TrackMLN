use std::collections::HashMap;

use tauri::{AppHandle, Emitter, Manager, State};
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::app_names::looks_like_exe_path;
use crate::db;
use crate::models::{AppSettings, AppTotal, Goal, GoalAlertPayload, GoalCandidate, GoalDraft, HourlyData, WeekData};
use crate::settings;
use crate::AppState;

#[tauri::command]
pub fn get_today_totals(state: State<AppState>) -> Result<Vec<AppTotal>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    let apps = db::get_today_totals(&connection).map_err(|err| err.to_string())?;
    let mut resolver = state
        .app_name_resolver
        .lock()
        .map_err(|err| err.to_string())?;
    Ok(apps
        .into_iter()
        .map(|app| present_app_total(&mut resolver, app))
        .collect())
}

#[tauri::command]
pub fn get_week_dashboard(state: State<AppState>) -> Result<WeekData, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    let mut data = db::get_week_dashboard(&connection).map_err(|err| err.to_string())?;
    let mut resolver = state
        .app_name_resolver
        .lock()
        .map_err(|err| err.to_string())?;

    data.apps = data
        .apps
        .into_iter()
        .map(|app| present_app_total(&mut resolver, app))
        .collect();
    data.top_app = data
        .top_app
        .map(|app| present_app_total(&mut resolver, app));
    data.days = data
        .days
        .into_iter()
        .map(|mut day| {
            day.apps = day
                .apps
                .into_iter()
                .map(|app| present_app_total(&mut resolver, app))
                .collect();
            day
        })
        .collect();

    Ok(data)
}

#[tauri::command]
pub fn get_hourly_today(state: State<AppState>) -> Result<Vec<HourlyData>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    db::get_hourly_today(&connection).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_goals(state: State<AppState>) -> Result<Vec<Goal>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    let mut goals = db::get_goals(&connection).map_err(|err| err.to_string())?;
    let mut resolver = state
        .app_name_resolver
        .lock()
        .map_err(|err| err.to_string())?;

    for goal in &mut goals {
        goal.label = present_goal_label(&mut resolver, goal.target_kind.as_str(), goal.target_value.as_str());
    }

    Ok(goals)
}

#[tauri::command]
pub fn get_goal_candidates(state: State<AppState>) -> Result<Vec<GoalCandidate>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    let mut candidates = db::get_goal_candidates(&connection).map_err(|err| err.to_string())?;
    let mut resolver = state
        .app_name_resolver
        .lock()
        .map_err(|err| err.to_string())?;

    for candidate in &mut candidates {
        if looks_like_exe_path(&candidate.app_identity) {
            candidate.app_name = resolver.resolve_app_name(&candidate.app_identity).app_name;
        }
    }

    Ok(candidates)
}

#[tauri::command]
pub fn save_goal(state: State<AppState>, draft: GoalDraft) -> Result<Vec<Goal>, String> {
    let normalized = normalize_goal_draft(draft)?;
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    db::upsert_goal(&connection, &normalized).map_err(|err| err.to_string())?;
    drop(connection);

    if let Some(id) = normalized.id {
        if let Ok(mut runtime) = state.limit_runtime.lock() {
            runtime.snooze_counts.insert(id, 0);
        }
    }

    get_goals(state)
}

#[tauri::command]
pub fn delete_goal(state: State<AppState>, goal_id: i64) -> Result<Vec<Goal>, String> {
    let connection = state.db.lock().map_err(|err| err.to_string())?;
    db::delete_goal(&connection, goal_id).map_err(|err| err.to_string())?;
    drop(connection);

    if let Ok(mut runtime) = state.limit_runtime.lock() {
        runtime.goal_states.remove(&goal_id);
        runtime.snooze_counts.remove(&goal_id);
    }

    get_goals(state)
}

#[tauri::command]
pub fn snooze_goal(state: State<AppState>, goal_id: i64, minutes: u32) -> Result<(), String> {
    let mut runtime = state.limit_runtime.lock().map_err(|err| err.to_string())?;
    runtime.snooze(goal_id, minutes as i64);
    Ok(())
}

#[tauri::command]
pub fn get_pending_alert(state: State<AppState>, label: String) -> Result<Option<GoalAlertPayload>, String> {
    let pending_alerts = state.pending_alerts.lock().map_err(|err| err.to_string())?;
    Ok(pending_alerts.get(label.as_str()).cloned())
}

#[tauri::command]
pub fn clear_pending_alert(state: State<AppState>, label: String) -> Result<(), String> {
    let mut pending_alerts = state.pending_alerts.lock().map_err(|err| err.to_string())?;
    pending_alerts.remove(label.as_str());
    Ok(())
}

#[tauri::command]
pub fn set_warn_clickthrough(app: AppHandle, clickthrough: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("warn")
        .ok_or_else(|| "warn window not found".to_string())?;
    window
        .set_ignore_cursor_events(clickthrough)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    println!("[DEBUG] get_settings called");
    let settings = state.settings.lock().map_err(|err| {
        let e = format!("Failed to lock settings: {}", err);
        println!("[DEBUG] {}", e);
        e
    })?;
    let ui_prefs_str = serde_json::to_string(&settings.ui_prefs).unwrap_or_default();
    println!("[DEBUG] settings loaded: ui_prefs = {}", ui_prefs_str);
    println!("[DEBUG] settings_path = {:?}", state.settings_path);
    Ok(settings.clone())
}

#[tauri::command]
pub fn get_storage_location(state: State<AppState>) -> Result<String, String> {
    Ok(state.settings_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn set_blur_percent(state: State<AppState>, blur_percent: u8) -> Result<AppSettings, String> {
    let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
    settings_value.blur_percent = blur_percent.min(settings::MAX_BLUR_PERCENT);
    settings::save_settings(&state.settings_path, &settings_value)?;
    Ok(settings_value.clone())
}

#[tauri::command]
pub fn reset_blur_percent(state: State<AppState>) -> Result<AppSettings, String> {
    set_blur_percent(state, settings::DEFAULT_BLUR_PERCENT)
}

#[tauri::command]
pub fn set_material(state: State<AppState>, material: String) -> Result<AppSettings, String> {
    let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
    settings_value.material = settings::normalize_material(&material).into();
    settings::save_settings(&state.settings_path, &settings_value)?;
    Ok(settings_value.clone())
}

#[tauri::command]
pub fn reset_material(state: State<AppState>) -> Result<AppSettings, String> {
    set_material(state, settings::DEFAULT_MATERIAL.into())
}

#[tauri::command]
pub fn set_exe_labels(state: State<AppState>, exe_labels: HashMap<String, String>) -> Result<AppSettings, String> {
    let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
    settings_value.exe_labels = settings::normalize_exe_labels(exe_labels);
    {
        let mut resolver = state
            .app_name_resolver
            .lock()
            .map_err(|err| err.to_string())?;
        resolver.set_user_exe_names(settings_value.exe_labels.clone());
    }
    settings::save_settings(&state.settings_path, &settings_value)?;
    Ok(settings_value.clone())
}

#[tauri::command]
pub fn set_idle_timeout(state: State<AppState>, seconds: u64) -> Result<AppSettings, String> {
    let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
    settings_value.idle_timeout_seconds = settings::normalize_idle_timeout(seconds);
    settings::save_settings(&state.settings_path, &settings_value)?;
    Ok(settings_value.clone())
}

#[tauri::command]
pub fn set_ui_prefs(app: AppHandle, state: State<AppState>, prefs: String) -> Result<AppSettings, String> {
    if prefs.len() > settings::MAX_UI_PREFS_BYTES {
        return Err("Preferences are too large".into());
    }
    println!("[DEBUG] set_ui_prefs called with: {}", prefs);
    println!("[DEBUG] settings_path = {:?}", state.settings_path);
    let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
    settings_value.ui_prefs = prefs.clone();
    match settings::save_settings(&state.settings_path, &settings_value) {
        Ok(_) => {
            println!("[DEBUG] settings successfully saved to disk: {:?}", state.settings_path);
            // Verify the file was actually written
            if let Ok(content) = std::fs::read_to_string(&state.settings_path) {
                println!("[DEBUG] Verified file content: {}", content);
            }
        }
        Err(e) => println!("[DEBUG] settings save FAILED: {}", e),
    }
    let _ = app.emit("ui-prefs-changed", prefs);
    Ok(settings_value.clone())
}

#[tauri::command]
pub fn show_app_window(app: AppHandle) {
    crate::show_main_window(&app);
}

#[tauri::command]
pub fn show_overlay(app: AppHandle) {
    crate::show_overlay_window(&app);
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn set_chrome_dark(app: AppHandle, dark: bool) {
    for label in ["main", "overlay"] {
        if let Some(window) = app.get_webview_window(label) {
            crate::set_window_dark_mode(&window, dark);
        }
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<AppState>) {
    if let Ok(mut resolver) = state.app_name_resolver.lock() {
        let _ = resolver.save_if_dirty();
    }
    app.exit(0);
}

#[tauri::command]
pub fn set_hotkey(app: tauri::AppHandle, state: State<AppState>, hotkey: String) -> Result<AppSettings, String> {
    update_hotkey(app, state, hotkey)
}

#[tauri::command]
pub fn reset_hotkey(app: tauri::AppHandle, state: State<AppState>) -> Result<AppSettings, String> {
    update_hotkey(app, state, settings::DEFAULT_HOTKEY.into())
}

fn update_hotkey(app: tauri::AppHandle, state: State<AppState>, hotkey: String) -> Result<AppSettings, String> {
    #[cfg(not(desktop))]
    {
        let _ = app;
        let _ = state;
        let _ = hotkey;
        return Err("Global shortcuts are not supported on this platform".into());
    }

    #[cfg(desktop)]
    {
        let parsed = Shortcut::try_from(hotkey.as_str()).map_err(|err| err.to_string())?;
        let normalized = parsed.into_string();

        let mut settings_value = state.settings.lock().map_err(|err| err.to_string())?;
        if settings_value.hotkey == normalized {
            return Ok(settings_value.clone());
        }

        app.global_shortcut()
            .register(parsed)
            .map_err(|err| format!("Failed to register shortcut: {err}"))?;

        if let Err(err) = app.global_shortcut().unregister(settings_value.hotkey.as_str()) {
            let _ = app.global_shortcut().unregister(parsed);
            return Err(format!("Failed to remove previous shortcut: {err}"));
        }

        let previous_hotkey = settings_value.hotkey.clone();
        settings_value.hotkey = normalized;

        if let Err(err) = settings::save_settings(&state.settings_path, &settings_value) {
            let _ = app.global_shortcut().register(previous_hotkey.as_str());
            let _ = app.global_shortcut().unregister(parsed);
            settings_value.hotkey = previous_hotkey;
            return Err(err);
        }

        Ok(settings_value.clone())
    }
}

pub fn present_app_total(
    resolver: &mut crate::app_names::AppNameResolver,
    mut app: AppTotal,
) -> AppTotal {
    if looks_like_exe_path(&app.app_identity) || app.app_identity == "idle" || app.app_identity == "unknown" {
        app.app_name = resolver.resolve_app_name(&app.app_identity).app_name;
    }
    app
}

fn present_goal_label(
    resolver: &mut crate::app_names::AppNameResolver,
    target_kind: &str,
    target_value: &str,
) -> String {
    if target_kind == "total" || target_value == db::TOTAL_GOAL_TARGET {
        return "Total screen time".into();
    }

    if looks_like_exe_path(target_value) {
        return resolver.resolve_app_name(target_value).app_name;
    }

    target_value.to_string()
}

fn normalize_goal_draft(mut draft: GoalDraft) -> Result<GoalDraft, String> {
    draft.target_kind = draft.target_kind.trim().to_lowercase();
    draft.target_value = draft.target_value.trim().to_string();
    draft.warn_seconds = draft.warn_seconds.filter(|value| *value > 0);
    draft.annoy_seconds = draft.annoy_seconds.filter(|value| *value > 0);

    if draft.target_kind != "app" && draft.target_kind != "total" {
        return Err("Goal type must be app or total".into());
    }

    if draft.warn_seconds.is_none() && draft.annoy_seconds.is_none() {
        return Err("Set at least one threshold".into());
    }

    if draft.target_kind == "total" {
        draft.target_value = db::TOTAL_GOAL_TARGET.into();
    } else if draft.target_value.is_empty() {
        return Err("Choose an app for this goal".into());
    }

    Ok(draft)
}
