import trackmlnLogo from "../../assets/trackmln.svg";
import { phaseLabel, useFocus } from "../lib/hooks";
import { usePrefs } from "../lib/prefs";
import { formatClock } from "../utils/format";
import { call } from "../utils/ipc";
import { shortcutParts } from "../utils/shortcut";
import { Icon, IconName } from "../ui/Icon";

export type PageId = "home" | "screentime" | "focus" | "tasks" | "timetable" | "goals" | "settings";

const NAV: { id: PageId; label: string; icon: IconName }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "screentime", label: "Screen time", icon: "screen" },
  { id: "focus", label: "Focus", icon: "timer" },
  { id: "tasks", label: "Tasks", icon: "tasks" },
  { id: "timetable", label: "Timetable", icon: "calendar" },
  { id: "goals", label: "Limits", icon: "target" },
  { id: "settings", label: "Settings", icon: "sliders" }
];

type SidebarProps = {
  page: PageId;
  onChange: (page: PageId) => void;
};

export function Sidebar({ page, onChange }: SidebarProps) {
  const { snapshot, remaining } = useFocus();
  const { settings } = usePrefs();

  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <img className="sidebar__logo" src={trackmlnLogo} alt="" />
        <span className="sidebar__name">TrackMLN</span>
      </div>

      <nav className="sidebar__nav">
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nav-item${page === item.id ? " is-active" : ""}`}
            onClick={() => onChange(item.id)}
            aria-current={page === item.id ? "page" : undefined}
            title={item.label}
          >
            <Icon name={item.icon} size={19} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      <div className="sidebar__spacer" />

      {snapshot.active ? (
        <button className="sidebar__focus" type="button" onClick={() => onChange("focus")} title="Focus session">
          <strong>{formatClock(remaining)}</strong>
          <span>
            {phaseLabel(snapshot.phase)}
            {snapshot.status === "paused" ? " · paused" : ""}
          </span>
        </button>
      ) : null}

      <button className="sidebar__hint" type="button" onClick={() => void call("show_overlay")} title="Open quick overlay">
        {shortcutParts(settings?.hotkey ?? "control+shift+Space").map((part) => (
          <span className="kbd" key={part}>
            {part}
          </span>
        ))}
      </button>
    </aside>
  );
}
