import { listen } from "@tauri-apps/api/event";
import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { AppSettings } from "../types";
import { call } from "../utils/ipc";
import { applyTheme, themeById, ThemeId, THEMES } from "./theme";

export type Prefs = {
  nickname: string;
  theme: ThemeId;
  /** 0 = solid sidebar, 100 = very translucent. */
  sidebarTranslucency: number;
  onboardingDone: boolean;
  /** Target for the "time focused" ring, in minutes. */
  focusGoalMinutes: number;
  /** Pop the overlay up when a focus phase ends. */
  focusPopup: boolean;
};

export const DEFAULT_PREFS: Prefs = {
  nickname: "",
  theme: "grey",
  sidebarTranslucency: 55,
  onboardingDone: false,
  focusGoalMinutes: 120,
  focusPopup: true
};

export function parsePrefs(raw: string | null | undefined): Prefs {
  if (!raw) {
    return DEFAULT_PREFS;
  }

  try {
    const value = JSON.parse(raw) as Partial<Prefs>;
    const theme = THEMES.some((entry) => entry.id === value.theme) ? (value.theme as ThemeId) : DEFAULT_PREFS.theme;
    const translucency =
      typeof value.sidebarTranslucency === "number"
        ? Math.max(0, Math.min(100, value.sidebarTranslucency))
        : DEFAULT_PREFS.sidebarTranslucency;

    return {
      nickname: typeof value.nickname === "string" ? value.nickname.slice(0, 40) : "",
      theme,
      sidebarTranslucency: translucency,
      onboardingDone: value.onboardingDone === true,
      focusPopup: value.focusPopup !== false,
      focusGoalMinutes:
        typeof value.focusGoalMinutes === "number" && value.focusGoalMinutes >= 5 && value.focusGoalMinutes <= 1440
          ? Math.round(value.focusGoalMinutes)
          : DEFAULT_PREFS.focusGoalMinutes
    };
  } catch {
    return DEFAULT_PREFS;
  }
}

type PrefsContextValue = {
  prefs: Prefs;
  loaded: boolean;
  settings: AppSettings | null;
  setSettings: (settings: AppSettings) => void;
  update: (patch: Partial<Prefs>) => void;
};

const PrefsContext = createContext<PrefsContextValue | null>(null);

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [prefs, setPrefs] = useState<Prefs>(DEFAULT_PREFS);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loaded, setLoaded] = useState(false);
  const latest = useRef<Prefs>(DEFAULT_PREFS);
  const saveTimer = useRef<number | null>(null);
  const sent = useRef<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;

    call<AppSettings>("get_settings")
      .then((value) => {
        if (cancelled) return;
        const next = parsePrefs(value.uiPrefs);
        latest.current = next;
        setPrefs(next);
        setSettings(value);
        setLoaded(true);
      })
      .catch((error) => {
        console.error("Failed to load settings", error);
        if (!cancelled) setLoaded(true);
      });

    let dispose: (() => void) | undefined;
    void listen<string>("ui-prefs-changed", (event) => {
      // Our own save echoes back once; consume it so a later identical change from the other window still applies.
      if (sent.current.delete(event.payload)) {
        return;
      }
      const next = parsePrefs(event.payload);
      latest.current = next;
      setPrefs(next);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else dispose = unlisten;
    });

    return () => {
      cancelled = true;
      dispose?.();
    };
  }, []);

  useEffect(() => {
    const theme = applyTheme(prefs.theme, prefs.sidebarTranslucency);
    void call("set_chrome_dark", { dark: theme.mode === "dark" }).catch(() => undefined);
  }, [prefs.theme, prefs.sidebarTranslucency]);

  const update = useCallback((patch: Partial<Prefs>) => {
    const next = { ...latest.current, ...patch };
    latest.current = next;
    setPrefs(next);

    if (saveTimer.current !== null) {
      window.clearTimeout(saveTimer.current);
    }
    saveTimer.current = window.setTimeout(() => {
      const payload = JSON.stringify(latest.current);
      if (sent.current.size > 40) sent.current.clear();
      sent.current.add(payload);
      call<AppSettings>("set_ui_prefs", { prefs: payload })
        .then((value) => setSettings(value))
        .catch((error) => console.error("Failed to save preferences", error));
    }, 220);
  }, []);

  const value = useMemo(
    () => ({ prefs, loaded, settings, setSettings, update }),
    [prefs, loaded, settings, update]
  );

  return <PrefsContext.Provider value={value}>{children}</PrefsContext.Provider>;
}

export function usePrefs(): PrefsContextValue {
  const context = useContext(PrefsContext);
  if (!context) {
    throw new Error("usePrefs must be used inside PrefsProvider");
  }
  return context;
}

export function useTheme() {
  const { prefs } = usePrefs();
  return themeById(prefs.theme);
}
