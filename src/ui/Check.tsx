import { Icon } from "./Icon";

type CheckProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
};

export function Check({ checked, onChange, label, disabled }: CheckProps) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`check${checked ? " is-checked" : ""}`}
      onClick={(event) => {
        event.stopPropagation();
        onChange(!checked);
      }}
    >
      {checked ? <Icon name="check" size={13} strokeWidth={2.6} /> : null}
    </button>
  );
}
