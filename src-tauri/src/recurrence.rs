use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

const MAX_EXPANSION_DAYS: i64 = 3660;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecurrenceKind {
    Daily,
    Weekly,
    Monthly,
}

/// A repeat rule shared by recurring todos and recurring timetable entries.
/// Dates are local `YYYY-MM-DD` strings; weekdays are 0 = Monday .. 6 = Sunday.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    pub kind: RecurrenceKind,
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(default)]
    pub weekdays: Vec<u8>,
    #[serde(default)]
    pub month_day: Option<u32>,
    pub start_date: String,
    #[serde(default)]
    pub end_date: Option<String>,
}

fn default_interval() -> u32 {
    1
}

pub fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| format!("Invalid date '{value}', expected YYYY-MM-DD"))
}

/// Local "today" and the current minute of the day (0..1440).
pub fn local_clock() -> (NaiveDate, i64) {
    let now = Local::now();
    (now.date_naive(), (now.hour() * 60 + now.minute()) as i64)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let first_of_next = NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("valid month");
    (first_of_next - Duration::days(1)).day()
}

fn monday_of(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

impl Recurrence {
    /// Checks the rule and returns a cleaned-up copy (sorted weekdays, trimmed dates).
    pub fn validated(mut self) -> Result<Recurrence, String> {
        if self.interval == 0 || self.interval > 365 {
            return Err("Repeat interval must be between 1 and 365".into());
        }

        let start = parse_date(&self.start_date)?;
        self.start_date = start.to_string();

        if let Some(end_raw) = self.end_date.clone() {
            if end_raw.trim().is_empty() {
                self.end_date = None;
            } else {
                let end = parse_date(&end_raw)?;
                if end < start {
                    return Err("Repeat end date is before the start date".into());
                }
                self.end_date = Some(end.to_string());
            }
        }

        if self.weekdays.iter().any(|day| *day > 6) {
            return Err("Weekdays must be between 0 (Monday) and 6 (Sunday)".into());
        }
        self.weekdays.sort_unstable();
        self.weekdays.dedup();

        match self.kind {
            RecurrenceKind::Weekly => {
                if self.weekdays.is_empty() {
                    self.weekdays = vec![start.weekday().num_days_from_monday() as u8];
                }
                self.month_day = None;
            }
            RecurrenceKind::Monthly => {
                let day = self.month_day.unwrap_or_else(|| start.day());
                if !(1..=31).contains(&day) {
                    return Err("Day of month must be between 1 and 31".into());
                }
                self.month_day = Some(day);
                self.weekdays.clear();
            }
            RecurrenceKind::Daily => {
                self.weekdays.clear();
                self.month_day = None;
            }
        }

        Ok(self)
    }

    pub fn matches(&self, date: NaiveDate) -> bool {
        let Ok(start) = parse_date(&self.start_date) else {
            return false;
        };
        if date < start {
            return false;
        }
        if let Some(end) = self.end_date.as_deref().and_then(|raw| parse_date(raw).ok()) {
            if date > end {
                return false;
            }
        }

        let interval = self.interval.max(1) as i64;
        match self.kind {
            RecurrenceKind::Daily => (date - start).num_days() % interval == 0,
            RecurrenceKind::Weekly => {
                let weeks = (monday_of(date) - monday_of(start)).num_days() / 7;
                let weekday = date.weekday().num_days_from_monday() as u8;
                let allowed = if self.weekdays.is_empty() {
                    start.weekday().num_days_from_monday() as u8 == weekday
                } else {
                    self.weekdays.contains(&weekday)
                };
                allowed && weeks % interval == 0
            }
            RecurrenceKind::Monthly => {
                let months = (date.year() - start.year()) as i64 * 12
                    + date.month() as i64
                    - start.month() as i64;
                if months < 0 || months % interval != 0 {
                    return false;
                }
                let wanted = self.month_day.unwrap_or_else(|| start.day());
                let target = wanted.min(days_in_month(date.year(), date.month()));
                date.day() == target
            }
        }
    }

    /// Every date in `[from, to]` (inclusive) the rule lands on.
    pub fn occurrences_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
        let mut dates = Vec::new();
        if to < from {
            return dates;
        }

        let to = to.min(from + Duration::days(MAX_EXPANSION_DAYS));
        let mut day = from;
        while day <= to {
            if self.matches(day) {
                dates.push(day);
            }
            day += Duration::days(1);
        }
        dates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    fn rule(kind: RecurrenceKind, start: &str) -> Recurrence {
        Recurrence {
            kind,
            interval: 1,
            weekdays: vec![],
            month_day: None,
            start_date: start.into(),
            end_date: None,
        }
    }

    #[test]
    fn daily_every_other_day() {
        let mut daily = rule(RecurrenceKind::Daily, "2026-01-01");
        daily.interval = 2;
        let dates = daily.occurrences_between(date("2026-01-01"), date("2026-01-07"));
        assert_eq!(
            dates,
            vec![date("2026-01-01"), date("2026-01-03"), date("2026-01-05"), date("2026-01-07")]
        );
    }

    #[test]
    fn weekly_on_monday_and_wednesday() {
        let mut weekly = rule(RecurrenceKind::Weekly, "2026-01-05");
        weekly.weekdays = vec![0, 2];
        let dates = weekly.occurrences_between(date("2026-01-05"), date("2026-01-18"));
        assert_eq!(
            dates,
            vec![date("2026-01-05"), date("2026-01-07"), date("2026-01-12"), date("2026-01-14")]
        );
    }

    #[test]
    fn weekly_every_two_weeks() {
        let mut weekly = rule(RecurrenceKind::Weekly, "2026-01-05");
        weekly.interval = 2;
        weekly.weekdays = vec![0];
        let dates = weekly.occurrences_between(date("2026-01-05"), date("2026-02-02"));
        assert_eq!(dates, vec![date("2026-01-05"), date("2026-01-19"), date("2026-02-02")]);
    }

    #[test]
    fn monthly_clamps_to_short_months() {
        let mut monthly = rule(RecurrenceKind::Monthly, "2026-01-31");
        monthly.month_day = Some(31);
        let dates = monthly.occurrences_between(date("2026-01-01"), date("2026-04-30"));
        assert_eq!(
            dates,
            vec![date("2026-01-31"), date("2026-02-28"), date("2026-03-31"), date("2026-04-30")]
        );
    }

    #[test]
    fn respects_end_date_and_start_date() {
        let mut daily = rule(RecurrenceKind::Daily, "2026-01-03");
        daily.end_date = Some("2026-01-05".into());
        let dates = daily.occurrences_between(date("2026-01-01"), date("2026-01-10"));
        assert_eq!(dates, vec![date("2026-01-03"), date("2026-01-04"), date("2026-01-05")]);
    }

    #[test]
    fn validation_fills_defaults_and_rejects_bad_input() {
        let weekly = rule(RecurrenceKind::Weekly, "2026-01-07").validated().unwrap();
        assert_eq!(weekly.weekdays, vec![2]);

        let mut bad = rule(RecurrenceKind::Daily, "2026-01-07");
        bad.interval = 0;
        assert!(bad.validated().is_err());

        let mut backwards = rule(RecurrenceKind::Daily, "2026-01-07");
        backwards.end_date = Some("2026-01-01".into());
        assert!(backwards.validated().is_err());
    }
}
