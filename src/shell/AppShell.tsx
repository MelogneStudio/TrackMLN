import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { Onboarding } from "../onboarding/Onboarding";
import { usePrefs } from "../lib/prefs";
import { Home } from "../pages/Home";
import { Focus } from "../pages/Focus";
import { Goals } from "../pages/Goals";
import { ScreenTime } from "../pages/ScreenTime";
import { Settings } from "../pages/Settings";
import { Tasks } from "../pages/Tasks";
import { Timetable } from "../pages/Timetable";
import { call } from "../utils/ipc";
import { PageId, Sidebar } from "./Sidebar";
import { Titlebar, useMaximized } from "./Titlebar";

const PAGE_IDS: PageId[] = ["home", "screentime", "focus", "tasks", "timetable", "goals", "settings"];

export function AppShell() {
  const { prefs, loaded } = usePrefs();
  const maximized = useMaximized();
  const [page, setPage] = useState<PageId>("home");

  useEffect(() => {
    if (loaded && !prefs.onboardingDone) {
      void call("show_app_window");
    }
  }, [loaded, prefs.onboardingDone]);

  useEffect(() => {
    let cancelled = false;
    let dispose: (() => void) | undefined;
    void listen<{ page: PageId }>("navigate", (event) => {
      if (PAGE_IDS.includes(event.payload.page)) {
        setPage(event.payload.page);
      }
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else dispose = unlisten;
    });
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, []);

  const popupRef = useRef(prefs.focusPopup);
  popupRef.current = prefs.focusPopup;

  useEffect(() => {
    let cancelled = false;
    const disposers: Array<() => void> = [];
    const popup = () => {
      if (popupRef.current) void call("show_overlay");
    };
    for (const name of ["focus-phase-ended", "focus-finished"]) {
      void listen(name, popup).then((unlisten) => {
        if (cancelled) unlisten();
        else disposers.push(unlisten);
      });
    }
    return () => {
      cancelled = true;
      disposers.forEach((dispose) => dispose());
    };
  }, []);

  if (!loaded) {
    return null;
  }

  const frameClass = `win${maximized ? " is-max" : ""}`;

  if (!prefs.onboardingDone) {
    return (
      <div className={`${frameClass} win--solo`}>
        <div className="win__bg" />
        <Titlebar maximized={maximized} />
        <main className="win__main">
          <Onboarding />
        </main>
      </div>
    );
  }

  return (
    <div className={frameClass}>
      <Titlebar maximized={maximized} onOpenSettings={() => setPage("settings")} />
      <Sidebar page={page} onChange={setPage} />
      <main className="win__main">
        {page === "home" ? <Home onNavigate={setPage} /> : null}
        {page === "settings" ? <Settings /> : null}
        {page === "screentime" ? <ScreenTime /> : null}
        {page === "focus" ? <Focus /> : null}
        {page === "tasks" ? <Tasks onNavigate={setPage} /> : null}
        {page === "timetable" ? <Timetable onNavigate={setPage} /> : null}
        {page === "goals" ? <Goals /> : null}
      </main>
    </div>
  );
}
