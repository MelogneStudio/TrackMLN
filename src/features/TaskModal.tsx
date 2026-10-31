import { useEffect, useRef, useState } from "react";
import type { Recurrence, Todo, TodoDraft, TodoTemplate, TodoTemplateDraft } from "../types";
import { Modal } from "../ui/Modal";
import { Segmented, Toggle } from "../ui/Controls";
import { todayIso } from "../utils/dates";
import { minuteToTimeInput, timeInputToMinute } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";
import { newRecurrence } from "../utils/recurrence";
import { RecurrenceEditor } from "./RecurrenceEditor";

type TaskModalProps = {
  /** Editing a single task. */
  todo?: Todo | null;
  /** Editing a repeat rule. */
  template?: TodoTemplate | null;
  /** Pre-filled due date for brand-new tasks. */
  defaultDate?: string | null;
  onClose: () => void;
  onSaved: () => void;
};

const PRIORITIES = [
  { value: 0, label: "None" },
  { value: 1, label: "Low" },
  { value: 2, label: "Medium" },
  { value: 3, label: "High" }
];

export function TaskModal({ todo, template, defaultDate, onClose, onSaved }: TaskModalProps) {
  const source = todo ?? template ?? null;
  const editingTemplate = Boolean(template);

  const [title, setTitle] = useState(source?.title ?? "");
  const [notes, setNotes] = useState(source?.notes ?? "");
  const [priority, setPriority] = useState(source?.priority ?? 0);
  const [dueDate, setDueDate] = useState(todo?.dueDate ?? (source ? "" : (defaultDate ?? "")));
  const [dueTime, setDueTime] = useState(minuteToTimeInput(source?.dueMinute ?? null));
  const [estimate, setEstimate] = useState(source?.estimateMinutes ? String(source.estimateMinutes) : "");
  const [repeat, setRepeat] = useState(editingTemplate);
  const [recurrence, setRecurrence] = useState<Recurrence>(template?.recurrence ?? newRecurrence(defaultDate || todayIso()));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    window.setTimeout(() => titleRef.current?.focus(), 40);
  }, []);

  const save = async () => {
    if (!title.trim()) {
      setError("Give the task a title.");
      return;
    }

    const estimateMinutes = estimate.trim() ? Math.round(Number(estimate)) : null;
    if (estimateMinutes !== null && (!Number.isFinite(estimateMinutes) || estimateMinutes <= 0)) {
      setError("Estimate must be a positive number of minutes.");
      return;
    }

    const dueMinute = timeInputToMinute(dueTime);
    if (dueTime && dueMinute === null) {
      setError("That due time isn't valid.");
      return;
    }
    if (!repeat && dueMinute !== null && !dueDate) {
      setError("Pick a due date to go with the due time.");
      return;
    }
    if (repeat && recurrence.kind === "weekly" && recurrence.weekdays.length === 0) {
      setError("Pick at least one weekday.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      if (repeat) {
        const draft: TodoTemplateDraft = {
          id: template?.id ?? null,
          title: title.trim(),
          notes,
          priority,
          dueMinute,
          estimateMinutes,
          recurrence,
          active: template?.active ?? true
        };
        await call("save_todo_template", { draft });
      } else {
        const draft: TodoDraft = {
          id: todo?.id ?? null,
          title: title.trim(),
          notes,
          priority,
          dueDate: dueDate || null,
          dueMinute,
          estimateMinutes
        };
        await call("save_todo_item", { draft });
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const heading = editingTemplate ? "Edit repeating task" : todo ? "Edit task" : "New task";

  return (
    <Modal
      title={heading}
      onClose={onClose}
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
          <span>Title</span>
          <input
            ref={titleRef}
            className="input"
            value={title}
            maxLength={200}
            placeholder="What needs doing?"
            onChange={(event) => setTitle(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void save();
            }}
          />
        </label>

        <label className="field">
          <span>Notes</span>
          <textarea className="input" value={notes} placeholder="Optional details" onChange={(event) => setNotes(event.target.value)} />
        </label>

        <div className="field">
          <span>Priority</span>
          <Segmented value={priority} onChange={setPriority} options={PRIORITIES} />
        </div>

        <div className="grid-3">
          {!repeat ? (
            <label className="field">
              <span>Due date</span>
              <input className="input" type="date" value={dueDate} onChange={(event) => setDueDate(event.target.value)} />
            </label>
          ) : null}
          <label className="field">
            <span>Due time</span>
            <input className="input" type="time" value={dueTime} onChange={(event) => setDueTime(event.target.value)} />
          </label>
          <label className="field">
            <span>Estimate (min)</span>
            <input
              className="input"
              type="number"
              min={1}
              placeholder="30"
              value={estimate}
              onChange={(event) => setEstimate(event.target.value)}
            />
          </label>
        </div>

        {!editingTemplate && !todo ? (
          <div className="row" style={{ justifyContent: "space-between" }}>
            <div>
              <div>Repeat</div>
              <div className="faint" style={{ fontSize: 12 }}>
                Creates a new copy of this task on a schedule.
              </div>
            </div>
            <Toggle on={repeat} onChange={setRepeat} label="Repeat" />
          </div>
        ) : null}

        {repeat ? <RecurrenceEditor value={recurrence} onChange={setRecurrence} /> : null}

        {error ? <span className="error-text">{error}</span> : null}
      </div>
    </Modal>
  );
}
