use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Local};
use crate::app_names::{AppNameResolver, ResolvedApp};
use crate::models::{AppSettings, Goal, GoalAlertPayload};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowRect, GetWindowThreadProcessId, MSG, RegisterClassW, MB_ICONERROR,
    MB_ICONWARNING,
    WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, POWERBROADCAST_SETTING,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::SystemServices::GUID_MONITOR_POWER_ON;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use crate::db::{self, SharedDb};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
// A tick further apart than this means the machine slept or the process stalled.
const MAX_TICK_GAP_SECONDS: i64 = 4;
// Long sessions are written out this often so a crash loses at most this much.
const CHECKPOINT_SECONDS: i64 = 60;
const ANNOY_REPEAT_INTERVAL_MINUTES: i64 = 10;
const SNOOZE_DURATIONS_MINUTES: [i64; 3] = [5, 3, 1];

// Shared flag — true = screen is on, false = screen is off
static SCREEN_ON: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Default)]
pub struct GoalRuntimeState {
    pub warn_sent_on: Option<String>,
    pub annoy_shown_on: Option<String>,
    pub last_annoy_notification_at: Option<i64>,
    pub snoozed_until: Option<i64>,
}

#[derive(Debug, Default)]
pub struct LimitRuntime {
    pub goal_states: HashMap<i64, GoalRuntimeState>,
    pub snooze_counts: HashMap<i64, u32>,
}

impl LimitRuntime {
    pub fn snooze(&mut self, goal_id: i64, minutes: i64) {
        let state = self.goal_states.entry(goal_id).or_default();
        state.snoozed_until = Some(Local::now().timestamp() + minutes * 60);
        state.annoy_shown_on = None;
        state.last_annoy_notification_at = None;
        *self.snooze_counts.entry(goal_id).or_insert(0) += 1;
    }
}

fn snooze_minutes_for_count(snooze_count: u32) -> i64 {
    let index = (snooze_count as usize).min(SNOOZE_DURATIONS_MINUTES.len() - 1);
    SNOOZE_DURATIONS_MINUTES[index]
}

pub fn start_tracker(
    db: SharedDb,
    app_name_resolver: Arc<Mutex<AppNameResolver>>,
    limit_runtime: Arc<Mutex<LimitRuntime>>,
    pending_alerts: Arc<Mutex<HashMap<String, GoalAlertPayload>>>,
    settings: Arc<Mutex<AppSettings>>,
    app_handle: AppHandle,
) {
    // Spin up the power monitor on its own thread (needs its own message loop)
    thread::spawn(|| {
        run_power_monitor();
    });

    thread::spawn(move || {
        let mut tracker = Tracker::new(
            db,
            app_name_resolver,
            limit_runtime,
            pending_alerts,
            settings,
            app_handle,
        );
        tracker.run();
    });
}

// ── Power monitor ────────────────────────────────────────────────────────────

fn run_power_monitor() {
    unsafe {
        // Register a minimal hidden window class just to receive messages
        let class_name: Vec<u16> = "ScreenMonitor\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(power_wnd_proc),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_OVERLAPPEDWINDOW,
            0, 0, 0, 0,
            None,
            None,
            None,
            None,
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(_) => return,
        };

        // Subscribe to monitor power events
        RegisterPowerSettingNotification(
            windows::Win32::Foundation::HANDLE(hwnd.0),
            &GUID_MONITOR_POWER_ON,
            windows::Win32::UI::WindowsAndMessaging::REGISTER_NOTIFICATION_FLAGS(0),
        )
        .ok();

        // Standard Win32 message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn power_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_POWERBROADCAST: u32 = 0x0218;
    const PBT_POWERSETTINGCHANGE: usize = 0x8013;

    if msg == WM_POWERBROADCAST && wparam.0 == PBT_POWERSETTINGCHANGE {
        let setting = &*(lparam.0 as *const POWERBROADCAST_SETTING);
        if setting.PowerSetting == GUID_MONITOR_POWER_ON {
            // Data is a DWORD: 0 = off, 1 = on, 2 = dimmed (treat dimmed as on)
            let state = *(setting.Data.as_ptr() as *const u32);
            let is_on = state != 0;
            SCREEN_ON.store(is_on, Ordering::Relaxed);
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Tracker ──────────────────────────────────────────────────────────────────

struct Tracker {
    db: SharedDb,
    app_name_resolver: Arc<Mutex<AppNameResolver>>,
    limit_runtime: Arc<Mutex<LimitRuntime>>,
    pending_alerts: Arc<Mutex<HashMap<String, GoalAlertPayload>>>,
    settings: Arc<Mutex<AppSettings>>,
    app_handle: AppHandle,
    current_identity: Option<String>,
    current_exe_name: Option<String>,
    current_app: Option<String>,
    session_start: Option<DateTime<Local>>,
    last_tick: DateTime<Local>,
}

impl Tracker {
    fn new(
        db: SharedDb,
        app_name_resolver: Arc<Mutex<AppNameResolver>>,
        limit_runtime: Arc<Mutex<LimitRuntime>>,
        pending_alerts: Arc<Mutex<HashMap<String, GoalAlertPayload>>>,
        settings: Arc<Mutex<AppSettings>>,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            db,
            app_name_resolver,
            limit_runtime,
            pending_alerts,
            settings,
            app_handle,
            current_identity: None,
            current_exe_name: None,
            current_app: None,
            session_start: None,
            last_tick: Local::now(),
        }
    }

    fn run(&mut self) {
        loop {
            if let Err(err) = self.tick() {
                eprintln!("tracker tick failed: {err}");
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn tick(&mut self) -> Result<(), String> {
        let now = Local::now();
        let previous_tick = self.last_tick;
        let gap = now.timestamp() - previous_tick.timestamp();
        self.last_tick = now;

        if gap > MAX_TICK_GAP_SECONDS {
            // Slept or stalled: credit the app only up to the last tick we saw.
            self.flush_current_at(previous_tick)?;
        } else if gap < 0 {
            // Clock moved backwards; close the session rather than stretch it.
            self.flush_current_at(now)?;
        }

        // Screen is off — flush whatever was running and skip this tick
        if !SCREEN_ON.load(Ordering::Relaxed) {
            self.flush_current_at(now)?;
            return Ok(());
        }

        // No keyboard/mouse input for longer than the idle timeout — stop counting
        // from the moment the threshold was crossed, not from when we noticed.
        if let Some(excess_seconds) = self.idle_excess_seconds() {
            self.flush_current_at(now - ChronoDuration::seconds(excess_seconds))?;
            return Ok(());
        }

        let resolved = get_active_window(&self.app_name_resolver);

        if self.current_identity.as_deref() != Some(resolved.identity.as_str()) {
            self.flush_current_at(now)?;
            self.current_identity = Some(resolved.identity.clone());
            self.current_exe_name = Some(resolved.exe_name.clone());
            self.current_app = Some(resolved.app_name.clone());
            self.session_start = Some(now);
        }

        self.checkpoint_if_due(now)?;
        self.check_limits()?;
        self
            .app_name_resolver
            .lock()
            .map_err(|err| err.to_string())?
            .flush_if_due()?;
        Ok(())
    }

    /// Seconds past the idle threshold the user has been inactive, or `None` if
    /// they are active, idle detection is off, or a fullscreen window (video,
    /// game) is in front — watching something without touching input is not idle.
    fn idle_excess_seconds(&self) -> Option<i64> {
        let timeout = self
            .settings
            .lock()
            .map(|settings| settings.idle_timeout_seconds)
            .unwrap_or(0);
        if timeout == 0 {
            return None;
        }

        let idle = system_idle_seconds()?;
        if idle < timeout || foreground_is_fullscreen() {
            return None;
        }
        Some((idle - timeout) as i64)
    }

    /// Writes the running session so far and restarts it from `now`, keeping the
    /// same app. Adjacent rows are merged in `db::log_session`.
    fn checkpoint_if_due(&mut self, now: DateTime<Local>) -> Result<(), String> {
        let Some(start) = self.session_start else {
            return Ok(());
        };
        if now.timestamp() - start.timestamp() < CHECKPOINT_SECONDS {
            return Ok(());
        }
        let Some(app_name) = self.current_app.clone() else {
            return Ok(());
        };

        let connection = self.db.lock().map_err(|err| err.to_string())?;
        db::log_session(
            &connection,
            self.current_identity.as_deref(),
            self.current_exe_name.as_deref(),
            &app_name,
            start,
            now,
        )
        .map_err(|err| err.to_string())?;
        drop(connection);

        self.session_start = Some(now);
        Ok(())
    }

    fn flush_current_at(&mut self, end: DateTime<Local>) -> Result<(), String> {
        let Some(app_name) = self.current_app.clone() else {
            self.current_identity = None;
            self.current_exe_name = None;
            self.session_start = None;
            return Ok(());
        };
        let Some(start) = self.session_start else {
            self.current_identity = None;
            self.current_exe_name = None;
            self.current_app = None;
            return Ok(());
        };

        let connection = self.db.lock().map_err(|err| err.to_string())?;
        db::log_session(
            &connection,
            self.current_identity.as_deref(),
            self.current_exe_name.as_deref(),
            &app_name,
            start,
            end,
        )
        .map_err(|err| err.to_string())?;

        self.current_identity = None;
        self.current_exe_name = None;
        self.current_app = None;
        self.session_start = None;
        Ok(())
    }

    fn check_limits(&mut self) -> Result<(), String> {
        let connection = self.db.lock().map_err(|err| err.to_string())?;
        let goals = db::get_goals(&connection).map_err(|err| err.to_string())?;
        let persisted_total = db::get_today_total(&connection).map_err(|err| err.to_string())?;
        let now = Local::now();
        let today_key = now.format("%Y-%m-%d").to_string();
        let live_total = self.live_elapsed_for_total(now);
        let current_identity = self.current_identity.clone();
        let current_app_name = self.current_app.clone();
        let mut goal_totals = HashMap::new();

        for goal in &goals {
            let total = if goal.target_kind == "total" {
                persisted_total + live_total
            } else {
                let mut value = db::get_today_total_for_identity(&connection, &goal.target_value)
                    .map_err(|err| err.to_string())?;
                if current_identity.as_deref() == Some(goal.target_value.as_str()) {
                    value += self.live_elapsed_for_current(now);
                }
                value
            };
            goal_totals.insert(goal.id, total);
        }
        drop(connection);

        let mut runtime = self.limit_runtime.lock().map_err(|err| err.to_string())?;
        for goal in goals {
            // Normalize state day rollover without holding the state borrow while mutating snooze_counts
            let mut reset_snooze_on_day = false;
            {
                let state = runtime.goal_states.entry(goal.id).or_default();
                if state.warn_sent_on.as_deref() != Some(today_key.as_str()) {
                    // reset to None (cleared)
                    // note: leave as None
                    state.warn_sent_on = None;
                }
                if state.annoy_shown_on.as_deref() != Some(today_key.as_str()) {
                    state.annoy_shown_on = None;
                    state.last_annoy_notification_at = None;
                    reset_snooze_on_day = true;
                }
            }

            if reset_snooze_on_day {
                runtime.snooze_counts.insert(goal.id, 0);
            }

            let mut total = goal_totals.get(&goal.id).copied().unwrap_or(0);

            if goal.target_kind != "total" && goal.target_value == "idle" {
                total = 0;
            }

            // Prepare snooze_count early (copy) so we don't need to borrow runtime.snooze_counts later
            let snooze_count = runtime.snooze_counts.get(&goal.id).copied().unwrap_or(0);

            if let Some(warn_seconds) = goal.warn_seconds {
                if total >= warn_seconds {
                    // mutate state only in a scoped block
                    let mut sent = false;
                    {
                        let state = runtime.goal_states.entry(goal.id).or_default();
                        if state.warn_sent_on.is_none() {
                            state.warn_sent_on = Some(today_key.clone());
                            sent = true;
                        }
                    }

                    if sent {
                        self.show_warn_alert(
                            &goal,
                            total,
                            warn_seconds,
                            current_identity.as_deref(),
                            current_app_name.as_deref(),
                            snooze_count,
                        );
                    }
                }
            }

            let Some(annoy_seconds) = goal.annoy_seconds else {
                continue;
            };

            if total < annoy_seconds {
                {
                    let state = runtime.goal_states.entry(goal.id).or_default();
                    state.annoy_shown_on = None;
                    state.last_annoy_notification_at = None;
                    state.snoozed_until = None;
                }
                runtime.snooze_counts.insert(goal.id, 0);
                continue;
            }

            // check if snoozed
            let is_snoozed = runtime
                .goal_states
                .get(&goal.id)
                .and_then(|s| s.snoozed_until)
                .map(|until| until > now.timestamp())
                .unwrap_or(false);

            if is_snoozed {
                continue;
            }

            // first time annoy shown
            let mut shown_this_iteration = false;
            {
                let state = runtime.goal_states.entry(goal.id).or_default();
                if state.annoy_shown_on.is_none() {
                    state.annoy_shown_on = Some(today_key.clone());
                    state.last_annoy_notification_at = Some(now.timestamp());
                    shown_this_iteration = true;
                }
            }

            if shown_this_iteration {
                let snooze_count = runtime.snooze_counts.get(&goal.id).copied().unwrap_or(0);
                self.show_annoy_alert(
                    &goal,
                    total,
                    annoy_seconds,
                    current_identity.as_deref(),
                    current_app_name.as_deref(),
                    snooze_count,
                );
                continue;
            }

            let should_repeat = runtime
                .goal_states
                .get(&goal.id)
                .and_then(|s| s.last_annoy_notification_at)
                .map(|timestamp| now.timestamp() - timestamp >= ANNOY_REPEAT_INTERVAL_MINUTES * 60)
                .unwrap_or(true);

            if should_repeat {
                {
                    let state = runtime.goal_states.entry(goal.id).or_default();
                    state.last_annoy_notification_at = Some(now.timestamp());
                }
                let snooze_count = runtime.snooze_counts.get(&goal.id).copied().unwrap_or(0);
                self.show_annoy_alert(
                    &goal,
                    total,
                    annoy_seconds,
                    current_identity.as_deref(),
                    current_app_name.as_deref(),
                    snooze_count,
                );
            }
        }

        Ok(())
    }

    fn live_elapsed_for_current(&self, now: DateTime<Local>) -> i64 {
        self.session_start
            .map(|start| (now.timestamp() - start.timestamp()).max(0))
            .unwrap_or(0)
    }

    fn live_elapsed_for_total(&self, now: DateTime<Local>) -> i64 {
        match self.current_identity.as_deref() {
            Some("idle") | Some("unknown") | None => 0,
            Some(_) => self.live_elapsed_for_current(now),
        }
    }

    fn build_goal_alert(
        &self,
        goal: &Goal,
        total_seconds: i64,
        threshold_seconds: i64,
        threshold: &str,
        current_identity: Option<&str>,
        current_app_name: Option<&str>,
        snooze_count: u32,
    ) -> GoalAlertPayload {
        let label = if goal.target_kind == "total" {
            "Total screen time".to_string()
        } else {
            current_app_name
                .filter(|_| current_identity == Some(goal.target_value.as_str()))
                .map(str::to_string)
                .unwrap_or_else(|| self.resolve_goal_label(goal.target_value.as_str()))
        };

        let payload = GoalAlertPayload {
            goal_id: goal.id,
            target_kind: goal.target_kind.clone(),
            target_value: goal.target_value.clone(),
            label,
            threshold: threshold.to_string(),
            total_seconds,
            threshold_seconds,
            repeat_minutes: snooze_minutes_for_count(snooze_count),
            show_overlay: threshold == "annoy",
            snooze_count,
        };
        payload
    }

    fn resolve_goal_label(&self, target_value: &str) -> String {
        self.app_name_resolver
            .lock()
            .ok()
            .map(|mut resolver| resolver.resolve_app_name(target_value).app_name)
            .unwrap_or_else(|| target_value.to_string())
    }

    fn show_warn_alert(
        &self,
        goal: &Goal,
        total_seconds: i64,
        threshold_seconds: i64,
        current_identity: Option<&str>,
        current_app_name: Option<&str>,
        snooze_count: u32,
    ) {
        let payload = self.build_goal_alert(
            goal,
            total_seconds,
            threshold_seconds,
            "warn",
            current_identity,
            current_app_name,
            snooze_count,
        );
        let Some(window) = self.app_handle.get_webview_window("warn") else {
            return;
        };

        self.store_pending_alert("warn", payload.clone());
        self.play_alert_sound("warn");
        self.position_warn_window(&window);
        let _ = window.show();
        let _ = window.emit("warn-alert", payload);
    }

    fn show_annoy_alert(
        &self,
        goal: &Goal,
        total_seconds: i64,
        threshold_seconds: i64,
        current_identity: Option<&str>,
        current_app_name: Option<&str>,
        snooze_count: u32,
    ) {
        let payload = self.build_goal_alert(
            goal,
            total_seconds,
            threshold_seconds,
            "annoy",
            current_identity,
            current_app_name,
            snooze_count,
        );
        let Some(window) = self.app_handle.get_webview_window("annoy") else {
            return;
        };

        self.store_pending_alert("annoy", payload.clone());
        self.play_alert_sound("annoy");
        self.position_annoy_window(&window);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("annoy-alert", payload);
    }

    fn store_pending_alert(&self, label: &str, payload: GoalAlertPayload) {
        if let Ok(mut pending_alerts) = self.pending_alerts.lock() {
            pending_alerts.insert(label.to_string(), payload);
        }
    }

    fn play_alert_sound(&self, threshold: &str) {
        unsafe {
            let tone = if threshold == "warn" { MB_ICONWARNING } else { MB_ICONERROR };
            let _ = MessageBeep(tone);
        }
    }

    fn position_warn_window(&self, window: &tauri::WebviewWindow) {
        let Some(area) = active_monitor_area() else {
            return;
        };

        let width = ((area.work.right - area.work.left) as f64 * 0.19).round().clamp(380.0, 500.0) as i32;
        let height = ((area.work.bottom - area.work.top) as f64 * 0.14).round().clamp(200.0, 260.0) as i32;
        let x = area.work.right - width - 26;
        let y = area.work.bottom - height - 26;

        eprintln!("position_warn_window: work={:?} w={} h={} x={} y={}", area.work, width, height, x, y);

        let _ = window.set_size(tauri::PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }

    fn position_annoy_window(&self, window: &tauri::WebviewWindow) {
        let Some(area) = active_monitor_area() else {
            return;
        };

        let width = (area.monitor.right - area.monitor.left).max(1);
        let height = (area.monitor.bottom - area.monitor.top).max(1);

        let _ = window.set_position(tauri::PhysicalPosition::new(area.monitor.left, area.monitor.top));
        let _ = window.set_size(tauri::PhysicalSize::new(width as u32, height as u32));
    }
}

// ── Win32 helpers ─────────────────────────────────────────────────────────────

fn get_active_window(app_name_resolver: &Arc<Mutex<AppNameResolver>>) -> ResolvedApp {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd == HWND(std::ptr::null_mut()) {
        return ResolvedApp {
            identity: "idle".into(),
            exe_name: "idle".into(),
            app_name: "Idle".into(),
        };
    }

    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }

    if process_id == 0 {
        return ResolvedApp {
            identity: "unknown".into(),
            exe_name: "unknown".into(),
            app_name: "Unknown".into(),
        };
    }

    match process_exe_path(process_id) {
        Some(path) => app_name_resolver
            .lock()
            .map(|mut resolver| resolver.resolve_app_name(&path))
            .unwrap_or_else(|_| ResolvedApp {
                identity: "unknown".into(),
                exe_name: "unknown".into(),
                app_name: "Unknown".into(),
            }),
        None => ResolvedApp {
            identity: "unknown".into(),
            exe_name: "unknown".into(),
            app_name: "Unknown".into(),
        },
    }
}

fn process_exe_path(process_id: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };
    let mut size = 260u32;
    let mut buffer = vec![0u16; size as usize];

    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };

    unsafe {
        let _ = CloseHandle(handle);
    }

    if result.is_err() {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..size as usize]))
}

fn system_idle_seconds() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    let ok = unsafe { GetLastInputInfo(&mut info) }.as_bool();
    if !ok {
        return None;
    }

    let now = unsafe { GetTickCount() };
    Some((now.wrapping_sub(info.dwTime) / 1000) as u64)
}

fn foreground_is_fullscreen() -> bool {
    let Some(area) = active_monitor_area() else {
        return false;
    };

    let hwnd = unsafe { GetForegroundWindow() };
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return false;
    }

    rect.left <= area.monitor.left
        && rect.top <= area.monitor.top
        && rect.right >= area.monitor.right
        && rect.bottom >= area.monitor.bottom
}

struct MonitorArea {
    monitor: RECT,
    work: RECT,
}

fn active_monitor_area() -> Option<MonitorArea> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd == HWND(std::ptr::null_mut()) {
        return None;
    }

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return None;
    }

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    let ok = unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFO) }.as_bool();
    if !ok {
        return None;
    }

    Some(MonitorArea {
        monitor: info.rcMonitor,
        work: info.rcWork,
    })
}
