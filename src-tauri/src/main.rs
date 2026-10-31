#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_names;
mod commands;
mod db;
mod models;
mod recurrence;
mod settings;
mod stats;
mod todos;
mod tracker;

use app_names::{default_cache_path, AppNameResolver};
use db::{default_db_path, open_shared_database, SharedDb};
use models::{AppSettings, GoalAlertPayload};
use settings::{default_settings_path, load_settings, save_settings, DEFAULT_HOTKEY};
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    Emitter,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window, WindowEvent,
};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub db: SharedDb,
    pub settings: Arc<Mutex<AppSettings>>,
    pub settings_path: PathBuf,
    pub app_name_resolver: Arc<Mutex<AppNameResolver>>,
    pub limit_runtime: Arc<Mutex<tracker::LimitRuntime>>,
    pub pending_alerts: Arc<Mutex<std::collections::HashMap<String, GoalAlertPayload>>>,
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
            let db = open_shared_database(default_db_path(&data_dir))
                .expect("failed to initialize database");
            let settings_path = default_settings_path(&data_dir);
            let mut settings = load_settings(&settings_path).expect("failed to load settings");
            let settings_state = Arc::new(Mutex::new(settings.clone()));
            let resolver = Arc::new(Mutex::new(
                AppNameResolver::new(default_cache_path(&data_dir), settings.exe_labels.clone())
                    .expect("failed to initialize app name resolver"),
            ));
            let limit_runtime = Arc::new(Mutex::new(tracker::LimitRuntime::default()));
            let pending_alerts = Arc::new(Mutex::new(std::collections::HashMap::new()));
            let tracker_db = db.clone();
            let main_window = app
                .get_webview_window("main")
                .expect("main window should exist");
            let warn_window = app
                .get_webview_window("warn")
                .expect("warn window should exist");
            let annoy_window = app
                .get_webview_window("annoy")
                .expect("annoy window should exist");

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
            });
            // Hide before applying glass effects — vibrancy calls briefly force
            // the window visible via DWM compositor redraw on Windows
            let _ = main_window.hide();
            let _ = warn_window.hide();
            let _ = annoy_window.hide();
            apply_window_glass(&main_window);
            apply_window_glass(&warn_window);
            apply_window_glass(&annoy_window);
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    use tauri::Manager;
                    use std::collections::HashMap;
                    let mut was_visible: HashMap<&str, bool> = HashMap::new();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        for label in ["main", "warn", "annoy"] {
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
            configure_main_window(&main_window)?;
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
                    toggle_main_window(app);
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

fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let is_visible = window.is_visible().unwrap_or(false);
    let is_minimized = window.is_minimized().unwrap_or(false);

    if is_visible && !is_minimized {
        let _ = window.hide();
    } else {
        let _ = configure_main_window(&window);
        apply_window_glass(&window);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn handle_window_event(window: &Window, event: &WindowEvent) {
    let label = window.label().to_string();

    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if label == "main" {
                let _ = window.minimize();
            } else if label == "annoy" {
                let _ = window.emit("annoy-close-requested", ());
            } else {
                let _ = window.hide();
            }
        }
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            if label == "main" {
                let _ = configure_window(window);
            }
        }
        _ => {}
    }
}

fn configure_main_window(window: &WebviewWindow) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

fn configure_window(window: &Window) -> Result<(), Box<dyn Error>> {
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
    let open = MenuItemBuilder::with_id("open", "Open").build(app)?;
    let close = MenuItemBuilder::with_id("close", "Close").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&open, &close]).build()?;
    let tray_icon = app
        .default_window_icon()
        .cloned()
        .ok_or("missing default window icon")?;
    let app_handle = app.clone();

    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => toggle_main_window(app),
            "close" => {
                let state = app.state::<AppState>();
                if let Ok(mut resolver) = state.app_name_resolver.lock() {
                    let _ = resolver.save_if_dirty();
                }
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
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
        }
    }

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
