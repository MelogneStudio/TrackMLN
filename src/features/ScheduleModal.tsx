import { useState } from "react";
import { useFocusPresets, usePoll } from "../lib/hooks";
import type { FreeSlot, Todo } from "../types";
import { Modal } from "../ui/Modal";
import { Select } from "../ui/Select";
import { Toggle } from "../ui/Controls";
import { nowMinute, todayIso } from "../utils/dates";
import { formatMinuteOfDay, formatMinutes, minuteToTimeInput, timeInputToMinute } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";

type ScheduleModalProps = {
  todo: Todo;
  onClose: () => void;
  onDone: () => void;
};

/** Puts a task into the timetable as a block, optionally auto-starting a focus preset. */
export function ScheduleModal({ todo, onClose, onDone }: ScheduleModalProps) {
  const initialDate = todo.dueDate && todo.dueDate >= todayIso() ? todo.dueDate : todayIso();
  const [date, setDate] = useState(initialDate);
  const [duration, setDuration] = useState(String(todo.estimateMinutes ?? 30));
  const [start, setStart] = useState(() => {
    const next = Math.ceil((nowMinute() + 5) / 15) * 15;
    return minuteToTimeInput(Math.min(next, 22 * 60));
  });
  const [presetId, setPresetId] = useState(0);
  const [autoStart, setAutoStart] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const durationMinutes = Math.max(5, Math.round(Number(duration) || 0));
  const presets = useFocusPresets();
  const slots = usePoll<FreeSlot[]>(
    () => call<FreeSlot[]>("get_free_slots", { date, minMinutes: durationMinutes }),
    0,
    [date, durationMinutes]
  );

  const save = async () => {
    const startMinute = timeInputToMinute(start);
    if (startMinute === null) {
      setError("Pick a start time.");
      return;
    }
    if (!Number.isFinite(durationMinutes) || durationMinutes <= 0) {
      setError("Duration must be a positive number of minutes.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await call("schedule_todo_block", {
        todoId: todo.id,
        date,
        startMinute,
        durationMinutes,
        presetId: presetId || null,
        autoStart
      });
      onDone();
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const unschedule = async () => {
    setBusy(true);
    try {
      await call("unschedule_todo_blocks", { todoId: todo.id });
      onDone();
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const presetOptions = [
    { value: 0, label: "No focus preset" },
    ...(presets.data ?? []).map((preset) => ({ value: preset.id ?? 0, label: preset.name, sublabel: `${preset.focusMinutes}m` }))
  ];

  return (
    <Modal
      title="Schedule task"
      onClose={onClose}
      footer={
        <>
          <button className="btn btn--ghost btn--danger" type="button" disabled={busy} onClick={() => void unschedule()}>
            Remove blocks
          </button>
          <div className="grow" />
          <button className="btn" type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--accent" type="button" disabled={busy} onClick={() => void save()}>
            Schedule
          </button>
        </>
      }
    >
      <div className="stack">
        <div className="dim">{todo.title}</div>

        <div className="grid-3">
          <label className="field">
            <span>Date</span>
            <input className="input" type="date" value={date} onChange={(event) => event.target.value && setDate(event.target.value)} />
          </label>
          <label className="field">
            <span>Start</span>
            <input className="input" type="time" value={start} onChange={(event) => setStart(event.target.value)} />
          </label>
          <label className="field">
            <span>Length (min)</span>
            <input className="input" type="number" min={5} step={5} value={duration} onChange={(event) => setDuration(event.target.value)} />
          </label>
        </div>

        <div className="field">
          <span>Free slots that fit {formatMinutes(durationMinutes)}</span>
          <div className="row row--wrap">
            {(slots.data ?? []).length === 0 ? <span className="faint">{slots.loading ? "Checking…" : "No free slot that long today"}</span> : null}
            {(slots.data ?? []).slice(0, 8).map((slot) => (
              <button
                key={slot.startMinute}
                type="button"
                className={`chip chip--button${timeInputToMinute(start) === slot.startMinute ? " chip--accent" : ""}`}
                onClick={() => setStart(minuteToTimeInput(slot.startMinute))}
              >
                {formatMinuteOfDay(slot.startMinute)} – {formatMinuteOfDay(slot.endMinute)}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <span>Focus preset</span>
          <Select options={presetOptions} value={presetId} onChange={setPresetId} />
        </div>

        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div>Auto-start focus</div>
            <div className="faint" style={{ fontSize: 12 }}>
              Begin the preset by itself when this block starts.
            </div>
          </div>
          <Toggle on={autoStart} onChange={setAutoStart} label="Auto-start focus" disabled={presetId === 0} />
        </div>

        {error ? <span className="error-text">{error}</span> : null}
      </div>
    </Modal>
  );
}
