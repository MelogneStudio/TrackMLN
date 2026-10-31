import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Onboarding } from "../onboarding/Onboarding";
import { usePrefs } from "../lib/prefs";
import { Home } from "../pages/Home";
import { Settings } from "../pages/Settings";
import { call } from "../utils/ipc";
import { PageId, Sidebar } from "./Sidebar";
import { Titlebar, useMaximized } from "./Titlebar";

function ComingSoon({ title }: { title: string }) {
  return (
    <div className="page">
      <h1 className="page-title">{title}</h1>
      <div className="empty">This screen is being built.</div>
    </div>
  );
}

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
        {page === "screentime" ? <ComingSoon title="Screen time" /> : null}
        {page === "focus" ? <ComingSoon title="Focus" /> : null}
        {page === "tasks" ? <ComingSoon title="Tasks" /> : null}
        {page === "timetable" ? <ComingSoon title="Timetable" /> : null}
        {page === "goals" ? <ComingSoon title="Limits" /> : null}
      </main>
    </div>
  );
}
