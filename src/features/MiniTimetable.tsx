import { useTimetableToday } from "../lib/hooks";
import type { TimetableOccurrence } from "../types";
import { formatMinuteOfDay } from "../utils/format";

type MiniTimetableProps = {
  enabled?: boolean;
  onOpen?: () => void;
  /** Show every remaining block of the day, not just now + next. */
  full?: boolean;
};

function Block({ block, label }: { block: TimetableOccurrence; label: string }) {
  return (
    <div className="tt-line">
      <span className="tt-line__dot" style={{ background: block.color || "var(--accent)" }} />
      <div className="grow">
        <div className="faint tt-line__cap">{label}</div>
        <div className="tt-line__title">{block.title}</div>
      </div>
      <span className="mono dim">
        {formatMinuteOfDay(block.startMinute)} – {formatMinuteOfDay(block.endMinute)}
      </span>
    </div>
  );
}

export function MiniTimetable({ enabled = true, onOpen, full }: MiniTimetableProps) {
  const { today } = useTimetableToday(enabled);

  if (!today.current && !today.next) {
    return (
      <div className="empty">
        <strong>{today.all.length > 0 ? "All done for today" : "Nothing scheduled"}</strong>
        {onOpen ? (
          <button className="btn btn--sm" type="button" onClick={onOpen}>
            Plan your day
          </button>
        ) : null}
      </div>
    );
  }

  const after = today.next?.startMinute ?? today.current?.startMinute ?? 0;
  const later = full ? today.all.filter((block) => block.startMinute > after) : [];

  return (
    <div className="stack stack--sm">
      {today.current ? <Block block={today.current} label="Now" /> : null}
      {today.next ? <Block block={today.next} label="Next up" /> : null}
      {later.map((block) => (
        <Block key={`${block.entryId}-${block.date}-${block.startMinute}`} block={block} label="Later" />
      ))}
    </div>
  );
}
