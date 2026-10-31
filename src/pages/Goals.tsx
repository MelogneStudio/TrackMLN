import { useMemo, useState } from "react";
import { usePoll, useTodayUsage } from "../lib/hooks";
import type { Goal, GoalCandidate, GoalDraft } from "../types";
import { Segmented } from "../ui/Controls";
import { Icon } from "../ui/Icon";
import { Modal } from "../ui/Modal";
import { Select } from "../ui/Select";
import { formatDuration, formatLongDuration } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";

const TOTAL_TARGET = "__total__";

type Editor = { id: number | null; kind: "app" | "total"; target: string; warn: string; annoy: string };

const EMPTY: Editor = { id: null, kind: "app", target: "", warn: "", annoy: "" };

const minutesToText = (seconds: number | null) => (seconds ? String(Math.round(seconds / 60)) : "");

function parseMinutes(value: string): number | null {
  if (!value.trim()) return null;
  const minutes = Number(value);
  return Number.isFinite(minutes) && minutes > 0 ? Math.round(minutes * 60) : null;
}

export function Goals() {
  const goals = usePoll<Goal[]>(() => call<Goal[]>("get_goals"), 0);
  const candidates = usePoll<GoalCandidate[]>(() => call<GoalCandidate[]>("get_goal_candidates"), 0);
  const usage = useTodayUsage();

  const [editor, setEditor] = useState<Editor | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const list = goals.data ?? [];

  const usedFor = (goal: Goal): number => {
    if (goal.targetKind === "total") return usage.data?.total ?? 0;
    return usage.data?.apps.find((app) => app.app_identity === goal.targetValue)?.total ?? 0;
  };

  const appOptions = useMemo(
    () =>
      (candidates.data ?? [])
        .filter((candidate) => !list.some((goal) => goal.targetValue === candidate.appIdentity && goal.id !== editor?.id))
        .map((candidate) => ({
          value: candidate.appIdentity,
          label: candidate.appName,
          sublabel: formatLongDuration(candidate.totalSeconds)
        })),
    [candidates.data, list, editor?.id]
  );

  const openEditor = (goal?: Goal) => {
    setError(null);
    setEditor(
      goal
        ? {
            id: goal.id,
            kind: goal.targetKind,
            target: goal.targetValue,
            warn: minutesToText(goal.warnSeconds),
            annoy: minutesToText(goal.annoySeconds)
          }
        : EMPTY
    );
  };

  const save = async () => {
    if (!editor) return;
    const warnSeconds = parseMinutes(editor.warn);
    const annoySeconds = parseMinutes(editor.annoy);

    if (editor.kind === "app" && !editor.target) return setError("Pick an app first.");
    if (!warnSeconds && !annoySeconds) return setError("Set at least one threshold.");
    if (warnSeconds && annoySeconds && annoySeconds < warnSeconds) {
      return setError("The annoy limit should be later than the warning.");
    }

    const draft: GoalDraft = {
      id: editor.id,
      targetKind: editor.kind,
      targetValue: editor.kind === "total" ? TOTAL_TARGET : editor.target,
      warnSeconds,
      annoySeconds
    };

    setBusy(true);
    setError(null);
    try {
      await call<Goal[]>("save_goal", { draft });
      await goals.refresh();
      setEditor(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (goal: Goal) => {
    try {
      await call<Goal[]>("delete_goal", { goalId: goal.id });
      await goals.refresh();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">Limits</h1>
          <p className="page-sub">Get a nudge, then a hard stop, when you spend too long on something.</p>
        </div>
        <div className="page__actions">
          <button className="btn btn--accent" type="button" onClick={() => openEditor()}>
            <Icon name="plus" size={16} /> New limit
          </button>
        </div>
      </header>

      {error && !editor ? <p className="error-text" style={{ marginBottom: 10 }}>{error}</p> : null}

      <div className="list">
        {!goals.loading && list.length === 0 ? (
          <div className="empty">
            <strong>No limits yet</strong>
            Add one to get a warning popup, and an annoying full-screen reminder if you keep going.
          </div>
        ) : null}

        {list.map((goal) => {
          const used = usedFor(goal);
          const ceiling = goal.annoySeconds ?? goal.warnSeconds ?? 1;
          const overWarn = goal.warnSeconds !== null && used >= goal.warnSeconds;
          const overAnnoy = goal.annoySeconds !== null && used >= goal.annoySeconds;

          return (
            <article className="item goal" key={goal.id}>
              <div className="item__body">
                <div className="row row--wrap" style={{ justifyContent: "space-between" }}>
                  <div className="item__title">
                    {goal.label}{" "}
                    <span className="chip" style={{ marginLeft: 6 }}>
                      {goal.targetKind === "total" ? "All apps" : "App"}
                    </span>
                  </div>
                  <span className={`mono ${overAnnoy ? "goal__over" : overWarn ? "goal__warn" : "dim"}`}>
                    {formatDuration(used)} today
                  </span>
                </div>

                <div className="goal__meter">
                  <div className="meter">
                    <span
                      style={{
                        width: `${Math.min(100, (used / ceiling) * 100)}%`,
                        background: overAnnoy ? "var(--danger)" : overWarn ? "var(--warning)" : undefined
                      }}
                    />
                  </div>
                  {goal.warnSeconds !== null && goal.annoySeconds !== null ? (
                    <i className="goal__tick" style={{ left: `${(goal.warnSeconds / ceiling) * 100}%` }} />
                  ) : null}
                </div>

                <div className="item__meta">
                  {goal.warnSeconds ? (
                    <span className="chip chip--warn">Warn at {formatLongDuration(goal.warnSeconds)}</span>
                  ) : null}
                  {goal.annoySeconds ? (
                    <span className="chip chip--danger">Annoy at {formatLongDuration(goal.annoySeconds)}</span>
                  ) : null}
                </div>
              </div>
              <div className="item__actions">
                <button className="icon-btn" type="button" title="Edit" onClick={() => openEditor(goal)}>
                  <Icon name="edit" size={16} />
                </button>
                <button className="icon-btn icon-btn--danger" type="button" title="Delete" onClick={() => void remove(goal)}>
                  <Icon name="trash" size={16} />
                </button>
              </div>
            </article>
          );
        })}
      </div>

      {editor ? (
        <Modal
          title={editor.id ? "Edit limit" : "New limit"}
          onClose={() => setEditor(null)}
          footer={
            <>
              <button className="btn" type="button" onClick={() => setEditor(null)}>
                Cancel
              </button>
              <button className="btn btn--accent" type="button" disabled={busy} onClick={() => void save()}>
                {busy ? "Saving…" : "Save"}
              </button>
            </>
          }
        >
          <div className="stack">
            <Segmented
              value={editor.kind}
              onChange={(kind) => setEditor({ ...editor, kind, target: kind === "total" ? TOTAL_TARGET : "" })}
              options={[
                { value: "app", label: "Per app" },
                { value: "total", label: "Total screen time" }
              ]}
            />

            {editor.kind === "app" ? (
              <div className="field">
                <span>App</span>
                <Select
                  options={appOptions}
                  value={editor.target || null}
                  onChange={(target) => setEditor({ ...editor, target })}
                  placeholder="Choose a tracked app"
                />
              </div>
            ) : null}

            <div className="grid-2">
              <label className="field">
                <span>Warn after (minutes)</span>
                <input
                  className="input"
                  type="number"
                  min={1}
                  placeholder="90"
                  value={editor.warn}
                  onChange={(event) => setEditor({ ...editor, warn: event.target.value })}
                />
              </label>
              <label className="field">
                <span>Annoy after (minutes)</span>
                <input
                  className="input"
                  type="number"
                  min={1}
                  placeholder="120"
                  value={editor.annoy}
                  onChange={(event) => setEditor({ ...editor, annoy: event.target.value })}
                />
              </label>
            </div>
            <p className="faint" style={{ fontSize: 12 }}>
              A warning is a small popup. Annoy is a full-screen reminder that shrinks its snooze time every time you dismiss it. Either
              threshold is optional.
            </p>

            {error ? <span className="error-text">{error}</span> : null}
          </div>
        </Modal>
      ) : null}
    </div>
  );
}
