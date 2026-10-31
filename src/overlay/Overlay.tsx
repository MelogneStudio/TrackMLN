import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { AppsList } from "../features/AppsList";
import { FocusMini } from "../features/FocusMini";
import { MiniTasks } from "../features/MiniTasks";
import { MiniTimetable } from "../features/MiniTimetable";
import { useClock, useTodayUsage } from "../lib/hooks";
import { usePrefs } from "../lib/prefs";
import type { PageId } from "../shell/Sidebar";
import type { FocusPhase } from "../types";
import { BarChart } from "../ui/BarChart";
import { Icon } from "../ui/Icon";
import { formatDuration, formatHeroDuration, formatHour, greetingFor } from "../utils/format";
import { formatDateShort, todayIso } from "../utils/dates";
import { call } from "../utils/ipc";

const overlayWindow = getCurrentWindow();

/** Only poll while the overlay is on screen; it lives hidden most of the time. */
function useOverlayActive(): boolean {
  const [active, setActive] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const disposers: Array<() => void> = [];
    const track = (promise: Promise<() => void>) =>
      void promise.then((unlisten) => {
        if (cancelled) unlisten();
        else disposers.push(unlisten);
      });

    track(listen("overlay-shown", () => setActive(true)));
    track(overlayWindow.onFocusChanged(({ payload }) => setActive(payload)));

    return () => {
      cancelled = true;
      disposers.forEach((dispose) => dispose());
    };
  }, []);

  return active;
}

export function Overlay() {
  const { prefs, loaded } = usePrefs();
  const active = useOverlayActive();
  const usage = useTodayUsage(active, 2000);
  const now = useClock(15_000);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: number | null = null;
    const disposers: Array<() => void> = [];
    const show = (message: string) => {
      setNotice(message);
      if (timer !== null) window.clearTimeout(timer);
      timer = window.setTimeout(() => setNotice(null), 12_000);
    };

    void listen<{ ended: FocusPhase; next: FocusPhase | null; completed: boolean }>("focus-phase-ended", (event) => {
      const { ended, next, completed } = event.payload;
      if (ended === "focus") {
        show(
          !completed
            ? "Focus session ended early."
            : next === "longBreak"
              ? "Great work. Time for a long break."
              : "Focus session done. Take a short break."
        );
      } else {
        show(next === "focus" ? "Break over. Ready for the next session?" : "Break finished.");
      }
    }).then((unlisten) => (cancelled ? unlisten() : disposers.push(unlisten)));

    void listen<{ focusSessions: number }>("focus-finished", (event) => {
      show(`All done: ${event.payload.focusSessions} focus sessions completed. Well done.`);
    }).then((unlisten) => (cancelled ? unlisten() : disposers.push(unlisten)));

    return () => {
      cancelled = true;
      if (timer !== null) window.clearTimeout(timer);
      disposers.forEach((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void call("hide_overlay");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const openApp = async (page?: PageId) => {
    await call("show_app_window");
    if (page) {
      await emit("navigate", { page });
    }
  };

  if (!loaded) {
    return null;
  }

  const total = usage.data?.total ?? 0;
  const hero = formatHeroDuration(total);
  const hourly = usage.data?.hourly ?? Array.from({ length: 24 }, (_, hour) => ({ hour, total: 0 }));

  return (
    <div className="ov">
      <header className="ov__head" data-tauri-drag-region>
        <div data-tauri-drag-region>
          <h1 className="ov__hello display">
            {greetingFor(now)}
            {prefs.nickname ? `, ${prefs.nickname}` : ""} <span aria-hidden="true">👋</span>
          </h1>
          <p className="faint ov__date" data-tauri-drag-region>
            {formatDateShort(todayIso())} ·{" "}
            {now.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
          </p>
        </div>
        <div className="row">
          <button className="btn btn--sm" type="button" onClick={() => void openApp()}>
            <Icon name="layout" size={15} /> Open app
          </button>
          <button className="icon-btn" type="button" aria-label="Close overlay" onClick={() => void call("hide_overlay")}>
            <Icon name="x" />
          </button>
        </div>
      </header>

      {notice ? (
        <div className="ov__notice" role="status">
          <Icon name="bolt" size={16} />
          <span className="grow">{notice}</span>
          <button className="icon-btn icon-btn--sm" type="button" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="x" size={14} />
          </button>
        </div>
      ) : null}

      <div className="ov__grid">
        <section className="ov__card ov__card--screen">
          <button className="ov__title" type="button" onClick={() => void openApp("screentime")}>
            Screen time
          </button>
          <div className="stat-big ov__big">{hero.hours > 0 ? `${hero.hours}hr ${hero.minutes}m` : `${hero.minutes}m`}</div>
          <div className="ov__chart">
            <BarChart
              values={hourly.map((entry) => entry.total)}
              height={64}
              highlight={now.getHours()}
              formatTip={(value, index) => `${formatHour(index)} · ${formatDuration(value)}`}
            />
          </div>
          <div className="ov__apps">
            <AppsList apps={usage.data?.apps ?? []} limit={3} />
          </div>
        </section>

        <div className="ov__col">
          <section className="ov__card">
            <button className="ov__title" type="button" onClick={() => void openApp("focus")}>
              Focus
            </button>
            <FocusMini enabled={active} onOpen={() => void openApp("focus")} />
          </section>
          <section className="ov__card">
            <button className="ov__title" type="button" onClick={() => void openApp("timetable")}>
              Timetable
            </button>
            <MiniTimetable enabled={active} onOpen={() => void openApp("timetable")} />
          </section>
        </div>

        <section className="ov__card">
          <button className="ov__title" type="button" onClick={() => void openApp("tasks")}>
            Tasks
          </button>
          <MiniTasks limit={5} enabled={active} onOpen={() => void openApp("tasks")} />
        </section>
      </div>
    </div>
  );
}
