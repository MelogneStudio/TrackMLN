import { useState } from "react";

type BarChartProps = {
  values: number[];
  /** One label per bar, or a sparse list rendered under the bars. */
  labels?: string[];
  /** Show a label only every n bars (default 1). */
  labelEvery?: number;
  height?: number;
  /** Index drawn in the full accent colour; others are dimmer. */
  highlight?: number | null;
  formatTip?: (value: number, index: number) => string;
  onSelect?: (index: number) => void;
  mono?: boolean;
};

export function BarChart({
  values,
  labels,
  labelEvery = 1,
  height = 120,
  highlight = null,
  formatTip,
  onSelect,
  mono = true
}: BarChartProps) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(1, ...values);

  return (
    <div className="bars">
      <div className="bars__plot" style={{ height }} onMouseLeave={() => setHover(null)}>
        {values.map((value, index) => {
          const ratio = value / max;
          const active = highlight === null || highlight === index || hover === index;
          return (
            <button
              key={index}
              type="button"
              className={`bars__col${active ? " is-active" : ""}${onSelect ? " is-clickable" : ""}`}
              onMouseEnter={() => setHover(index)}
              onClick={() => onSelect?.(index)}
              tabIndex={onSelect ? 0 : -1}
              aria-label={formatTip ? formatTip(value, index) : String(value)}
            >
              <span className="bars__bar" style={{ height: `max(4px, ${ratio * 100}%)` }} />
            </button>
          );
        })}
        {hover !== null && formatTip ? (
          <div
            className="bars__tip"
            style={{ left: `${((hover + 0.5) / values.length) * 100}%` }}
          >
            {formatTip(values[hover], hover)}
          </div>
        ) : null}
      </div>
      {labels ? (
        <div className={`bars__labels${mono ? " mono" : ""}`}>
          {labels.map((label, index) => (
            <span key={index} style={{ visibility: index % labelEvery === 0 ? "visible" : "hidden" }}>
              {label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}
