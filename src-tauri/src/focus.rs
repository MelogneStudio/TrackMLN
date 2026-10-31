use serde::{Deserialize, Serialize};

const MAX_NAME_CHARS: usize = 60;
const MAX_PHASE_SECONDS: i64 = 4 * 60 * 60;

/// A saved focus configuration. Also used as the input when editing a preset or
/// starting an unsaved, one-off session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusPreset {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub focus_minutes: i64,
    pub short_break_minutes: i64,
    pub long_break_minutes: i64,
    /// After this many focus sessions the break is the long one.
    pub sessions_until_long_break: i64,
    /// Focus sessions in one run; the run ends after the last one (no trailing break).
    pub total_sessions: i64,
    #[serde(default = "default_true")]
    pub auto_start_breaks: bool,
    #[serde(default)]
    pub auto_start_focus: bool,
    #[serde(default)]
    pub is_default: bool,
}

fn default_true() -> bool {
    true
}

impl FocusPreset {
    pub fn validated(mut self) -> Result<FocusPreset, String> {
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err("Give the preset a name".into());
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(format!("Preset name is limited to {MAX_NAME_CHARS} characters"));
        }
        if !(1..=240).contains(&self.focus_minutes) {
            return Err("Focus length must be between 1 and 240 minutes".into());
        }
        if !(1..=120).contains(&self.short_break_minutes) {
            return Err("Short break must be between 1 and 120 minutes".into());
        }
        if !(1..=240).contains(&self.long_break_minutes) {
            return Err("Long break must be between 1 and 240 minutes".into());
        }
        if !(1..=20).contains(&self.sessions_until_long_break) {
            return Err("Sessions before a long break must be between 1 and 20".into());
        }
        if !(1..=50).contains(&self.total_sessions) {
            return Err("Number of sessions must be between 1 and 50".into());
        }
        Ok(self)
    }

    fn phase_seconds(&self, phase: FocusPhase) -> i64 {
        match phase {
            FocusPhase::Focus => self.focus_minutes * 60,
            FocusPhase::ShortBreak => self.short_break_minutes * 60,
            FocusPhase::LongBreak => self.long_break_minutes * 60,
        }
    }

    fn auto_starts(&self, phase: FocusPhase) -> bool {
        match phase {
            FocusPhase::Focus => self.auto_start_focus,
            FocusPhase::ShortBreak | FocusPhase::LongBreak => self.auto_start_breaks,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FocusPhase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FocusStatus {
    Running,
    Paused,
    /// The next phase is ready but waits for the user to start it.
    Waiting,
}

/// A finished or abandoned focus phase, ready to be stored.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusLog {
    pub started_at: i64,
    pub ended_at: i64,
    pub planned_seconds: i64,
    pub actual_seconds: i64,
    pub completed: bool,
    pub preset_id: Option<i64>,
    pub preset_name: String,
    pub todo_id: Option<i64>,
    pub entry_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusEvent {
    /// A focus phase ended and should be stored.
    Logged(FocusLog),
    /// A phase ended; `next` is `None` when the whole run is over.
    PhaseEnded {
        ended: FocusPhase,
        next: Option<FocusPhase>,
        completed: bool,
    },
    Finished {
        focus_sessions: i64,
    },
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusSnapshot {
    pub active: bool,
    pub phase: Option<FocusPhase>,
    pub status: Option<FocusStatus>,
    pub remaining_seconds: i64,
    pub phase_total_seconds: i64,
    /// Unix time the running phase ends; `None` while paused or waiting.
    pub phase_ends_at: Option<i64>,
    pub focus_completed: i64,
    pub total_sessions: i64,
    /// 1-based number of the focus session in progress (or just finished).
    pub current_session: i64,
    pub next_phase: Option<FocusPhase>,
    pub preset: Option<FocusPreset>,
    pub todo_id: Option<i64>,
    pub entry_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct ActiveSession {
    preset: FocusPreset,
    todo_id: Option<i64>,
    entry_id: Option<i64>,
    phase: FocusPhase,
    status: FocusStatus,
    focus_done: i64,
    phase_total: i64,
    phase_started_at: i64,
    phase_end_at: i64,
    remaining_when_idle: i64,
}

impl ActiveSession {
    fn remaining(&self, now: i64) -> i64 {
        match self.status {
            FocusStatus::Running => (self.phase_end_at - now).clamp(0, self.phase_total),
            FocusStatus::Paused | FocusStatus::Waiting => self.remaining_when_idle,
        }
    }

    fn next_phase(&self) -> Option<FocusPhase> {
        match self.phase {
            FocusPhase::Focus => {
                let completed_after_this = self.focus_done + 1;
                if completed_after_this >= self.preset.total_sessions {
                    None
                } else if completed_after_this % self.preset.sessions_until_long_break == 0 {
                    Some(FocusPhase::LongBreak)
                } else {
                    Some(FocusPhase::ShortBreak)
                }
            }
            FocusPhase::ShortBreak | FocusPhase::LongBreak => Some(FocusPhase::Focus),
        }
    }
}

/// Pomodoro-style timer. It only knows wall-clock seconds handed to it, so it
/// is deterministic to test and keeps correct time across sleep or a slow UI.
#[derive(Debug, Default)]
pub struct FocusEngine {
    active: Option<ActiveSession>,
}

impl FocusEngine {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn start(
        &mut self,
        preset: FocusPreset,
        todo_id: Option<i64>,
        entry_id: Option<i64>,
        now: i64,
    ) -> Result<(), String> {
        if self.active.is_some() {
            return Err("A focus session is already in progress".into());
        }

        let preset = preset.validated()?;
        let total = preset.phase_seconds(FocusPhase::Focus);
        self.active = Some(ActiveSession {
            preset,
            todo_id,
            entry_id,
            phase: FocusPhase::Focus,
            status: FocusStatus::Running,
            focus_done: 0,
            phase_total: total,
            phase_started_at: now,
            phase_end_at: now + total,
            remaining_when_idle: 0,
        });
        Ok(())
    }

    pub fn pause(&mut self, now: i64) -> Result<(), String> {
        let session = self.active.as_mut().ok_or("No focus session is in progress")?;
        match session.status {
            FocusStatus::Running => {
                session.remaining_when_idle = session.remaining(now);
                session.status = FocusStatus::Paused;
                Ok(())
            }
            FocusStatus::Paused => Ok(()),
            FocusStatus::Waiting => Err("The next phase has not started yet".into()),
        }
    }

    /// Continues a paused phase, or starts a phase that is waiting for the user.
    pub fn resume(&mut self, now: i64) -> Result<(), String> {
        let session = self.active.as_mut().ok_or("No focus session is in progress")?;
        match session.status {
            FocusStatus::Running => Ok(()),
            FocusStatus::Paused => {
                session.phase_end_at = now + session.remaining_when_idle;
                session.status = FocusStatus::Running;
                Ok(())
            }
            FocusStatus::Waiting => {
                session.phase_started_at = now;
                session.phase_end_at = now + session.remaining_when_idle;
                session.status = FocusStatus::Running;
                Ok(())
            }
        }
    }

    /// Lengthens (or shortens, if negative) the current phase.
    pub fn add_time(&mut self, now: i64, seconds: i64) -> Result<(), String> {
        let session = self.active.as_mut().ok_or("No focus session is in progress")?;
        let remaining = session.remaining(now);
        if remaining + seconds < 1 {
            return Err("That would end the phase; use skip instead".into());
        }
        if session.phase_total + seconds > MAX_PHASE_SECONDS {
            return Err("A phase cannot be longer than 4 hours".into());
        }

        session.phase_total += seconds;
        match session.status {
            FocusStatus::Running => session.phase_end_at += seconds,
            FocusStatus::Paused | FocusStatus::Waiting => session.remaining_when_idle += seconds,
        }
        Ok(())
    }

    /// Ends the current phase early and moves on to the next one.
    pub fn skip(&mut self, now: i64) -> Result<Vec<FocusEvent>, String> {
        if self.active.is_none() {
            return Err("No focus session is in progress".into());
        }
        Ok(self.advance(now, false))
    }

    /// Abandons the whole run, keeping a log of any focus time already spent.
    pub fn stop(&mut self, now: i64) -> Vec<FocusEvent> {
        let mut events = Vec::new();
        if let Some(session) = self.active.take() {
            if session.phase == FocusPhase::Focus {
                let elapsed = (session.phase_total - session.remaining(now)).clamp(0, session.phase_total);
                if elapsed > 0 {
                    events.push(FocusEvent::Logged(log_for(&session, now, elapsed, false)));
                }
            }
        }
        events
    }

    /// Advances the clock; call about once a second. Returns what happened.
    pub fn tick(&mut self, now: i64) -> Vec<FocusEvent> {
        let due = match &self.active {
            Some(session) => session.status == FocusStatus::Running && now >= session.phase_end_at,
            None => false,
        };
        if due {
            self.advance(now, true)
        } else {
            Vec::new()
        }
    }

    pub fn snapshot(&self, now: i64) -> FocusSnapshot {
        let Some(session) = &self.active else {
            return FocusSnapshot::default();
        };

        FocusSnapshot {
            active: true,
            phase: Some(session.phase),
            status: Some(session.status),
            remaining_seconds: session.remaining(now),
            phase_total_seconds: session.phase_total,
            phase_ends_at: if session.status == FocusStatus::Running {
                Some(session.phase_end_at)
            } else {
                None
            },
            focus_completed: session.focus_done,
            total_sessions: session.preset.total_sessions,
            current_session: if session.phase == FocusPhase::Focus {
                session.focus_done + 1
            } else {
                session.focus_done
            },
            next_phase: session.next_phase(),
            preset: Some(session.preset.clone()),
            todo_id: session.todo_id,
            entry_id: session.entry_id,
        }
    }

    fn advance(&mut self, now: i64, completed: bool) -> Vec<FocusEvent> {
        let mut events = Vec::new();
        let Some(session) = self.active.as_mut() else {
            return events;
        };

        let ended = session.phase;
        let elapsed = if completed {
            session.phase_total
        } else {
            (session.phase_total - session.remaining(now)).clamp(0, session.phase_total)
        };
        let ended_at = if completed { session.phase_end_at.min(now) } else { now };

        if ended == FocusPhase::Focus {
            session.focus_done += 1;
            if elapsed > 0 {
                events.push(FocusEvent::Logged(log_for(session, ended_at, elapsed, completed)));
            }
        }

        let next = session.next_phase_after_advance(ended);
        let Some(next) = next else {
            let sessions_done = session.focus_done;
            self.active = None;
            events.push(FocusEvent::PhaseEnded { ended, next: None, completed });
            events.push(FocusEvent::Finished { focus_sessions: sessions_done });
            return events;
        };

        let seconds = session.preset.phase_seconds(next);
        session.phase = next;
        session.phase_total = seconds;
        if session.preset.auto_starts(next) {
            session.status = FocusStatus::Running;
            session.phase_started_at = now;
            session.phase_end_at = now + seconds;
            session.remaining_when_idle = 0;
        } else {
            session.status = FocusStatus::Waiting;
            session.remaining_when_idle = seconds;
        }

        events.push(FocusEvent::PhaseEnded { ended, next: Some(next), completed });
        events
    }
}

impl ActiveSession {
    /// Like `next_phase`, but `focus_done` has already been incremented when the
    /// phase that just ended was a focus phase.
    fn next_phase_after_advance(&self, ended: FocusPhase) -> Option<FocusPhase> {
        match ended {
            FocusPhase::Focus => {
                if self.focus_done >= self.preset.total_sessions {
                    None
                } else if self.focus_done % self.preset.sessions_until_long_break == 0 {
                    Some(FocusPhase::LongBreak)
                } else {
                    Some(FocusPhase::ShortBreak)
                }
            }
            FocusPhase::ShortBreak | FocusPhase::LongBreak => Some(FocusPhase::Focus),
        }
    }
}

fn log_for(session: &ActiveSession, ended_at: i64, elapsed: i64, completed: bool) -> FocusLog {
    FocusLog {
        started_at: session.phase_started_at,
        ended_at,
        planned_seconds: session.phase_total,
        actual_seconds: elapsed,
        completed,
        preset_id: session.preset.id,
        preset_name: session.preset.name.clone(),
        todo_id: session.todo_id,
        entry_id: session.entry_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset() -> FocusPreset {
        FocusPreset {
            id: Some(1),
            name: "Test".into(),
            focus_minutes: 1,
            short_break_minutes: 1,
            long_break_minutes: 2,
            sessions_until_long_break: 2,
            total_sessions: 3,
            auto_start_breaks: true,
            auto_start_focus: true,
            is_default: false,
        }
    }

    fn phase_of(engine: &FocusEngine, now: i64) -> Option<FocusPhase> {
        engine.snapshot(now).phase
    }

    #[test]
    fn runs_a_full_cycle_with_a_long_break_after_two_sessions() {
        let mut engine = FocusEngine::default();
        engine.start(preset(), None, None, 0).unwrap();
        assert_eq!(phase_of(&engine, 0), Some(FocusPhase::Focus));
        assert!(engine.tick(59).is_empty());

        // focus 1 -> short break
        let events = engine.tick(60);
        assert!(matches!(events[0], FocusEvent::Logged(FocusLog { actual_seconds: 60, completed: true, .. })));
        assert_eq!(
            events[1],
            FocusEvent::PhaseEnded { ended: FocusPhase::Focus, next: Some(FocusPhase::ShortBreak), completed: true }
        );
        assert_eq!(phase_of(&engine, 60), Some(FocusPhase::ShortBreak));

        // short break -> focus 2 -> long break
        engine.tick(120);
        assert_eq!(phase_of(&engine, 120), Some(FocusPhase::Focus));
        assert_eq!(engine.snapshot(120).current_session, 2);
        engine.tick(180);
        let snapshot = engine.snapshot(180);
        assert_eq!(snapshot.phase, Some(FocusPhase::LongBreak));
        assert_eq!(snapshot.phase_total_seconds, 120);
        assert_eq!(snapshot.next_phase, Some(FocusPhase::Focus));

        // long break -> focus 3 (last) -> finished, no trailing break
        engine.tick(300);
        assert_eq!(engine.snapshot(300).next_phase, None);
        let events = engine.tick(360);
        assert_eq!(events.last(), Some(&FocusEvent::Finished { focus_sessions: 3 }));
        assert!(!engine.is_active());
        assert_eq!(engine.snapshot(360), FocusSnapshot::default());
    }

    #[test]
    fn pause_freezes_the_remaining_time() {
        let mut engine = FocusEngine::default();
        engine.start(preset(), None, None, 0).unwrap();

        engine.pause(20).unwrap();
        assert_eq!(engine.snapshot(500).remaining_seconds, 40);
        assert!(engine.tick(500).is_empty());

        engine.resume(500).unwrap();
        assert_eq!(engine.snapshot(500).remaining_seconds, 40);
        assert!(engine.tick(539).is_empty());
        assert!(!engine.tick(540).is_empty());
    }

    #[test]
    fn manual_start_waits_for_the_user() {
        let mut manual = preset();
        manual.auto_start_breaks = false;
        let mut engine = FocusEngine::default();
        engine.start(manual, None, None, 0).unwrap();

        engine.tick(60);
        let snapshot = engine.snapshot(1_000);
        assert_eq!(snapshot.phase, Some(FocusPhase::ShortBreak));
        assert_eq!(snapshot.status, Some(FocusStatus::Waiting));
        assert_eq!(snapshot.remaining_seconds, 60);
        assert!(engine.tick(5_000).is_empty());
        assert!(engine.pause(5_000).is_err());

        engine.resume(5_000).unwrap();
        assert_eq!(engine.snapshot(5_000).status, Some(FocusStatus::Running));
        assert!(engine.tick(5_059).is_empty());
        assert!(!engine.tick(5_060).is_empty());
    }

    #[test]
    fn skipping_and_stopping_log_partial_focus_time() {
        let mut engine = FocusEngine::default();
        engine.start(preset(), Some(7), Some(9), 0).unwrap();

        let events = engine.skip(25).unwrap();
        match &events[0] {
            FocusEvent::Logged(log) => {
                assert_eq!(log.actual_seconds, 25);
                assert_eq!(log.planned_seconds, 60);
                assert!(!log.completed);
                assert_eq!((log.todo_id, log.entry_id), (Some(7), Some(9)));
            }
            other => panic!("unexpected event {other:?}"),
        }
        assert_eq!(phase_of(&engine, 25), Some(FocusPhase::ShortBreak));

        // Stopping during a break logs nothing; stopping during focus logs the partial time.
        assert!(engine.stop(30).is_empty());
        assert!(!engine.is_active());

        engine.start(preset(), None, None, 100).unwrap();
        let events = engine.stop(110);
        assert!(matches!(events[0], FocusEvent::Logged(FocusLog { actual_seconds: 10, completed: false, .. })));
    }

    #[test]
    fn add_time_and_guards() {
        let mut engine = FocusEngine::default();
        assert!(engine.pause(0).is_err());
        assert!(engine.skip(0).is_err());
        engine.start(preset(), None, None, 0).unwrap();
        assert!(engine.start(preset(), None, None, 0).is_err());

        engine.add_time(10, 300).unwrap();
        let snapshot = engine.snapshot(10);
        assert_eq!(snapshot.remaining_seconds, 350);
        assert_eq!(snapshot.phase_total_seconds, 360);
        assert!(engine.add_time(10, -1_000).is_err());
    }

    #[test]
    fn validates_presets() {
        assert!(preset().validated().is_ok());

        let mut bad = preset();
        bad.name = "  ".into();
        assert!(bad.validated().is_err());

        let mut bad = preset();
        bad.focus_minutes = 0;
        assert!(bad.validated().is_err());

        let mut bad = preset();
        bad.sessions_until_long_break = 0;
        assert!(bad.validated().is_err());

        let mut bad = preset();
        bad.total_sessions = 51;
        assert!(bad.validated().is_err());
    }
}
