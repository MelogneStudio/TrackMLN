import type { Recurrence, RecurrenceKind } from "../types";
import { fromIso, WEEKDAYS_SHORT } from "../utils/dates";
import { Select } from "../ui/Select";

type RecurrenceEditorProps = {
  value: Recurrence;
  onChange: (value: Recurrence) => void;
};

const KINDS: { value: RecurrenceKind; label: string }[] = [
  { value: "daily", label: "Day(s)" },
  { value: "weekly", label: "Week(s)" },
  { value: "monthly", label: "Month(s)" }
];

export function RecurrenceEditor({ value, onChange }: RecurrenceEditorProps) {
  const patch = (next: Partial<Recurrence>) => onChange({ ...value, ...next });

  const changeKind = (kind: RecurrenceKind) => {
    const startDay = fromIso(value.startDate).getDate();
    patch({
      kind,
      weekdays: kind === "weekly" && value.weekdays.length === 0 ? [(fromIso(value.startDate).getDay() + 6) % 7] : value.weekdays,
      monthDay: kind === "monthly" ? (value.monthDay ?? startDay) : null
    });
  };

  const toggleWeekday = (day: number) => {
    const has = value.weekdays.includes(day);
    const next = has ? value.weekdays.filter((entry) => entry !== day) : [...value.weekdays, day];
    patch({ weekdays: next.sort((a, b) => a - b) });
  };

  return (
    <div className="recur">
      <div className="recur__line">
        <span className="dim">Every</span>
        <input
          className="input recur__num"
          type="number"
          min={1}
          max={365}
          value={value.interval}
          onChange={(event) => patch({ interval: Math.max(1, Math.min(365, Number(event.target.value) || 1)) })}
        />
        <div className="recur__kind">
          <Select options={KINDS} value={value.kind} onChange={changeKind} />
        </div>
      </div>

      {value.kind === "weekly" ? (
        <div className="recur__days" role="group" aria-label="Repeat on">
          {WEEKDAYS_SHORT.map((label, day) => (
            <button
              key={label}
              type="button"
              className={`recur__day${value.weekdays.includes(day) ? " is-on" : ""}`}
              aria-pressed={value.weekdays.includes(day)}
              onClick={() => toggleWeekday(day)}
            >
              {label}
            </button>
          ))}
        </div>
      ) : null}

      {value.kind === "monthly" ? (
        <div className="recur__line">
          <span className="dim">On day</span>
          <input
            className="input recur__num"
            type="number"
            min={1}
            max={31}
            value={value.monthDay ?? 1}
            onChange={(event) => patch({ monthDay: Math.max(1, Math.min(31, Number(event.target.value) || 1)) })}
          />
        </div>
      ) : null}

      <div className="grid-2">
        <label className="field">
          <span>Starts</span>
          <input
            className="input"
            type="date"
            value={value.startDate}
            onChange={(event) => event.target.value && patch({ startDate: event.target.value })}
          />
        </label>
        <label className="field">
          <span>Ends (optional)</span>
          <input
            className="input"
            type="date"
            value={value.endDate ?? ""}
            min={value.startDate}
            onChange={(event) => patch({ endDate: event.target.value || null })}
          />
        </label>
      </div>
    </div>
  );
}
