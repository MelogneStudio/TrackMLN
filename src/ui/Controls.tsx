type ToggleProps = {
  on: boolean;
  onChange: (on: boolean) => void;
  label?: string;
  disabled?: boolean;
};

export function Toggle({ on, onChange, label, disabled }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      className={`toggle${on ? " is-on" : ""}`}
      onClick={() => onChange(!on)}
    />
  );
}

type SegmentedProps<V extends string | number> = {
  options: { value: V; label: string }[];
  value: V;
  onChange: (value: V) => void;
};

export function Segmented<V extends string | number>({ options, value, onChange }: SegmentedProps<V>) {
  return (
    <div className="segmented" role="tablist">
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          role="tab"
          aria-selected={option.value === value}
          className={option.value === value ? "is-active" : ""}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
