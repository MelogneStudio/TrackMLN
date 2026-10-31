import { useState } from "react";
import { useFocusPresets, useTodos } from "../lib/hooks";
import type { Recurrence, TimetableEntry, TimetableEntryDraft, TimetableOccurrence } from "../types";
import { Modal } from "../ui/Modal";
import { Select } from "../ui/Select";
import { Toggle } from "../ui/Controls";
import { Icon } from "../ui/Icon";
import { formatDateShort } from "../utils/dates";
import { formatMinuteOfDay, formatMinutes, minuteToTimeInput, timeInputToMinute } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";
import { newRecurrence } from "../utils/recurrence";
import { RecurrenceEditor } from "./RecurrenceEditor";

export const BLOCK_COLORS = ["", "#a7dc7b", "#74d6b0", "#7ab6ff", "#b9a2ff", "#ff9fcd", "#ffb272", "#f1d46a"];

/* ── Entry editor (create / edit the whole block or series) ─────────────── */

type EntryModalProps = {
  entry?: TimetableEntry | null;
  defaultDate: string;
  defaultStart: number;
  onClose: () => void;
  onSaved: () => void;
};

export function EntryModal({ entry, defaultDate, defaultStart, onClose, onSaved }: EntryModalProps) {
  const todos = useTodos("open");
  const presets = useFocusPresets();

  const [title, setTitle] = useState(entry?.title ?? "");
  const [notes, setNotes] = useState(entry?.notes ?? "");
  const [color, setColor] = useState(entry?.color ?? "");
  const [repeat, setRepeat] = useState(Boolean(entry?.recurrence));
  const [date, setDate] = useState(entry?.date ?? defaultDate);
  const [recurrence, setRecurrence] = useState<Recurrence>(entry?.recurrence ?? newRecurrence(entry?.date ?? defaultDate));
  const [start, setStart] = useState(minuteToTimeInput(entry?.startMinute ?? defaultStart));
  const [duration, setDuration] = useState(String(entry?.durationMinutes ?? 60));
  const [todoId, setTodoId] = useState(entry?.todoId ?? 0);
  const [presetId, setPresetId] = useState(entry?.presetId ?? 0);
  const [autoStart, setAutoStart] = useState(entry?.autoStart ?? false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const todoOptions = [
    { value: 0, label: "No linked task" },
    ...(todos.data ?? []).map((todo) => ({ value: todo.id, label: todo.title })),
    ...(entry?.todoId && !(todos.data ?? []).some((todo) => todo.id === entry.todoId)
      ? [{ value: entry.todoId, label: "Linked task (completed)" }]
      : [])
  ];
  const presetOptions = [
    { value: 0, label: "No focus preset" },
    ...(presets.data ?? []).map((preset) => ({ value: preset.id ?? 0, label: preset.name, sublabel: `${preset.focusMinutes}m` }))
  ];

  const save = async () => {
    const startMinute = timeInputToMinute(start);
    const durationMinutes = Math.round(Number(duration));

    if (startMinute === null) return setError("Pick a start time.");
    if (!Number.isFinite(durationMinutes) || durationMinutes < 1) return setError("Duration must be at least 1 minute.");
    if (!title.trim() && todoId === 0) return setError("Give the block a title or link a task.");
    if (repeat && recurrence.kind === "weekly" && recurrence.weekdays.length === 0) return setError("Pick at least one weekday.");

    const draft: TimetableEntryDraft = {
      id: entry?.id ?? null,
      title: title.trim(),
      notes,
      color,
      date: repeat ? null : date,
      startMinute,
      durationMinutes,
      recurrence: repeat ? recurrence : null,
      todoId: todoId || null,
      presetId: presetId || null,
      autoStart: autoStart && presetId !== 0
    };

    setBusy(true);
    setError(null);
    try {
      await call("save_timetable_entry", { draft });
      onSaved();
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={entry ? (entry.recurrence ? "Edit series" : "Edit block") : "New block"}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn" type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--accent" type="button" disabled={busy} onClick={() => void save()}>
            {busy ? "Saving…" : "Save"}
          </button>
        </>
      }
    >
      <div className="stack">
        <label className="field">
          <span>Title {todoId !== 0 ? "(optional, follows the task)" : ""}</span>
          <input
            className="input"
            value={title}
            maxLength={200}
            autoFocus
            placeholder="Deep work, Maths class…"
            onChange={(event) => setTitle(event.target.value)}
          />
        </label>

        <div className="grid-3">
          <label className="field">
            <span>{repeat ? "Series starts" : "Date"}</span>
            <input
              className="input"
              type="date"
              value={repeat ? recurrence.startDate : date}
              onChange={(event) => {
                if (!event.target.value) return;
                if (repeat) setRecurrence({ ...recurrence, startDate: event.target.value });
                else setDate(event.target.value);
              }}
            />
          </label>
          <label className="field">
            <span>Start</span>
            <input className="input" type="time" value={start} onChange={(event) => setStart(event.target.value)} />
          </label>
          <label className="field">
            <span>Length (min)</span>
            <input className="input" type="number" min={1} step={5} value={duration} onChange={(event) => setDuration(event.target.value)} />
          </label>
        </div>

        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div>Repeat</div>
            <div className="faint" style={{ fontSize: 12 }}>
              Make this a recurring block, like “Mon/Wed/Fri 09:00”.
            </div>
          </div>
          <Toggle on={repeat} onChange={setRepeat} label="Repeat" />
        </div>
        {repeat ? <RecurrenceEditor value={recurrence} onChange={setRecurrence} /> : null}

        <div className="field">
          <span>Colour</span>
          <div className="row row--wrap">
            {BLOCK_COLORS.map((swatch) => (
              <button
                key={swatch || "none"}
                type="button"
                aria-label={swatch || "Default colour"}
                className={`dot-swatch${color === swatch ? " is-active" : ""}`}
                style={{ background: swatch || "var(--accent)" }}
                onClick={() => setColor(swatch)}
              />
            ))}
          </div>
        </div>

        <div className="grid-2">
          <div className="field">
            <span>Linked task</span>
            <Select options={todoOptions} value={todoId} onChange={setTodoId} />
          </div>
          <div className="field">
            <span>Focus preset</span>
            <Select
              options={presetOptions}
              value={presetId}
              onChange={(value) => {
                setPresetId(value);
                if (value === 0) setAutoStart(false);
              }}
            />
          </div>
        </div>

        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div>Auto-start focus</div>
            <div className="faint" style={{ fontSize: 12 }}>
              Begin the preset by itself when this block starts.
            </div>
          </div>
          <Toggle on={autoStart && presetId !== 0} onChange={setAutoStart} label="Auto-start focus" disabled={presetId === 0} />
        </div>

        <label className="field">
          <span>Notes</span>
          <textarea className="input" value={notes} onChange={(event) => setNotes(event.target.value)} placeholder="Optional" />
        </label>

        {error ? <span className="error-text">{error}</span> : null}
      </div>
    </Modal>
  );
}

/* ── Occurrence sheet (one block on one day) ────────────────────────────── */

type OccurrenceModalProps = {
  occurrence: TimetableOccurrence;
  onClose: () => void;
  onChanged: () => void;
  onEditEntry: (entryId: number) => void;
  onStartFocus: () => void;
};

export function OccurrenceModal({ occurrence, onClose, onChanged, onEditEntry, onStartFocus }: OccurrenceModalProps) {
  const [mode, setMode] = useState<"view" | "day">("view");
  const [title, setTitle] = useState(occurrence.title);
  const [start, setStart] = useState(minuteToTimeInput(occurrence.startMinute));
  const [duration, setDuration] = useState(String(occurrence.durationMinutes));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (action: () => Promise<unknown>, closeAfter = true) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      onChanged();
      if (closeAfter) onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const startFocus = () =>
    run(async () => {
      await call("start_focus", { presetId: null, preset: null, todoId: occurrence.todoId, entryId: occurrence.entryId });
      onStartFocus();
    });

  const saveDay = () => {
    const startMinute = timeInputToMinute(start);
    const durationMinutes = Math.round(Number(duration));
    if (startMinute === null) return setError("Pick a start time.");
    if (!Number.isFinite(durationMinutes) || durationMinutes < 1) return setError("Duration must be at least 1 minute.");

    void run(() =>
      call("edit_timetable_occurrence", {
        entryId: occurrence.entryId,
        date: occurrence.date,
        patch: {
          title: title.trim() && title.trim() !== occurrence.title ? title.trim() : null,
          startMinute,
          durationMinutes
        }
      })
    );
  };

  if (mode === "day") {
    return (
      <Modal
        title="Edit this day only"
        onClose={() => setMode("view")}
        footer={
          <>
            <button className="btn" type="button" onClick={() => setMode("view")}>
              Back
            </button>
            <button className="btn btn--accent" type="button" disabled={busy} onClick={saveDay}>
              Save
            </button>
          </>
        }
      >
        <div className="stack">
          <p className="faint">Changes only {formatDateShort(occurrence.date)}; the rest of the series stays the same.</p>
          <label className="field">
            <span>Title</span>
            <input className="input" value={title} onChange={(event) => setTitle(event.target.value)} />
          </label>
          <div className="grid-2">
            <label className="field">
              <span>Start</span>
              <input className="input" type="time" value={start} onChange={(event) => setStart(event.target.value)} />
            </label>
            <label className="field">
              <span>Length (min)</span>
              <input className="input" type="number" min={1} step={5} value={duration} onChange={(event) => setDuration(event.target.value)} />
            </label>
          </div>
          {error ? <span className="error-text">{error}</span> : null}
        </div>
      </Modal>
    );
  }

  return (
    <Modal title={occurrence.title || "Block"} onClose={onClose}>
      <div className="stack">
        <div className="row row--wrap">
          <span className="chip">
            <Icon name="calendar" size={12} /> {formatDateShort(occurrence.date)}
          </span>
          <span className="chip">
            <Icon name="clock" size={12} /> {formatMinuteOfDay(occurrence.startMinute)} – {formatMinuteOfDay(occurrence.endMinute)} ·{" "}
            {formatMinutes(occurrence.durationMinutes)}
          </span>
          {occurrence.recurring ? (
            <span className="chip chip--accent">
              <Icon name="repeat" size={12} /> Repeats
            </span>
          ) : null}
          {occurrence.modified ? <span className="chip">Edited</span> : null}
          {occurrence.autoStart ? (
            <span className="chip chip--accent">
              <Icon name="timer" size={12} /> Auto-focus
            </span>
          ) : null}
          {occurrence.overlaps ? <span className="chip chip--warn">Overlaps another block</span> : null}
        </div>

        {occurrence.notes ? <p className="dim" style={{ whiteSpace: "pre-wrap" }}>{occurrence.notes}</p> : null}

        <div className="sheet">
          <button className="sheet__item" type="button" disabled={busy} onClick={() => void startFocus()}>
            <Icon name="play" size={17} /> Start focus now
          </button>

          {occurrence.todoId !== null ? (
            <button
              className="sheet__item"
              type="button"
              disabled={busy}
              onClick={() =>
                void run(() => call("set_todo_completed", { todoId: occurrence.todoId, completed: !occurrence.todoCompleted }))
              }
            >
              <Icon name="check" size={17} /> {occurrence.todoCompleted ? "Mark task as not done" : "Mark task as done"}
            </button>
          ) : null}

          {occurrence.recurring ? (
            <>
              <button className="sheet__item" type="button" disabled={busy} onClick={() => setMode("day")}>
                <Icon name="edit" size={17} /> Edit this day only
              </button>
              <button
                className="sheet__item"
                type="button"
                disabled={busy}
                onClick={() => {
                  onEditEntry(occurrence.entryId);
                  onClose();
                }}
              >
                <Icon name="repeat" size={17} /> Edit whole series
              </button>
              <button
                className="sheet__item"
                type="button"
                disabled={busy}
                onClick={() => void run(() => call("skip_timetable_occurrence", { entryId: occurrence.entryId, date: occurrence.date }))}
              >
                <Icon name="skip" size={17} /> Skip this day
              </button>
              <button
                className="sheet__item sheet__item--danger"
                type="button"
                disabled={busy}
                onClick={() => void run(() => call("delete_timetable_entry", { entryId: occurrence.entryId }))}
              >
                <Icon name="trash" size={17} /> Delete whole series
              </button>
            </>
          ) : (
            <>
              <button
                className="sheet__item"
                type="button"
                disabled={busy}
                onClick={() => {
                  onEditEntry(occurrence.entryId);
                  onClose();
                }}
              >
                <Icon name="edit" size={17} /> Edit block
              </button>
              <button
                className="sheet__item sheet__item--danger"
                type="button"
                disabled={busy}
                onClick={() => void run(() => call("delete_timetable_entry", { entryId: occurrence.entryId }))}
              >
                <Icon name="trash" size={17} /> Delete block
              </button>
            </>
          )}
        </div>

        {error ? <span className="error-text">{error}</span> : null}
      </div>
    </Modal>
  );
}
