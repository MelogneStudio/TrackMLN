const pad = (value: number) => String(value).padStart(2, "0");

export const WEEKDAYS_SHORT = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;
export const WEEKDAYS_LONG = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"] as const;

/** Local YYYY-MM-DD for a Date. */
export function toIso(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function fromIso(iso: string): Date {
  return new Date(`${iso}T00:00:00`);
}

export function todayIso(): string {
  return toIso(new Date());
}

export function addDays(iso: string, days: number): string {
  const date = fromIso(iso);
  date.setDate(date.getDate() + days);
  return toIso(date);
}

/** 0 = Monday .. 6 = Sunday (matches the backend's weekday numbering). */
export function weekdayIndex(iso: string): number {
  return (fromIso(iso).getDay() + 6) % 7;
}

export function mondayOf(iso: string): string {
  return addDays(iso, -weekdayIndex(iso));
}

export function nowMinute(): number {
  const now = new Date();
  return now.getHours() * 60 + now.getMinutes();
}

export function isToday(iso: string): boolean {
  return iso === todayIso();
}

/** "Mon, 12 Oct" */
export function formatDateShort(iso: string): string {
  return fromIso(iso).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
}

/** "12 Oct" */
export function formatDayMonth(iso: string): string {
  return fromIso(iso).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** Friendly relative label for a due date. */
export function describeDueDate(iso: string | null): string {
  if (!iso) {
    return "No deadline";
  }
  const today = todayIso();
  if (iso === today) return "Today";
  if (iso === addDays(today, 1)) return "Tomorrow";
  if (iso === addDays(today, -1)) return "Yesterday";
  return formatDateShort(iso);
}
