#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_names;
mod commands;
mod db;
mod focus;
mod focus_store;
mod models;
mod recurrence;
mod scheduler;
mod settings;
mod stats;
mod timetable;
mod todos;
mod tracker;

use app_names::{default_cache_path, AppNameResolver};
use db::{default_db_path, open_shared_database, SharedDb};
use models::{AppSettings, GoalAlertPayload};
use settings::{default_settings_path, load_settings, save_settings, DEFAULT_HOTKEY};
use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    Emitter,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent,
};
use windows_core::Interface;


#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub db: SharedDb,
    pub settings: Arc<Mutex<AppSettings>>,
    pub settings_path: PathBuf,
    pub app_name_resolver: Arc<Mutex<AppNameResolver>>,
    pub limit_runtime: Arc<Mutex<tracker::LimitRuntime>>,
    pub pending_alerts: Arc<Mutex<std::collections::HashMap<String, GoalAlertPayload>>>,
    pub focus: Arc<Mutex<focus::FocusEngine>>,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_today_totals,
            commands::get_week_dashboard,
            commands::get_hourly_today,
            stats::get_usage_range,
            stats::get_app_history,
            todos::get_todos,
            todos::save_todo_item,
            todos::set_todo_completed,
            todos::delete_todo_item,
            todos::get_todo_summary,
            todos::get_todo_templates,
            todos::save_todo_template,
            todos::set_todo_template_active,
            todos::delete_todo_template,
            timetable::get_timetable,
            timetable::get_timetable_entries,
            timetable::save_timetable_entry,
            timetable::delete_timetable_entry,
            timetable::skip_timetable_occurrence,
            timetable::edit_timetable_occurrence,
            timetable::restore_timetable_occurrence,
            timetable::schedule_todo_block,
            timetable::unschedule_todo_blocks,
            timetable::get_free_slots,
            focus_store::get_focus_presets,
            focus_store::save_focus_preset,
            focus_store::delete_focus_preset,
            focus_store::get_focus_state,
            focus_store::start_focus,
            focus_store::pause_focus,
            focus_store::resume_focus,
            focus_store::skip_focus_phase,
            focus_store::stop_focus,
            focus_store::add_focus_time,
            focus_store::get_focus_stats,
            focus_store::get_focus_history,
            commands::get_goals,
            commands::get_goal_candidates,
            commands::get_settings,
            commands::get_storage_location,
            commands::save_goal,
            commands::delete_goal,
            commands::snooze_goal,
            commands::set_hotkey,
            commands::reset_hotkey,
            commands::set_blur_percent,
            commands::reset_blur_percent,
            commands::set_material,
            commands::reset_material,
            commands::set_exe_labels,
            commands::set_idle_timeout,
            commands::set_ui_prefs,
            commands::show_app_window,
            commands::show_overlay,
            commands::hide_overlay,
            commands::set_chrome_dark,
            commands::quit_app,
            commands::get_pending_alert,
            commands::clear_pending_alert,
            commands::set_warn_clickthrough,
        ])
        .on_window_event(handle_window_event)
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            println!("[DEBUG] app_data_dir = {:?}", data_dir);
            let db = open_shared_database(default_db_path(&data_dir))
                .expect("failed to initialize database");
            let settings_path = default_settings_path(&data_dir);
            println!("[DEBUG] settings_path = {:?}", settings_path);
            let mut settings = load_settings(&settings_path).expect("failed to load settings");
            println!("[DEBUG] Loaded settings ui_prefs: {}", settings.ui_prefs);
            let settings_state = Arc::new(Mutex::new(settings.clone()));
            let resolver = Arc::new(Mutex::new(
                AppNameResolver::new(default_cache_path(&data_dir), settings.exe_labels.clone())
                    .expect("failed to initialize app name resolver"),
            ));
            let limit_runtime = Arc::new(Mutex::new(tracker::LimitRuntime::default()));
            let pending_alerts = Arc::new(Mutex::new(std::collections::HashMap::new()));
            let tracker_db = db.clone();
            let scheduler_db = db.clone();
            let focus_engine = Arc::new(Mutex::new(focus::FocusEngine::default()));

            #[cfg(desktop)]
            {
                if settings.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>().is_err() {
                    settings.hotkey = DEFAULT_HOTKEY.into();
                    save_settings(&settings_path, &settings).expect("failed to repair settings");
                }
            }

            app.manage(AppState {
                db,
                settings: settings_state.clone(),
                settings_path: settings_path.clone(),
                app_name_resolver: resolver.clone(),
                limit_runtime: limit_runtime.clone(),
                pending_alerts: pending_alerts.clone(),
                focus: focus_engine.clone(),
            });

            // Create windows from config AFTER state is managed, so state exists before any JS runs
            let main_window = WebviewWindowBuilder::from_config(app, &app.config().app.windows.iter().find(|w| w.label == "main").cloned().expect("main window config"))?
                .build()?;
            let overlay_window = WebviewWindowBuilder::from_config(app, &app.config().app.windows.iter().find(|w| w.label == "overlay").cloned().expect("overlay window config"))?
                .build()?;
            let warn_window = WebviewWindowBuilder::from_config(app, &app.config().app.windows.iter().find(|w| w.label == "warn").cloned().expect("warn window config"))?
                .build()?;
            let annoy_window = WebviewWindowBuilder::from_config(app, &app.config().app.windows.iter().find(|w| w.label == "annoy").cloned().expect("annoy window config"))?
                .build()?;

            // Hide before applying glass effects — vibrancy calls briefly force
            // the window visible via DWM compositor redraw on Windows
            let _ = main_window.hide();
            let _ = overlay_window.hide();
            let _ = warn_window.hide();
            let _ = annoy_window.hide();
            apply_window_glass(&main_window);
            apply_window_glass(&overlay_window);
            apply_window_glass(&warn_window);
            apply_window_glass(&annoy_window);

            // Enable WebView2 DevTools for debugging
            let windows = [&main_window, &overlay_window, &warn_window, &annoy_window];
            for window in windows {
                let _ = window.with_webview(|webview| {
                    unsafe {
                        if let Ok(controller) = webview.controller().cast::<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller>() {
                            if let Ok(settings) = controller.cast::<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings>() {
                                let _ = settings.SetAreDevToolsEnabled(true);
                            }
                        }
                    }
                });
            }

            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    use tauri::Manager;
                    use std::collections::HashMap;
                    let mut was_visible: HashMap<&str, bool> = HashMap::new();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        for label in ["main", "overlay", "warn", "annoy"] {
                            if let Some(w) = app_handle.get_webview_window(label) {
                                let visible = w.is_visible().unwrap_or(false);
                                let prev = *was_visible.get(label).unwrap_or(&false);
                                if visible && !prev {
                                    // hidden -> visible: force a recomposite
                                    kick_transient_acrylic(&w);
                                } else if visible {
                                    // stay glass, idempotent, no flicker
                                    apply_transient_acrylic(&w);
                                }
                                was_visible.insert(label, visible);
                            }
                        }
                    }
                });
            }
            configure_overlay_window(&overlay_window)?;
            configure_warn_window(&warn_window)?;
            configure_alert_window(&annoy_window)?;
            setup_tray(app.handle())?;
            setup_global_shortcut(app.handle(), &settings.hotkey)?;
            tracker::start_tracker(
                tracker_db,
                resolver,
                limit_runtime,
                pending_alerts,
                settings_state.clone(),
                app.handle().clone(),
            );
            scheduler::start_scheduler(scheduler_db, focus_engine, app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(desktop)]
fn setup_global_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), Box<dyn Error>> {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    toggle_overlay(app);
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(shortcut)?;
    Ok(())
}

#[cfg(not(desktop))]
fn setup_global_shortcut(_app: &AppHandle, _shortcut: &str) -> Result<(), Box<dyn Error>> {
    Ok(())
}

/// Milliseconds since the epoch; only used to debounce overlay show/hide.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

static OVERLAY_SHOWN_AT: AtomicU64 = AtomicU64::new(0);
static OVERLAY_BLUR_HID_AT: AtomicU64 = AtomicU64::new(0);

/// Shows the full app window (onboarding, dashboards, settings).
pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }

    apply_window_glass(&window);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

pub fn show_overlay_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("overlay") else {
        return;
    };

    let _ = window.center();
    apply_window_glass(&window);
    OVERLAY_SHOWN_AT.store(now_millis(), Ordering::SeqCst);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("overlay-shown", ());
}

fn toggle_overlay(app: &AppHandle) {
    let Some(window) = app.get_webview_window("overlay") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    // A tray click first steals focus (which hides the overlay) and then
    // arrives as a toggle; swallow that second half so it doesn't reopen.
    if now_millis().saturating_sub(OVERLAY_BLUR_HID_AT.load(Ordering::SeqCst)) < 350 {
        return;
    }

    show_overlay_window(app);
}

fn handle_window_event(window: &Window, event: &WindowEvent) {
    let label = window.label().to_string();

    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if label == "annoy" {
                let _ = window.emit("annoy-close-requested", ());
            } else {
                let _ = window.hide();
            }
        }
        WindowEvent::Focused(false) => {
            if label == "overlay" {
                // Focus can flicker right after show(); ignore that window.
                if now_millis().saturating_sub(OVERLAY_SHOWN_AT.load(Ordering::SeqCst)) > 600 {
                    OVERLAY_BLUR_HID_AT.store(now_millis(), Ordering::SeqCst);
                    let _ = window.hide();
                }
            }
        }
        _ => {}
    }
}

fn configure_overlay_window(window: &WebviewWindow) -> Result<(), Box<dyn Error>> {
    window.set_resizable(false)?;
    window.set_always_on_top(true)?;
    window.set_skip_taskbar(true)?;
    Ok(())
}

fn configure_warn_window(window: &WebviewWindow) -> Result<(), Box<dyn Error>> {
    window.set_resizable(false)?;
    window.set_always_on_top(true)?;
    window.set_skip_taskbar(true)?;
    window.set_focusable(false)?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

fn configure_alert_window(window: &WebviewWindow) -> Result<(), Box<dyn Error>> {
    if let Some(monitor) = window.current_monitor()? {
        let monitor_size = monitor.size();
        let monitor_position = monitor.position();

        window.set_position(PhysicalPosition::new(
            monitor_position.x,
            monitor_position.y,
        ))?;
        window.set_size(PhysicalSize::new(monitor_size.width, monitor_size.height))?;
    }

    window.set_fullscreen(true)?;
    window.set_resizable(false)?;
    window.set_always_on_top(true)?;
    window.set_skip_taskbar(true)?;
    window.set_focusable(true)?;
    window.set_ignore_cursor_events(false)?;
    Ok(())
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn Error>> {
    let tray_icon = app
        .default_window_icon()
        .cloned()
        .ok_or("missing default window icon")?;

    // No native menu: left-click toggles the quick overlay, right-click opens
    // the full app (which has its own quit action).
    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .tooltip("TrackMLN")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                match button {
                    MouseButton::Left => toggle_overlay(tray.app_handle()),
                    MouseButton::Right => show_main_window(tray.app_handle()),
                    _ => {}
                }
            }
        })
        .build(app)?;

    Ok(())
}

    // Tint the acrylic glass is stained with: (R, G, B, A).
    // Matches TrackMLN's dark surface tone. Tune here for control.


    fn apply_window_glass(window: &WebviewWindow) {
        #[cfg(target_os = "windows")]
        {
            // Force WebView2's default background alpha to 0 so the acrylic
            // shows through instead of a flat opaque fill.
            let _ = window.with_webview(|webview| {
                use webview2_com::Microsoft::Web::WebView2::Win32::{
                    COREWEBVIEW2_COLOR, ICoreWebView2Controller2,
                };
                use windows_core::Interface;
                unsafe {
                    if let Ok(controller2) = webview
                        .controller()
                        .cast::<ICoreWebView2Controller2>()
                    {
                        let _ = controller2.SetDefaultBackgroundColor(
                            COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 },
                        );
                    }
                }
            });

            apply_transient_acrylic(window);
        }
    }

    #[cfg(target_os = "windows")]
    fn apply_transient_acrylic(window: &WebviewWindow) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DwmExtendFrameIntoClientArea,
            DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW,
            DWMWA_WINDOW_CORNER_PREFERENCE,
        };
        use windows::Win32::UI::Controls::MARGINS;

        let Ok(raw) = window.hwnd() else { return };
        // Reconstruct HWND with THIS crate's windows version to avoid a type
        // mismatch with Tauri's re-exported HWND. If `HWND(raw.0)` fails to
        // compile, use `HWND(raw.0 as _)`.
        let hwnd = HWND(raw.0);

        let backdrop = DWMSBT_TRANSIENTWINDOW;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&backdrop) as u32,
            );

            // -1 margins = extend the glass across the whole client area, required
            // for the system backdrop to paint on a borderless/transparent window.
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

            // 2 = DWMWCP_ROUND: keep the acrylic clipped to rounded corners.
            let corner: i32 = 2;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// Switches the system backdrop tint between its dark and light variants so
    /// the translucent sidebar matches the chosen theme.
    #[cfg(target_os = "windows")]
    pub fn set_window_dark_mode(window: &WebviewWindow, dark: bool) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};

        let Ok(raw) = window.hwnd() else { return };
        let hwnd = HWND(raw.0);
        let value: i32 = if dark { 1 } else { 0 };
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn set_window_dark_mode(_window: &WebviewWindow, _dark: bool) {}

    #[cfg(target_os = "windows")]
    fn kick_transient_acrylic(window: &WebviewWindow) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMSBT_NONE,
        };

        let Ok(raw) = window.hwnd() else { return };
        let hwnd = HWND(raw.0); // use `raw.0 as _` if this doesn't compile

        unsafe {
            // Force a real state change so DWM recomposites the backdrop.
            let none = DWMSBT_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &none as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&none) as u32,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        // Re-apply the real backdrop (+ frame extension) via the existing fn.
        apply_transient_acrylic(window);
    }

    #[cfg(not(target_os = "windows"))]
    fn kick_transient_acrylic(_window: &WebviewWindow) {}
