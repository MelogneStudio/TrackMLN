import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { usePrefs } from "../lib/prefs";
import { THEMES } from "../lib/theme";
import type { AppSettings } from "../types";
import { call, errorMessage } from "../utils/ipc";
import { buildShortcut, shortcutParts } from "../utils/shortcut";
import { Toggle } from "../ui/Controls";
import { Icon } from "../ui/Icon";
import { Modal } from "../ui/Modal";
import { Select } from "../ui/Select";

const IDLE_OPTIONS = [
  { value: 0, label: "Never" },
  { value: 60, label: "1 minute" },
  { value: 120, label: "2 minutes" },
  { value: 300, label: "5 minutes" },
  { value: 600, label: "10 minutes" },
  { value: 900, label: "15 minutes" },
  { value: 1800, label: "30 minutes" },
  { value: 3600, label: "1 hour" }
];

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="card settings__section">
      <div className="settings__head">
        <h2 className="card__title">{title}</h2>
        {hint ? <p className="faint settings__hint">{hint}</p> : null}
      </div>
      <div className="stack">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="settings__row">
      <div className="settings__label">
        <div>{label}</div>
        {hint ? <div className="faint settings__rowhint">{hint}</div> : null}
      </div>
      <div className="settings__control">{children}</div>
    </div>
  );
}

export function Settings() {
  const { prefs, settings, setSettings, update } = usePrefs();
  const [message, setMessage] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [labelsOpen, setLabelsOpen] = useState(false);
  const [storage, setStorage] = useState("");

  useEffect(() => {
    call<string>("get_storage_location").then(setStorage).catch((error) => setStorage(errorMessage(error)));
  }, []);

  const apply = useCallback(
    async (command: string, args?: Record<string, unknown>, success?: string) => {
      try {
        const next = await call<AppSettings>(command, args);
        setSettings(next);
        setMessage(success ?? null);
        return next;
      } catch (error) {
        setMessage(errorMessage(error));
        return null;
      }
    },
    [setSettings]
  );

  useEffect(() => {
    if (!recording) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) return;
      event.preventDefault();
      event.stopPropagation();

      if (event.key === "Escape") {
        setRecording(false);
        return;
      }

      const shortcut = buildShortcut(event);
      if (!shortcut) return;

      setRecording(false);
      void apply("set_hotkey", { hotkey: shortcut }, "Shortcut updated.");
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, apply]);

  const hotkey = settings?.hotkey ?? "control+shift+Space";

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-sub">Make TrackMLN look and behave the way you like.</p>
        </div>
        {message ? <span className="chip chip--accent">{message}</span> : null}
      </header>

      <div className="settings">
        <Section title="Profile">
          <Row label="Nickname" hint="Shown in your greeting.">
            <input
              className="input settings__input"
              value={prefs.nickname}
              maxLength={24}
              placeholder="Enter your nickname"
              onChange={(event) => update({ nickname: event.target.value })}
            />
          </Row>
        </Section>

        <Section title="Appearance" hint="Changes apply instantly.">
          <div className="swatches swatches--left" role="radiogroup" aria-label="Theme">
            {THEMES.map((theme) => (
              <button
                key={theme.id}
                type="button"
                role="radio"
                aria-checked={prefs.theme === theme.id}
                aria-label={theme.label}
                title={theme.label}
                className={`swatch${prefs.theme === theme.id ? " is-active" : ""}`}
                style={{ background: theme.swatch }}
                onClick={() => update({ theme: theme.id })}
              />
            ))}
          </div>
          <Row label="Sidebar translucency" hint="How much of your wallpaper shows through the sidebar.">
            <div className="settings__slider">
              <span className="faint">Solid</span>
              <input
                className="slider"
                type="range"
                min={0}
                max={100}
                value={prefs.sidebarTranslucency}
                onChange={(event) => update({ sidebarTranslucency: Number(event.target.value) })}
                aria-label="Sidebar translucency"
              />
              <span className="faint">Glassy</span>
            </div>
          </Row>
        </Section>

        <Section title="Quick overlay" hint="A compact glass panel you can summon from anywhere.">
          <Row label="Shortcut" hint="Toggles the overlay. Press Esc while recording to cancel.">
            <div className="row">
              <button
                className={`btn${recording ? " btn--accent" : ""}`}
                type="button"
                onClick={() => {
                  setMessage(null);
                  setRecording(true);
                }}
              >
                {recording ? (
                  "Press keys…"
                ) : (
                  <span className="row" style={{ gap: 5 }}>
                    {shortcutParts(hotkey).map((part) => (
                      <span className="kbd" key={part}>
                        {part}
                      </span>
                    ))}
                  </span>
                )}
              </button>
              <button
                className="btn btn--ghost"
                type="button"
                onClick={() => void apply("reset_hotkey", undefined, "Shortcut reset to default.")}
              >
                Reset
              </button>
              <button className="btn" type="button" onClick={() => void call("show_overlay")}>
                Try it
              </button>
            </div>
          </Row>
          <Row label="Pop up when a focus phase ends" hint="Shows the overlay so a finished session never goes unnoticed.">
            <Toggle on={prefs.focusPopup} onChange={(focusPopup) => update({ focusPopup })} label="Pop up when a focus phase ends" />
          </Row>
        </Section>

        <Section title="Tracking">
          <Row label="Pause when idle" hint="Stop counting screen time after no keyboard or mouse input for this long.">
            <div className="settings__select">
              <Select
                options={IDLE_OPTIONS}
                value={settings?.idleTimeoutSeconds ?? 300}
                onChange={(seconds) => void apply("set_idle_timeout", { seconds })}
              />
            </div>
          </Row>
          <Row label="Daily focus goal" hint="Fills the “time focused” ring on Home.">
            <div className="row">
              <input
                className="input settings__number"
                type="number"
                min={5}
                max={1440}
                step={5}
                value={prefs.focusGoalMinutes}
                onChange={(event) => {
                  const value = Number(event.target.value);
                  if (Number.isFinite(value) && value >= 5 && value <= 1440) {
                    update({ focusGoalMinutes: Math.round(value) });
                  }
                }}
              />
              <span className="dim">minutes</span>
            </div>
          </Row>
          <Row label="App names" hint="Give executables friendlier names without splitting history.">
            <button className="btn" type="button" onClick={() => setLabelsOpen(true)}>
              Manage names
            </button>
          </Row>
        </Section>

        <Section title="About">
          <Row label="Data location" hint="Your data stays on this machine.">
            <span className="settings__path mono" title={storage}>
              {storage || "…"}
            </span>
          </Row>
          <Row label="Setup" hint="Walk through the welcome flow again.">
            <button className="btn" type="button" onClick={() => update({ onboardingDone: false })}>
              Replay setup
            </button>
          </Row>
          <Row label="TrackMLN v3.0.0" hint="A Melogne Studio app.">
            <button className="btn btn--danger" type="button" onClick={() => void call("quit_app")}>
              <Icon name="power" size={16} /> Quit TrackMLN v3.0.0
            </button>
          </Row>
        </Section>
      </div>

      {labelsOpen && settings ? (
        <LabelsModal
          labels={settings.exeLabels}
          onClose={() => setLabelsOpen(false)}
          onSave={(next) => apply("set_exe_labels", { exeLabels: next })}
        />
      ) : null}
    </div>
  );
}

/* ── Exe labels ─────────────────────────────────────────────────────────── */

type LabelsModalProps = {
  labels: Record<string, string>;
  onClose: () => void;
  onSave: (labels: Record<string, string>) => Promise<AppSettings | null>;
};

type Editing = { key: string | null; exe: string; name: string } | null;

function LabelsModal({ labels, onClose, onSave }: LabelsModalProps) {
  const [editing, setEditing] = useState<Editing>(null);
  const [error, setError] = useState<string | null>(null);
  const [undo, setUndo] = useState<{ key: string; value: string } | null>(null);
  const undoTimer = useRef<number | null>(null);
  const exeRef = useRef<HTMLInputElement>(null);

  const sorted = Object.entries(labels).sort(([a], [b]) => a.localeCompare(b));

  useEffect(() => {
    if (editing) window.setTimeout(() => exeRef.current?.focus(), 30);
  }, [editing]);

  useEffect(
    () => () => {
      if (undoTimer.current !== null) window.clearTimeout(undoTimer.current);
    },
    []
  );

  const remove = async (key: string) => {
    const value = labels[key];
    const next = { ...labels };
    delete next[key];
    await onSave(next);
    setUndo({ key, value });
    if (undoTimer.current !== null) window.clearTimeout(undoTimer.current);
    undoTimer.current = window.setTimeout(() => setUndo(null), 6000);
  };

  const restore = async () => {
    if (!undo) return;
    const next = { ...labels, [undo.key]: undo.value };
    setUndo(null);
    await onSave(next);
  };

  const submit = async () => {
    if (!editing) return;
    const key = editing.exe.trim().toLowerCase();
    const name = editing.name.trim();
    if (!key) return setError("Enter the .exe filename.");
    if (!name) return setError("Enter the display name.");
    if (Object.keys(labels).some((existing) => existing === key && existing !== editing.key)) {
      return setError("That .exe name already has a label.");
    }

    const next = { ...labels };
    if (editing.key && editing.key !== key) delete next[editing.key];
    next[key] = name;
    await onSave(next);
    setEditing(null);
    setError(null);
  };

  if (editing) {
    return (
      <Modal
        title={editing.key ? "Edit app name" : "Add app name"}
        onClose={() => setEditing(null)}
        footer={
          <>
            <button className="btn" type="button" onClick={() => setEditing(null)}>
              Cancel
            </button>
            <button className="btn btn--accent" type="button" onClick={() => void submit()}>
              Save
            </button>
          </>
        }
      >
        <div className="stack">
          <label className="field">
            <span>Display name</span>
            <input
              className="input"
              value={editing.name}
              placeholder="Minecraft"
              onChange={(event) => setEditing({ ...editing, name: event.target.value })}
            />
          </label>
          <label className="field">
            <span>Executable name</span>
            <input
              ref={exeRef}
              className="input"
              value={editing.exe}
              placeholder="javaw.exe"
              onChange={(event) => setEditing({ ...editing, exe: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === "Enter") void submit();
              }}
            />
          </label>
          {error ? <span className="error-text">{error}</span> : null}
        </div>
      </Modal>
    );
  }

  return (
    <Modal
      title="App names"
      onClose={onClose}
      wide
      footer={
        <>
          {undo ? (
            <button className="btn btn--ghost" type="button" onClick={() => void restore()}>
              Deleted “{undo.value}” — Undo
            </button>
          ) : null}
          <button
            className="btn btn--accent"
            type="button"
            onClick={() => {
              setError(null);
              setEditing({ key: null, exe: "", name: "" });
            }}
          >
            <Icon name="plus" size={16} /> Add
          </button>
        </>
      }
    >
      <div className="labels">
        {sorted.length === 0 ? (
          <div className="empty">
            <strong>No custom names yet</strong>
            Add one to rename an app everywhere.
          </div>
        ) : null}
        {sorted.map(([key, value]) => (
          <div className="labels__row" key={key}>
            <div className="grow">
              <div>{value}</div>
              <div className="faint mono">{key}</div>
            </div>
            <button
              className="icon-btn"
              type="button"
              aria-label={`Edit ${key}`}
              onClick={() => {
                setError(null);
                setEditing({ key, exe: key, name: value });
              }}
            >
              <Icon name="edit" size={16} />
            </button>
            <button
              className="icon-btn icon-btn--danger"
              type="button"
              aria-label={`Delete ${key}`}
              onClick={() => void remove(key)}
            >
              <Icon name="trash" size={16} />
            </button>
          </div>
        ))}
      </div>
    </Modal>
  );
}
