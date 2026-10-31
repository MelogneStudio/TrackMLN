import { useState } from "react";
import { AppsList } from "../features/AppsList";
import { useTodayUsage, usePoll, useUsageRange } from "../lib/hooks";
import type { AppTotal, DayTotal } from "../types";
import { BarChart } from "../ui/BarChart";
import { Segmented } from "../ui/Controls";
import { addDays, formatDateShort, fromIso, todayIso } from "../utils/dates";
import {
  formatDayLabel,
  formatDuration,
  formatHeroDuration,
  formatHour,
  HOUR_LABELS,
  percentChange
} from "../utils/format";
import { call } from "../utils/ipc";

type Range = "today" | "week" | "month";

function Tile({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="tile">
      <div className="faint tile__label">{label}</div>
      <div className="tile__value display">{value}</div>
      {hint ? <div className="faint tile__hint">{hint}</div> : null}
    </div>
  );
}

export function ScreenTime() {
  const [range, setRange] = useState<Range>("today");
  const [focusApp, setFocusApp] = useState<AppTotal | null>(null);

  const days = range === "month" ? 30 : 7;
  const end = todayIso();
  const start = addDays(end, -(days - 1));
  const previousStart = addDays(start, -days);
  const previousEnd = addDays(start, -1);
  const lastWeekDay = addDays(end, -7);

  const today = useTodayUsage(range === "today");
  const ranged = useUsageRange(start, end, range !== "today");
  const previous = useUsageRange(previousStart, previousEnd, range !== "today");
  const lastWeek = useUsageRange(lastWeekDay, lastWeekDay, range === "today");

  const history = usePoll<DayTotal[]>(
    () => call<DayTotal[]>("get_app_history", { appIdentity: focusApp?.app_identity ?? "", startDate: start, endDate: end }),
    10_000,
    [focusApp?.app_identity, start, end],
    range !== "today" && focusApp !== null
  );

  const changeRange = (next: Range) => {
    setRange(next);
    setFocusApp(null);
  };

  const total = range === "today" ? (today.data?.total ?? 0) : (ranged.data?.totalSeconds ?? 0);
  const compare = range === "today" ? (lastWeek.data?.totalSeconds ?? 0) : (previous.data?.totalSeconds ?? 0);
  const change = percentChange(total, compare);
  const apps = (range === "today" ? today.data?.apps : ranged.data?.apps) ?? [];
  const hero = formatHeroDuration(total);

  const dailyRaw = range === "today" ? [] : (ranged.data?.daily ?? []);
  const dailyValues = focusApp && history.data ? history.data.map((entry) => entry.total) : dailyRaw.map((entry) => entry.total);
  const dailyDates = dailyRaw.map((entry) => entry.date);
  const dailyLabels = dailyDates.map((date) => (range === "week" ? formatDayLabel(date) : String(fromIso(date).getDate())));

  const hourly = today.data?.hourly ?? Array.from({ length: 24 }, (_, hour) => ({ hour, total: 0 }));
  const compareLabel =
    range === "today"
      ? `last ${new Date().toLocaleDateString(undefined, { weekday: "long" })}`
      : `the previous ${days} days`;

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">Screen time</h1>
          <p className="page-sub">
            {range === "today" ? formatDateShort(end) : `${formatDateShort(start)} – ${formatDateShort(end)}`}
          </p>
        </div>
        <div className="page__actions">
          <Segmented
            value={range}
            onChange={changeRange}
            options={[
              { value: "today", label: "Today" },
              { value: "week", label: "7 days" },
              { value: "month", label: "30 days" }
            ]}
          />
        </div>
      </header>

      <div className="stack">
        <section className="card">
          <div className="row row--wrap" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
            <div>
              <h2 className="card__title">{range === "today" ? "Today" : `Last ${days} days`}</h2>
              <div className="stat-big" style={{ marginTop: 6 }}>
                {hero.hours > 0 ? `${hero.hours}hr ${hero.minutes}m` : `${hero.minutes}m`}
                {change !== null ? <sup>{change > 0 ? `+${change}` : change}%</sup> : null}
              </div>
              <p className="faint" style={{ marginTop: 6, fontSize: 12 }}>
                {change === null ? `Nothing recorded for ${compareLabel} yet` : `${Math.abs(change)}% ${change <= 0 ? "less" : "more"} than ${compareLabel}`}
              </p>
            </div>
            {focusApp ? (
              <button className="chip chip--accent" type="button" onClick={() => setFocusApp(null)}>
                {focusApp.app_name} · clear ✕
              </button>
            ) : null}
          </div>

          <div style={{ marginTop: 20 }}>
            {range === "today" ? (
              <BarChart
                values={hourly.map((entry) => entry.total)}
                labels={HOUR_LABELS}
                labelEvery={2}
                height={170}
                highlight={new Date().getHours()}
                formatTip={(value, index) => `${formatHour(index)} · ${formatDuration(value)}`}
              />
            ) : (
              <BarChart
                values={dailyValues}
                labels={dailyLabels}
                labelEvery={range === "month" ? 3 : 1}
                height={170}
                highlight={dailyValues.length - 1}
                formatTip={(value, index) => `${formatDateShort(dailyDates[index] ?? end)} · ${formatDuration(value)}`}
              />
            )}
          </div>
        </section>

        {range !== "today" && ranged.data ? (
          <div className="tiles">
            <Tile label="Daily average" value={formatDuration(ranged.data.averagePerDay)} />
            <Tile
              label="Busiest day"
              value={ranged.data.busiestDay ? formatDuration(ranged.data.busiestDay.total) : "–"}
              hint={ranged.data.busiestDay ? formatDateShort(ranged.data.busiestDay.date) : undefined}
            />
            <Tile label="Longest session" value={formatDuration(ranged.data.longestSessionSeconds)} />
            <Tile label="Sessions" value={String(ranged.data.sessionCount)} />
          </div>
        ) : null}

        <section className="card">
          <div className="card__head">
            <h2 className="card__title">Apps</h2>
            {range !== "today" ? <span className="faint" style={{ fontSize: 12 }}>Click an app to see its daily history</span> : null}
          </div>
          <AppsList
            apps={apps}
            limit={15}
            onSelect={range === "today" ? undefined : (app) => setFocusApp(focusApp?.app_identity === app.app_identity ? null : app)}
            selectedId={focusApp?.app_identity ?? null}
          />
        </section>
      </div>
    </div>
  );
}
