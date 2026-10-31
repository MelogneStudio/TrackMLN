import type { AppTotal } from "../types";
import { formatDuration } from "../utils/format";

type AppsListProps = {
  apps: AppTotal[];
  limit?: number;
  /** Bars are relative to this value; defaults to the largest app. */
  max?: number;
  onSelect?: (app: AppTotal) => void;
  selectedId?: string | null;
};

export function AppsList({ apps, limit = 5, max, onSelect, selectedId }: AppsListProps) {
  const visible = apps.filter((app) => app.app_identity !== "idle" && app.app_identity !== "unknown").slice(0, limit);
  const ceiling = max ?? Math.max(1, ...visible.map((app) => app.total));

  if (visible.length === 0) {
    return (
      <div className="empty">
        <strong>No activity yet</strong>
        Apps you use will show up here.
      </div>
    );
  }

  return (
    <div className="rank">
      {visible.map((app) => {
        const content = (
          <>
            <span className="rank__name">{app.app_name}</span>
            <span className="rank__value">{formatDuration(app.total)}</span>
            <div className="meter">
              <span style={{ width: `${Math.max(3, (app.total / ceiling) * 100)}%` }} />
            </div>
          </>
        );

        return onSelect ? (
          <button
            type="button"
            className={`rank__row rank__row--button${selectedId === app.app_identity ? " is-selected" : ""}`}
            key={app.app_identity}
            onClick={() => onSelect(app)}
          >
            {content}
          </button>
        ) : (
          <div className="rank__row" key={app.app_identity}>
            {content}
          </div>
        );
      })}
    </div>
  );
}
