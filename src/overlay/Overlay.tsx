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
