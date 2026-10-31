import { useState } from "react";
import type { FocusPreset } from "../types";
import { Modal } from "../ui/Modal";
import { Toggle } from "../ui/Controls";
import { call, errorMessage } from "../utils/ipc";

type PresetModalProps = {
  preset: FocusPreset | null;
  onClose: () => void;
  onSaved: () => void;
};

const BLANK: FocusPreset = {
  id: null,
  name: "",
  focusMinutes: 25,
  shortBreakMinutes: 5,
  longBreakMinutes: 15,
  sessionsUntilLongBreak: 4,
  totalSessions: 4,
  autoStartBreaks: true,
  autoStartFocus: false,
  isDefault: false
};

function NumberField({
  label,
  value,
  min,
  max,
  onChange
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <input
        className="input"
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

export function PresetModal({ preset, onClose, onSaved }: PresetModalProps) {
  const [draft, setDraft] = useState<FocusPreset>(preset ?? BLANK);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const patch = (next: Partial<FocusPreset>) => setDraft((current) => ({ ...current, ...next }));

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await call("save_focus_preset", { preset: draft });
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
      title={preset ? "Edit preset" : "New preset"}
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
          <span>Name</span>
          <input
            className="input"
            value={draft.name}
            maxLength={40}
            placeholder="Deep work"
            autoFocus
            onChange={(event) => patch({ name: event.target.value })}
          />
        </label>

        <div className="grid-3">
          <NumberField label="Focus (min)" value={draft.focusMinutes} min={1} max={240} onChange={(focusMinutes) => patch({ focusMinutes })} />
          <NumberField
            label="Short break (min)"
            value={draft.shortBreakMinutes}
            min={1}
            max={120}
            onChange={(shortBreakMinutes) => patch({ shortBreakMinutes })}
          />
          <NumberField
            label="Long break (min)"
            value={draft.longBreakMinutes}
            min={1}
            max={240}
            onChange={(longBreakMinutes) => patch({ longBreakMinutes })}
          />
        </div>

        <div className="grid-2">
          <NumberField
            label="Sessions per long break"
            value={draft.sessionsUntilLongBreak}
            min={1}
            max={20}
            onChange={(sessionsUntilLongBreak) => patch({ sessionsUntilLongBreak })}
          />
          <NumberField label="Total sessions" value={draft.totalSessions} min={1} max={50} onChange={(totalSessions) => patch({ totalSessions })} />
        </div>

        <div className="stack stack--sm">
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span>Start breaks automatically</span>
            <Toggle on={draft.autoStartBreaks} onChange={(autoStartBreaks) => patch({ autoStartBreaks })} label="Auto-start breaks" />
          </div>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span>Start next focus automatically</span>
            <Toggle on={draft.autoStartFocus} onChange={(autoStartFocus) => patch({ autoStartFocus })} label="Auto-start focus" />
          </div>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span>Use as default preset</span>
            <Toggle on={draft.isDefault} onChange={(isDefault) => patch({ isDefault })} label="Default preset" />
          </div>
        </div>

        {error ? <span className="error-text">{error}</span> : null}
      </div>
    </Modal>
  );
}
