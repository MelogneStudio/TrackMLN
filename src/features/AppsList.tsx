import type { AppTotal } from "../types";
import { formatDuration } from "../utils/format";

type AppsListProps = {
  apps: AppTotal[];
  limit?: number;
  /** Bars are relative to this value; defaults to the largest app. */
  max?: number;
};

export function AppsList({ apps, limit = 5, max }: AppsListProps) {
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
      {visible.map((app) => (
        <div className="rank__row" key={app.app_identity}>
          <span className="rank__name">{app.app_name}</span>
          <span className="rank__value">{formatDuration(app.total)}</span>
          <div className="meter">
            <span style={{ width: `${Math.max(3, (app.total / ceiling) * 100)}%` }} />
          </div>
        </div>
      ))}
    </div>
  );
}
