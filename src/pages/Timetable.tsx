import { MouseEvent, useEffect, useMemo, useRef, useState } from "react";
import { EntryModal, OccurrenceModal } from "../features/TimetableModals";
import { useClock, useTimetable } from "../lib/hooks";
import type { PageId } from "../shell/Sidebar";
import type { TimetableEntry, TimetableOccurrence } from "../types";
import { Segmented } from "../ui/Controls";
import { Icon } from "../ui/Icon";
import { addDays, formatDayMonth, fromIso, isToday, mondayOf, nowMinute, todayIso, WEEKDAYS_SHORT } from "../utils/dates";
import { formatMinuteOfDay } from "../utils/format";
import { call } from "../utils/ipc";

type View = "week" | "day";

const HOUR_PX = 56;
const SNAP = 15;

type Placed = { block: TimetableOccurrence; lane: number; lanes: number };

/** Greedy lane assignment so overlapping blocks sit side by side. */
function layoutDay(blocks: TimetableOccurrence[]): Placed[] {
  const sorted = [...blocks].sort((a, b) => a.startMinute - b.startMinute || b.endMinute - a.endMinute);
  const result: Placed[] = [];
  let cluster: { block: TimetableOccurrence; lane: number }[] = [];
  let laneEnds: number[] = [];
  let clusterEnd = -1;

  const flush = () => {
    const lanes = Math.max(1, laneEnds.length);
    cluster.forEach((entry) => result.push({ ...entry, lanes }));
    cluster = [];
    laneEnds = [];
    clusterEnd = -1;
  };

  for (const block of sorted) {
    if (cluster.length > 0 && block.startMinute >= clusterEnd) {
      flush();
    }
    let lane = laneEnds.findIndex((end) => end <= block.startMinute);
    if (lane === -1) {
      lane = laneEnds.length;
      laneEnds.push(block.endMinute);
    } else {
      laneEnds[lane] = block.endMinute;
    }
    cluster.push({ block, lane });
    clusterEnd = Math.max(clusterEnd, block.endMinute);
  }
  flush();
  return result;
}

type TimetableProps = {
  onNavigate: (page: PageId) => void;
};

export function Timetable({ onNavigate }: TimetableProps) {
  const [view, setView] = useState<View>("week");
  const [anchor, setAnchor] = useState(todayIso());
  const [creating, setCreating] = useState<{ date: string; start: number } | null>(null);
  const [editingEntry, setEditingEntry] = useState<TimetableEntry | null>(null);
  const [selected, setSelected] = useState<TimetableOccurrence | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const scrolled = useRef(false);

  const weekStart = mondayOf(anchor);
  const days = useMemo(
    () => (view === "week" ? Array.from({ length: 7 }, (_, i) => addDays(weekStart, i)) : [anchor]),
    [view, weekStart, anchor]
  );
  const rangeStart = days[0];
  const rangeEnd = days[days.length - 1];

  const timetable = useTimetable(rangeStart, rangeEnd);
  const now = useClock(30_000);
  const minuteNow = now.getHours() * 60 + now.getMinutes();
  const blocks = timetable.data ?? [];

  const gridStart = Math.min(6 * 60, ...blocks.map((block) => Math.floor(block.startMinute / 60) * 60));
  const gridEnd = Math.max(22 * 60, ...blocks.map((block) => Math.ceil(block.endMinute / 60) * 60));
  const hours = Array.from({ length: (gridEnd - gridStart) / 60 }, (_, i) => gridStart / 60 + i);
  const gridHeight = ((gridEnd - gridStart) / 60) * HOUR_PX;

  useEffect(() => {
    if (!scrolled.current && scrollRef.current && timetable.data) {
      scrolled.current = true;
      const target = ((Math.max(gridStart, nowMinute() - 90) - gridStart) / 60) * HOUR_PX;
      scrollRef.current.scrollTop = target;
    }
  }, [timetable.data, gridStart]);

  const shift = (direction: -1 | 1) => setAnchor(addDays(anchor, direction * (view === "week" ? 7 : 1)));

  const openColumn = (date: string, event: MouseEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const raw = gridStart + ((event.clientY - rect.top) / HOUR_PX) * 60;
    const start = Math.max(0, Math.min(23 * 60, Math.floor(raw / SNAP) * SNAP));
    setCreating({ date, start });
  };

  const editEntry = async (entryId: number) => {
    const entries = await call<TimetableEntry[]>("get_timetable_entries");
    const entry = entries.find((candidate) => candidate.id === entryId);
    if (entry) setEditingEntry(entry);
  };

  const title =
    view === "week"
      ? `${formatDayMonth(rangeStart)} – ${formatDayMonth(rangeEnd)}`
      : fromIso(anchor).toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" });

  return (
    <div className="page page--grid">
      <header className="page__head">
        <div>
          <h1 className="page-title">Timetable</h1>
          <p className="page-sub">{title}</p>
        </div>
        <div className="page__actions">
          <Segmented
            value={view}
            onChange={setView}
            options={[
              { value: "week", label: "Week" },
              { value: "day", label: "Day" }
            ]}
          />
          <div className="row" style={{ gap: 2 }}>
            <button className="icon-btn" type="button" aria-label="Previous" onClick={() => shift(-1)}>
              <Icon name="chevronLeft" />
            </button>
            <button className="btn btn--sm" type="button" onClick={() => setAnchor(todayIso())}>
              Today
            </button>
            <button className="icon-btn" type="button" aria-label="Next" onClick={() => shift(1)}>
              <Icon name="chevronRight" />
            </button>
          </div>
          <button
            className="btn btn--accent"
            type="button"
            onClick={() => setCreating({ date: days.includes(todayIso()) ? todayIso() : anchor, start: Math.ceil((nowMinute() + 5) / SNAP) * SNAP })}
          >
            <Icon name="plus" size={16} /> New block
          </button>
        </div>
      </header>

      <div className="tt" ref={scrollRef}>
        <div className="tt__head" style={{ gridTemplateColumns: `52px repeat(${days.length}, minmax(0, 1fr))` }}>
          <div />
          {days.map((date) => {
            const index = (fromIso(date).getDay() + 6) % 7;
            return (
              <button
                key={date}
                type="button"
                className={`tt__day${isToday(date) ? " is-today" : ""}`}
                onClick={() => {
                  setAnchor(date);
                  setView("day");
                }}
              >
                <span className="faint">{WEEKDAYS_SHORT[index]}</span>
                <strong className="display">{fromIso(date).getDate()}</strong>
              </button>
            );
          })}
        </div>

        <div className="tt__body" style={{ gridTemplateColumns: `52px repeat(${days.length}, minmax(0, 1fr))`, height: gridHeight }}>
          <div className="tt__axis">
            {hours.map((hour) => (
              <div key={hour} className="tt__hour mono faint" style={{ height: HOUR_PX }}>
                {formatMinuteOfDay(hour * 60).replace(":00", "")}
              </div>
            ))}
          </div>

          {days.map((date) => {
            const placed = layoutDay(blocks.filter((block) => block.date === date));
            return (
              <div
                key={date}
                className={`tt__col${isToday(date) ? " is-today" : ""}`}
                style={{ backgroundSize: `100% ${HOUR_PX}px` }}
                onClick={(event) => openColumn(date, event)}
              >
                {placed.map(({ block, lane, lanes }) => {
                  const top = ((block.startMinute - gridStart) / 60) * HOUR_PX;
                  const height = Math.max(22, (block.durationMinutes / 60) * HOUR_PX - 2);
                  const color = block.color || "var(--accent)";
                  return (
                    <button
                      key={`${block.entryId}-${block.startMinute}`}
                      type="button"
                      className={`tt__block${block.todoCompleted ? " is-done" : ""}`}
                      style={{
                        top,
                        height,
                        left: `calc(${(lane / lanes) * 100}% + 2px)`,
                        width: `calc(${100 / lanes}% - 4px)`,
                        borderLeftColor: color,
                        background: `color-mix(in srgb, ${color} 20%, var(--bg))`
                      }}
                      onClick={(event) => {
                        event.stopPropagation();
                        setSelected(block);
                      }}
                    >
                      <span className="tt__btitle">
                        {block.todoCompleted ? <Icon name="check" size={12} strokeWidth={2.4} /> : null}
                        {block.title || "Untitled"}
                      </span>
                      {height > 36 ? (
                        <span className="tt__btime mono">
                          {formatMinuteOfDay(block.startMinute)} – {formatMinuteOfDay(block.endMinute)}
                        </span>
                      ) : null}
                      {height > 52 && (block.recurring || block.autoStart) ? (
                        <span className="tt__bicons">
                          {block.recurring ? <Icon name="repeat" size={11} /> : null}
                          {block.autoStart ? <Icon name="timer" size={11} /> : null}
                        </span>
                      ) : null}
                    </button>
                  );
                })}

                {isToday(date) && minuteNow >= gridStart && minuteNow <= gridEnd ? (
                  <div className="tt__now" style={{ top: ((minuteNow - gridStart) / 60) * HOUR_PX }} />
                ) : null}
              </div>
            );
          })}
        </div>
      </div>

      {creating ? (
        <EntryModal
          defaultDate={creating.date}
          defaultStart={creating.start}
          onClose={() => setCreating(null)}
          onSaved={() => void timetable.refresh()}
        />
      ) : null}

      {editingEntry ? (
        <EntryModal
          entry={editingEntry}
          defaultDate={editingEntry.date ?? anchor}
          defaultStart={editingEntry.startMinute}
          onClose={() => setEditingEntry(null)}
          onSaved={() => void timetable.refresh()}
        />
      ) : null}

      {selected ? (
        <OccurrenceModal
          occurrence={selected}
          onClose={() => setSelected(null)}
          onChanged={() => void timetable.refresh()}
          onEditEntry={(entryId) => void editEntry(entryId)}
          onStartFocus={() => onNavigate("focus")}
        />
      ) : null}
    </div>
  );
}
