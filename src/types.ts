export type AppTotal = {
  app_identity: string;
  app_name: string;
  total: number;
};

export type AppSettings = {
  hotkey: string;
  blurPercent: number;
  material: "mica" | "liquid";
  exeLabels: Record<string, string>;
  idleTimeoutSeconds: number;
  uiPrefs: string;
};

export type HourlyData = {
  hour: number;
  total: number;
};

export type WeekDay = {
  date: string;
  total: number;
  apps: AppTotal[];
};

export type WeekData = {
  days: WeekDay[];
  apps: AppTotal[];
  week_total: number;
  current_week_average: number;
  previous_week_average: number;
  top_app: AppTotal | null;
};

export type DayTotal = {
  date: string;
  total: number;
};

export type UsageRange = {
  startDate: string;
  endDate: string;
  totalSeconds: number;
  averagePerDay: number;
  daily: DayTotal[];
  apps: AppTotal[];
  busiestDay: DayTotal | null;
  longestSessionSeconds: number;
  sessionCount: number;
};

export type Goal = {
  id: number;
  targetKind: "app" | "total";
  targetValue: string;
  label: string;
  warnSeconds: number | null;
  annoySeconds: number | null;
};

export type GoalDraft = {
  id?: number | null;
  targetKind: "app" | "total";
  targetValue: string;
  warnSeconds: number | null;
  annoySeconds: number | null;
};

export type GoalCandidate = {
  appIdentity: string;
  appName: string;
  totalSeconds: number;
};

export type GoalAlertPayload = {
  goalId: number;
  targetKind: "app" | "total";
  targetValue: string;
  label: string;
  threshold: "warn" | "annoy";
  totalSeconds: number;
  thresholdSeconds: number;
  repeatMinutes: number;
  showOverlay: boolean;
  snoozeCount: number;
};

/* ── Recurrence ─────────────────────────────────────────────────────────── */

export type RecurrenceKind = "daily" | "weekly" | "monthly";

/** Weekdays are 0 = Monday .. 6 = Sunday; dates are local YYYY-MM-DD. */
export type Recurrence = {
  kind: RecurrenceKind;
  interval: number;
  weekdays: number[];
  monthDay: number | null;
  startDate: string;
  endDate: string | null;
};

/* ── Todos ──────────────────────────────────────────────────────────────── */

/** 0 = none, 1 = low, 2 = medium, 3 = high */
export type Priority = 0 | 1 | 2 | 3;

export type Todo = {
  id: number;
  title: string;
  notes: string;
  priority: number;
  dueDate: string | null;
  dueMinute: number | null;
  estimateMinutes: number | null;
  createdAt: number;
  completedAt: number | null;
  completed: boolean;
  overdue: boolean;
  templateId: number | null;
  occurrenceDate: string | null;
};

export type TodoDraft = {
  id?: number | null;
  title: string;
  notes: string;
  priority: number;
  dueDate: string | null;
  dueMinute: number | null;
  estimateMinutes: number | null;
};

export type TodoStatus = "open" | "completed" | "overdue" | "today" | "all";

export type TodoSummary = {
  open: number;
  overdue: number;
  dueToday: number;
  completedToday: number;
};

export type TodoTemplate = {
  id: number;
  title: string;
  notes: string;
  priority: number;
  dueMinute: number | null;
  estimateMinutes: number | null;
  recurrence: Recurrence;
  active: boolean;
  createdAt: number;
};

export type TodoTemplateDraft = {
  id?: number | null;
  title: string;
  notes: string;
  priority: number;
  dueMinute: number | null;
  estimateMinutes: number | null;
  recurrence: Recurrence;
  active: boolean;
};

/* ── Timetable ──────────────────────────────────────────────────────────── */

export type TimetableEntry = {
  id: number;
  title: string;
  notes: string;
  color: string;
  date: string | null;
  startMinute: number;
  durationMinutes: number;
  recurrence: Recurrence | null;
  todoId: number | null;
  presetId: number | null;
  autoStart: boolean;
};

export type TimetableEntryDraft = {
  id?: number | null;
  title: string;
  notes: string;
  color: string;
  date: string | null;
  startMinute: number;
  durationMinutes: number;
  recurrence: Recurrence | null;
  todoId: number | null;
  presetId: number | null;
  autoStart: boolean;
};

export type TimetableOccurrence = {
  entryId: number;
  date: string;
  title: string;
  notes: string;
  color: string;
  startMinute: number;
  endMinute: number;
  durationMinutes: number;
  todoId: number | null;
  todoCompleted: boolean | null;
  presetId: number | null;
  autoStart: boolean;
  recurring: boolean;
  modified: boolean;
  overlaps: boolean;
};

export type OccurrencePatch = {
  title?: string | null;
  startMinute?: number | null;
  durationMinutes?: number | null;
};

export type FreeSlot = {
  startMinute: number;
  endMinute: number;
};

/* ── Focus ──────────────────────────────────────────────────────────────── */

export type FocusPhase = "focus" | "shortBreak" | "longBreak";
export type FocusStatus = "running" | "paused" | "waiting";

export type FocusPreset = {
  id?: number | null;
  name: string;
  focusMinutes: number;
  shortBreakMinutes: number;
  longBreakMinutes: number;
  sessionsUntilLongBreak: number;
  totalSessions: number;
  autoStartBreaks: boolean;
  autoStartFocus: boolean;
  isDefault: boolean;
};

export type FocusSnapshot = {
  active: boolean;
  phase: FocusPhase | null;
  status: FocusStatus | null;
  remainingSeconds: number;
  phaseTotalSeconds: number;
  phaseEndsAt: number | null;
  focusCompleted: number;
  totalSessions: number;
  currentSession: number;
  nextPhase: FocusPhase | null;
  preset: FocusPreset | null;
  todoId: number | null;
  entryId: number | null;
};

export type FocusLog = {
  startedAt: number;
  endedAt: number;
  plannedSeconds: number;
  actualSeconds: number;
  completed: boolean;
  presetId: number | null;
  presetName: string;
  todoId: number | null;
  entryId: number | null;
};

export type DayFocus = {
  date: string;
  seconds: number;
  completedSessions: number;
};

export type TodoFocus = {
  todoId: number;
  seconds: number;
};

export type FocusStats = {
  startDate: string;
  endDate: string;
  totalFocusSeconds: number;
  completedSessions: number;
  abandonedSessions: number;
  daily: DayFocus[];
  byTodo: TodoFocus[];
};
