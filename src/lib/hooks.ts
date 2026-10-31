import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  AppTotal,
  FocusPreset,
  FocusSnapshot,
  FocusStats,
  HourlyData,
  TimetableOccurrence,
  Todo,
  TodoStatus,
  TodoSummary,
  UsageRange
} from "../types";
import { addDays, todayIso } from "../utils/dates";
import { call, errorMessage } from "../utils/ipc";

export type Polled<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
  refresh: () => Promise<void>;
};

/**
 * Runs `fetcher` immediately and then every `intervalMs`. State only changes
 * when the JSON of the result changes, so idle polling never re-renders.
 */
export function usePoll<T>(
  fetcher: () => Promise<T>,
  intervalMs: number,
  deps: ReadonlyArray<unknown> = [],
  enabled = true
): Polled<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;
  const lastJson = useRef<string>("");

  const refresh = useCallback(async () => {
    try {
      const next = await fetcherRef.current();
      const json = JSON.stringify(next);
      if (json !== lastJson.current) {
        lastJson.current = json;
        setData(next);
      }
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      return;
    }
    lastJson.current = "";
    setLoading(true);
    void refresh();
    if (intervalMs <= 0) {
      return;
    }
    const id = window.setInterval(() => void refresh(), intervalMs);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, intervalMs, refresh, ...deps]);

  return { data, error, loading, refresh };
}

/** A Date that re-renders every `intervalMs`. */
export function useClock(intervalMs = 1000): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

/* ── Screen time ────────────────────────────────────────────────────────── */

export type TodayUsage = {
  apps: AppTotal[];
  hourly: { hour: number; total: number }[];
  total: number;
};

export function useTodayUsage(enabled = true, intervalMs = 2000): Polled<TodayUsage> {
  return usePoll<TodayUsage>(
    async () => {
      const [apps, hourly] = await Promise.all([
        call<AppTotal[]>("get_today_totals"),
        call<HourlyData[]>("get_hourly_today")
      ]);
      const filled = Array.from({ length: 24 }, (_, hour) => ({
        hour,
        total: hourly.find((entry) => entry.hour === hour)?.total ?? 0
      }));
      return { apps, hourly: filled, total: apps.reduce((sum, app) => sum + app.total, 0) };
    },
    intervalMs,
    [],
    enabled
  );
}

export function useUsageRange(startDate: string, endDate: string, enabled = true): Polled<UsageRange> {
  return usePoll<UsageRange>(
    () => call<UsageRange>("get_usage_range", { startDate, endDate }),
    5000,
    [startDate, endDate],
    enabled
  );
}

/* ── Todos ──────────────────────────────────────────────────────────────── */

export function useTodos(status: TodoStatus, enabled = true, intervalMs = 3000): Polled<Todo[]> {
  return usePoll<Todo[]>(() => call<Todo[]>("get_todos", { status }), intervalMs, [status], enabled);
}

export function useTodoSummary(enabled = true): Polled<TodoSummary> {
  return usePoll<TodoSummary>(() => call<TodoSummary>("get_todo_summary"), 3000, [], enabled);
}

/* ── Timetable ──────────────────────────────────────────────────────────── */

export function useTimetable(startDate: string, endDate: string, enabled = true): Polled<TimetableOccurrence[]> {
  return usePoll<TimetableOccurrence[]>(
    () => call<TimetableOccurrence[]>("get_timetable", { startDate, endDate }),
    4000,
    [startDate, endDate],
    enabled
  );
}

export type TimetableToday = {
  all: TimetableOccurrence[];
  current: TimetableOccurrence | null;
  next: TimetableOccurrence | null;
};

/** Today's blocks plus which one is happening now and which is next. */
export function useTimetableToday(enabled = true): { today: TimetableToday; polled: Polled<TimetableOccurrence[]> } {
  const date = todayIso();
  const polled = useTimetable(date, date, enabled);
  const now = useClock(30_000);
  const minute = now.getHours() * 60 + now.getMinutes();

  const today = useMemo<TimetableToday>(() => {
    const all = [...(polled.data ?? [])].sort((a, b) => a.startMinute - b.startMinute);
    const current = all.find((block) => block.startMinute <= minute && minute < block.endMinute) ?? null;
    const next = all.find((block) => block.startMinute > minute) ?? null;
    return { all, current, next };
  }, [polled.data, minute]);

  return { today, polled };
}

export function useUpcomingWeek(enabled = true): Polled<TimetableOccurrence[]> {
  const start = todayIso();
  return useTimetable(start, addDays(start, 6), enabled);
}

/* ── Focus ──────────────────────────────────────────────────────────────── */

const IDLE_SNAPSHOT: FocusSnapshot = {
  active: false,
  phase: null,
  status: null,
  remainingSeconds: 0,
  phaseTotalSeconds: 0,
  phaseEndsAt: null,
  focusCompleted: 0,
  totalSessions: 0,
  currentSession: 0,
  nextPhase: null,
  preset: null,
  todoId: null,
  entryId: null
};

export type StartFocusOptions = {
  presetId?: number | null;
  preset?: FocusPreset | null;
  todoId?: number | null;
  entryId?: number | null;
};

export function useFocus(enabled = true) {
  const [snapshot, setSnapshot] = useState<FocusSnapshot>(IDLE_SNAPSHOT);
  const [error, setError] = useState<string | null>(null);
  const [nowSec, setNowSec] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    if (!enabled) {
      return;
    }
    let cancelled = false;
    let dispose: (() => void) | undefined;

    call<FocusSnapshot>("get_focus_state")
      .then((value) => {
        if (!cancelled) setSnapshot(value);
      })
      .catch((err) => !cancelled && setError(errorMessage(err)));

    void listen<FocusSnapshot>("focus-update", (event) => setSnapshot(event.payload)).then((unlisten) => {
      if (cancelled) unlisten();
      else dispose = unlisten;
    });

    return () => {
      cancelled = true;
      dispose?.();
    };
  }, [enabled]);

  const running = snapshot.active && snapshot.status === "running";

  useEffect(() => {
    if (!enabled || !running) {
      return;
    }
    setNowSec(Math.floor(Date.now() / 1000));
    const id = window.setInterval(() => setNowSec(Math.floor(Date.now() / 1000)), 500);
    return () => window.clearInterval(id);
  }, [enabled, running]);

  const remaining =
    running && snapshot.phaseEndsAt !== null
      ? Math.max(0, snapshot.phaseEndsAt - nowSec)
      : snapshot.remainingSeconds;

  const run = useCallback(async (command: string, args?: Record<string, unknown>) => {
    try {
      const next = await call<FocusSnapshot>(command, args);
      setSnapshot(next);
      setError(null);
      return next;
    } catch (err) {
      setError(errorMessage(err));
      return null;
    }
  }, []);

  const actions = useMemo(
    () => ({
      start: (options: StartFocusOptions = {}) =>
        run("start_focus", {
          presetId: options.presetId ?? null,
          preset: options.preset ?? null,
          todoId: options.todoId ?? null,
          entryId: options.entryId ?? null
        }),
      pause: () => run("pause_focus"),
      resume: () => run("resume_focus"),
      skip: () => run("skip_focus_phase"),
      stop: () => run("stop_focus"),
      addTime: (seconds: number) => run("add_focus_time", { seconds })
    }),
    [run]
  );

  const progress =
    snapshot.phaseTotalSeconds > 0 ? Math.min(1, Math.max(0, 1 - remaining / snapshot.phaseTotalSeconds)) : 0;

  return { snapshot, remaining, progress, error, actions };
}

export function useFocusPresets(enabled = true): Polled<FocusPreset[]> {
  return usePoll<FocusPreset[]>(() => call<FocusPreset[]>("get_focus_presets"), 6000, [], enabled);
}

/** Focus time logged for a date range (defaults to today). */
export function useFocusStats(startDate?: string, endDate?: string, enabled = true): Polled<FocusStats> {
  const start = startDate ?? todayIso();
  const end = endDate ?? start;
  return usePoll<FocusStats>(
    () => call<FocusStats>("get_focus_stats", { startDate: start, endDate: end }),
    5000,
    [start, end],
    enabled
  );
}

export function phaseLabel(phase: FocusSnapshot["phase"]): string {
  switch (phase) {
    case "focus":
      return "Focus";
    case "shortBreak":
      return "Short break";
    case "longBreak":
      return "Long break";
    default:
      return "Idle";
  }
}
