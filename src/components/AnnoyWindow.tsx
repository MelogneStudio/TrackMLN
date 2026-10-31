import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import type { GoalAlertPayload } from "../types";
import { formatLongDuration } from "../utils/format";

const appWindow = getCurrentWindow();

const SNOOZE_DURATIONS = [5, 3, 1];
const MESSAGES = [
  {
    heading: (label: string) => `${label} is officially a problem.`,
    sub: (_snoozeNumber: number) => "You crossed your threshold."
  },
  {
    heading: (label: string) => `Still on ${label}?`,
    sub: (_snoozeNumber: number) => "Touch grass."
  },
  {
    heading: (label: string) => `${label}. Again.`,
    sub: (snoozeNumber: number) => `This is snooze #${snoozeNumber}.`
  },
];

function getSnoozeMinutes(snoozeCount: number) {
  return SNOOZE_DURATIONS[Math.min(snoozeCount, SNOOZE_DURATIONS.length - 1)];
}

export function AnnoyWindow() {
  const [alert, setAlert] = useState<GoalAlertPayload | null>(null);
  const [busy, setBusy] = useState(false);
  const [instanceKey, setInstanceKey] = useState(0);
  const [messageIndex, setMessageIndex] = useState(0);
  const [drifted, setDrifted] = useState(false);
  const [snoozeReady, setSnoozeReady] = useState(false);
  const lastAlertAtRef = useRef(0);
  const lastGoalIdRef = useRef<number | null>(null);
  const closeActionRef = useRef({
    alert: null as GoalAlertPayload | null,
    busy: false,
    snoozeMinutes: SNOOZE_DURATIONS[0],
    snoozeReady: false
  });

  const snoozeCount = alert?.snoozeCount ?? 0;
  const snoozeNumber = snoozeCount + 1;
  const snoozeMinutes = getSnoozeMinutes(snoozeCount);
  const msg = MESSAGES[messageIndex];

  closeActionRef.current = {
    alert,
    busy,
    snoozeMinutes,
    snoozeReady
  };

  useEffect(() => {
    let dispose: (() => void) | undefined;

    const showAlert = (payload: GoalAlertPayload) => {
      const now = Date.now();
      const sameGoal = lastGoalIdRef.current === payload.goalId;
      const shouldRefresh = !sameGoal || now - lastAlertAtRef.current > 1500;

      lastGoalIdRef.current = payload.goalId;
      lastAlertAtRef.current = now;
      setAlert(payload);
      setBusy(false);

      if (shouldRefresh) {
        setMessageIndex(Math.floor(Math.random() * MESSAGES.length));
        setInstanceKey((v) => v + 1);
      }
    };

    void invoke<GoalAlertPayload | null>("get_pending_alert", { label: "annoy" })
      .then((payload) => {
        if (payload) {
          showAlert(payload);
        }
      })
      .catch((error) => { console.error("Failed to fetch pending annoy alert", error); });

    void listen<GoalAlertPayload>("annoy-alert", (event) => {
      showAlert(event.payload);
    })
      .then((unlisten) => { dispose = unlisten; })
      .catch((error) => { console.error("Failed to subscribe to annoy alerts", error); });

    return () => { dispose?.(); };
  }, []);

  // Snooze-ready delay
  useEffect(() => {
    if (!alert) return;
    setDrifted(false);
    setSnoozeReady(false);
    const t = window.setTimeout(() => setSnoozeReady(true), 3000 + Math.random() * 2000);
    return () => window.clearTimeout(t);
  }, [alert, instanceKey]);

  const hide = async () => {
    try {
      await appWindow.hide();
    } catch (e) {
      console.error("Failed to hide window", e);
    }
  };

  const snoozeAlert = async (targetAlert: GoalAlertPayload, minutes: number) => {
    closeActionRef.current.busy = true;
    setBusy(true);
    try {
      await invoke("snooze_goal", { goalId: targetAlert.goalId, minutes });
      await invoke("clear_pending_alert", { label: "annoy" });
      await hide();
    } finally {
      closeActionRef.current.busy = false;
      setBusy(false);
    }
  };

  const snooze = async () => {
    if (!alert || busy || !snoozeReady) return;
    await snoozeAlert(alert, snoozeMinutes);
  };

  useEffect(() => {
    let dispose: (() => void) | undefined;

    void listen("annoy-close-requested", () => {
      const current = closeActionRef.current;
      if (!current.alert || current.busy || !current.snoozeReady) return;
      void snoozeAlert(current.alert, current.snoozeMinutes);
    })
      .then((unlisten) => { dispose = unlisten; })
      .catch((error) => { console.error("Failed to subscribe to annoy close requests", error); });

    return () => { dispose?.(); };
  }, []);

  // Don't render content until an alert arrives — but keep the shell so
  // Tauri doesn't show a gray rectangle on first open
  if (!alert) {
    return <main className="annoy-shell" />;
  }

  return (
    <main className="annoy-shell">
      <div className="annoy-shell__flash" key={instanceKey} />
      <section className="annoy-screen">
        <span className="annoy-screen__kicker">Annoy</span>

        <h1>{msg.heading(alert.label)}</h1>

        <p className="annoy-screen__lead">
          You are at {formatLongDuration(alert.totalSeconds)}. The hard threshold was {formatLongDuration(alert.thresholdSeconds)}.
        </p>
        <p className="annoy-screen__lead">{msg.sub(snoozeNumber)}</p>
        <p className="annoy-screen__lead">
          This screen will come back in {snoozeMinutes} minutes if you snooze it.
        </p>

        <div className="annoy-screen__actions">
          <button
            className="btn btn--accent btn--lg"
            disabled={busy || !snoozeReady}
            onClick={() => void snooze()}
            onMouseEnter={() => { if (!drifted && snoozeCount > 0) setDrifted(true); }}
            style={drifted ? { transform: "translate(8px, -4px)", transition: "transform 0.15s ease" } : {}}
            type="button"
          >
            Snooze {snoozeMinutes} min
          </button>
        </div>
      </section>
    </main>
  );
}
