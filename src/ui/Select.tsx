import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

export type SelectOption<V extends string | number = string> = {
  value: V;
  label: string;
  sublabel?: string;
};

type SelectProps<V extends string | number> = {
  options: SelectOption<V>[];
  value: V | null;
  onChange: (value: V) => void;
  placeholder?: string;
  disabled?: boolean;
};

/** Custom dropdown: the native <select> popup can't be themed on WebView2. */
export function Select<V extends string | number>({
  options,
  value,
  onChange,
  placeholder = "Select…",
  disabled
}: SelectProps<V>) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find((option) => option.value === value);

  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  return (
    <div className={`select${open ? " is-open" : ""}`} ref={ref}>
      <button
        className="select__trigger"
        type="button"
        disabled={disabled}
        onClick={() => setOpen((current) => !current)}
      >
        <span className={`select__value${selected ? "" : " is-placeholder"}`}>
          {selected ? selected.label : placeholder}
        </span>
        <Icon className="select__chevron" name="chevronDown" size={16} />
      </button>

      {open ? (
        <div className="menu select__list">
          {options.length === 0 ? <div className="empty">Nothing to choose from</div> : null}
          {options.map((option) => (
            <button
              key={String(option.value)}
              className={`menu__item${option.value === value ? " is-selected" : ""}`}
              type="button"
              onClick={() => {
                onChange(option.value);
                setOpen(false);
              }}
            >
              <span>{option.label}</span>
              {option.sublabel ? <span className="select__sub">{option.sublabel}</span> : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
