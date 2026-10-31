import { useState } from "react";
import { phaseLabel, useFocus, useFocusPresets, useFocusStats } from "../lib/hooks";
import { usePrefs } from "../lib/prefs";
import { formatClock, formatDuration } from "../utils/format";
import { Icon } from "../ui/Icon";
import { Ring } from "../ui/Ring";
import { Select } from "../ui/Select";

type FocusMiniProps = {
  onOpen?: () => void;
  enabled?: boolean;
};

/** Compact focus control used on Home and in the overlay. */
export function FocusMini({ onOpen, enabled = true }: FocusMiniProps) {
  const { prefs } = usePrefs();
  const { snapshot, remaining, progress, error, actions } = useFocus(enabled);
  const presets = useFocusPresets(enabled);
  const stats = useFocusStats(undefined, undefined, enabled);
  const [presetId, setPresetId] = useState<number | null>(null);

  const list = presets.data ?? [];
  const chosen = presetId ?? list.find((preset) => preset.isDefault)?.id ?? list[0]?.id ?? null;
  const focused = stats.data?.totalFocusSeconds ?? 0;
  const goalSeconds = prefs.focusGoalMinutes * 60;
  const percent = Math.min(100, Math.round((focused / goalSeconds) * 100));

  if (snapshot.active) {
    const waiting = snapshot.status === "waiting";
    const paused = snapshot.status === "paused";
    return (
      <div className="focus-mini">
        <div className="focus-mini__main">
          <div className="row row--wrap">
            <span className="chip chip--accent">{phaseLabel(snapshot.phase)}</span>
            {snapshot.preset ? <span className="chip">{snapshot.preset.name}</span> : null}
            {paused ? <span className="chip chip--warn">Paused</span> : null}
            {waiting ? <span className="chip chip--warn">Ready to start</span> : null}
          </div>
          <div className="focus-mini__clock display">{formatClock(remaining)}</div>
          <div className="row">
            {waiting || paused ? (
              <button className="btn btn--accent btn--sm" type="button" onClick={() => void actions.resume()}>
                <Icon name="play" size={14} /> {waiting ? "Start" : "Resume"}
              </button>
            ) : (
              <button className="btn btn--sm" type="button" onClick={() => void actions.pause()}>
                <Icon name="pause" size={14} /> Pause
              </button>
            )}
            <button className="icon-btn" type="button" title="Skip phase" onClick={() => void actions.skip()}>
              <Icon name="skip" size={16} />
            </button>
            <button className="icon-btn icon-btn--danger" type="button" title="Stop" onClick={() => void actions.stop()}>
              <Icon name="stop" size={16} />
            </button>
          </div>
        </div>
        <button className="focus-mini__ring" type="button" onClick={onOpen} disabled={!onOpen} title="Open focus">
          <Ring progress={progress} size={92} stroke={9}>
            <span className="display focus-mini__pct">
              {snapshot.currentSession}/{snapshot.totalSessions}
            </span>
            <span className="faint focus-mini__cap">sessions</span>
          </Ring>
        </button>
      </div>
    );
  }

  return (
    <div className="focus-mini">
      <div className="focus-mini__main">
        <div className="row">
          <Icon name="timer" size={30} strokeWidth={1.4} />
          <div className="grow">
            <div className="faint focus-mini__cap">Session preset</div>
            <Select
              options={list.map((preset) => ({ value: preset.id ?? 0, label: preset.name, sublabel: `${preset.focusMinutes}m` }))}
              value={chosen}
              onChange={(value) => setPresetId(value)}
              placeholder="No presets"
              disabled={list.length === 0}
            />
          </div>
        </div>
        <button
          className="btn btn--accent"
          type="button"
          disabled={chosen === null}
          onClick={() => void actions.start({ presetId: chosen })}
        >
          <Icon name="play" size={15} /> Start session
        </button>
        {error ? <span className="error-text">{error}</span> : null}
      </div>
      <button className="focus-mini__ring" type="button" onClick={onOpen} disabled={!onOpen} title="Open focus">
        <Ring progress={percent / 100} size={92} stroke={9}>
          <span className="display focus-mini__pct">{percent}%</span>
        </Ring>
        <span className="focus-mini__ringcap">
          <span className="faint focus-mini__cap">Time focused</span>
          <strong>{formatDuration(focused)}</strong>
        </span>
      </button>
    </div>
  );
}
