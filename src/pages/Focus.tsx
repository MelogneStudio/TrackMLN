import { useState } from "react";
import { PresetModal } from "../features/PresetModal";
import { phaseLabel, useFocus, useFocusPresets, useFocusStats, usePoll, useTodos } from "../lib/hooks";
import type { FocusLog, FocusPreset } from "../types";
import { BarChart } from "../ui/BarChart";
import { Icon } from "../ui/Icon";
import { Ring } from "../ui/Ring";
import { Select } from "../ui/Select";
import { addDays, formatDateShort, todayIso } from "../utils/dates";
import { formatClock, formatDayLabel, formatDuration } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";

function describePreset(preset: FocusPreset): string {
  return `${preset.focusMinutes}m focus · ${preset.shortBreakMinutes}m break · ${preset.totalSessions} sessions`;
}

function formatLogTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  return `${date.toLocaleDateString(undefined, { day: "numeric", month: "short" })} · ${date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
}

export function Focus() {
  const { snapshot, remaining, progress, error: focusError, actions } = useFocus();
  const presets = useFocusPresets();
  const openTodos = useTodos("open");
  const end = todayIso();
  const start = addDays(end, -6);
  const stats = useFocusStats(start, end);
  const history = usePoll<FocusLog[]>(() => call<FocusLog[]>("get_focus_history", { limit: 8 }), 6000);

  const [presetId, setPresetId] = useState<number | null>(null);
  const [todoId, setTodoId] = useState<number>(0);
  const [editing, setEditing] = useState<FocusPreset | "new" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const list = presets.data ?? [];
  const chosen = presetId ?? list.find((preset) => preset.isDefault)?.id ?? list[0]?.id ?? null;
  const todos = openTodos.data ?? [];
  const linkedTodo = todos.find((todo) => todo.id === snapshot.todoId);

  const waiting = snapshot.status === "waiting";
  const paused = snapshot.status === "paused";
  const onBreak = snapshot.phase === "shortBreak" || snapshot.phase === "longBreak";

  const removePreset = async (preset: FocusPreset) => {
    try {
      await call("delete_focus_preset", { presetId: preset.id });
      setError(null);
      if (presetId === preset.id) setPresetId(null);
      await presets.refresh();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const dailyValues = (stats.data?.daily ?? []).map((day) => day.seconds);
  const dailyDates = (stats.data?.daily ?? []).map((day) => day.date);

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">Focus</h1>
          <p className="page-sub">Timed sessions with breaks, linked to your tasks.</p>
        </div>
        <div className="page__actions">
          <button className="btn" type="button" onClick={() => setEditing("new")}>
            <Icon name="plus" size={16} /> New preset
          </button>
        </div>
      </header>

      <div className="focus-layout">
        <section className="card focus-timer">
          <Ring
            progress={snapshot.active ? progress : 0}
            size={260}
            stroke={13}
            color={onBreak ? "var(--text-dim)" : "var(--accent)"}
          >
            {snapshot.active ? (
              <>
                <span className="chip chip--accent focus-timer__chip">{phaseLabel(snapshot.phase)}</span>
                <span className="display focus-timer__clock">{formatClock(remaining)}</span>
                <span className="faint">
                  {onBreak ? "Next: focus" : `Session ${snapshot.currentSession} of ${snapshot.totalSessions}`}
                </span>
              </>
            ) : (
              <>
                <Icon name="timer" size={34} strokeWidth={1.3} />
                <span className="display focus-timer__clock focus-timer__clock--idle">
                  {String(list.find((preset) => preset.id === chosen)?.focusMinutes ?? 25).padStart(2, "0")}:00
                </span>
                <span className="faint">Ready when you are</span>
              </>
            )}
          </Ring>

          {snapshot.active ? (
            <div className="stack" style={{ justifyItems: "center" }}>
              <div className="row row--wrap" style={{ justifyContent: "center" }}>
                {snapshot.preset ? <span className="chip">{snapshot.preset.name}</span> : null}
                {linkedTodo ? (
                  <span className="chip">
                    <Icon name="link" size={12} /> {linkedTodo.title}
                  </span>
                ) : null}
                {paused ? <span className="chip chip--warn">Paused</span> : null}
                {waiting ? <span className="chip chip--warn">Waiting for you to start</span> : null}
              </div>

              <div className="row">
                {waiting || paused ? (
                  <button className="btn btn--accent btn--lg" type="button" onClick={() => void actions.resume()}>
                    <Icon name="play" size={16} /> {waiting ? "Start" : "Resume"}
                  </button>
                ) : (
                  <button className="btn btn--lg" type="button" onClick={() => void actions.pause()}>
                    <Icon name="pause" size={16} /> Pause
                  </button>
                )}
                <button className="btn btn--lg" type="button" onClick={() => void actions.skip()}>
                  <Icon name="skip" size={16} /> Skip
                </button>
                <button className="btn btn--lg btn--danger" type="button" onClick={() => void actions.stop()}>
                  <Icon name="stop" size={16} /> Stop
                </button>
              </div>

              <button className="btn btn--ghost btn--sm" type="button" onClick={() => void actions.addTime(300)}>
                +5 minutes
              </button>
            </div>
          ) : (
            <div className="stack focus-timer__setup">
              <label className="field">
                <span>Focus on a task (optional)</span>
                <Select
                  options={[
                    { value: 0, label: "No task" },
                    ...todos.map((todo) => ({ value: todo.id, label: todo.title }))
                  ]}
                  value={todoId}
                  onChange={setTodoId}
                />
              </label>
              <button
                className="btn btn--accent btn--lg btn--block"
                type="button"
                disabled={chosen === null}
                onClick={() => void actions.start({ presetId: chosen, todoId: todoId || null })}
              >
                <Icon name="play" size={16} /> Start session
              </button>
            </div>
          )}

          {focusError ? <span className="error-text">{focusError}</span> : null}
        </section>

        <div className="focus-side">
          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Presets</h2>
            </div>
            {error ? <p className="error-text" style={{ marginBottom: 8 }}>{error}</p> : null}
            <div className="list">
              {list.length === 0 ? <div className="empty">No presets yet</div> : null}
              {list.map((preset) => (
                <article
                  key={preset.id ?? preset.name}
                  className={`preset${preset.id === chosen ? " is-selected" : ""}`}
                  onClick={() => !snapshot.active && setPresetId(preset.id ?? null)}
                >
                  <div className="item__body">
                    <div className="item__title">
                      {preset.name} {preset.isDefault ? <span className="chip chip--accent">Default</span> : null}
                    </div>
                    <div className="faint" style={{ fontSize: 12, marginTop: 2 }}>
                      {describePreset(preset)}
                    </div>
                  </div>
                  <div className="item__actions">
                    <button
                      className="icon-btn"
                      type="button"
                      title="Edit"
                      onClick={(event) => {
                        event.stopPropagation();
                        setEditing(preset);
                      }}
                    >
                      <Icon name="edit" size={16} />
                    </button>
                    <button
                      className="icon-btn icon-btn--danger"
                      type="button"
                      title="Delete"
                      onClick={(event) => {
                        event.stopPropagation();
                        void removePreset(preset);
                      }}
                    >
                      <Icon name="trash" size={16} />
                    </button>
                  </div>
                </article>
              ))}
            </div>
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Last 7 days</h2>
              <span className="faint" style={{ fontSize: 12 }}>
                {formatDuration(stats.data?.totalFocusSeconds ?? 0)} focused
              </span>
            </div>
            <BarChart
              values={dailyValues.length > 0 ? dailyValues : Array.from({ length: 7 }, () => 0)}
              labels={dailyDates.map((date) => formatDayLabel(date))}
              height={110}
              highlight={dailyValues.length - 1}
              formatTip={(value, index) => `${formatDateShort(dailyDates[index] ?? end)} · ${formatDuration(value)}`}
            />
            <div className="row row--wrap" style={{ marginTop: 14 }}>
              <span className="chip chip--accent">{stats.data?.completedSessions ?? 0} completed</span>
              <span className="chip">{stats.data?.abandonedSessions ?? 0} abandoned</span>
            </div>
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Recent sessions</h2>
            </div>
            <div className="stack stack--sm">
              {(history.data ?? []).length === 0 ? <div className="empty">Finished sessions will appear here</div> : null}
              {(history.data ?? []).map((log) => (
                <div className="log" key={`${log.startedAt}-${log.endedAt}`}>
                  <span className={`log__dot${log.completed ? " is-done" : ""}`} />
                  <div className="grow">
                    <div>{log.presetName || "Focus"}</div>
                    <div className="faint" style={{ fontSize: 11 }}>
                      {formatLogTime(log.startedAt)}
                    </div>
                  </div>
                  <span className="mono dim">{formatDuration(log.actualSeconds)}</span>
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>

      {editing ? (
        <PresetModal
          preset={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => void presets.refresh()}
        />
      ) : null}
    </div>
  );
}
