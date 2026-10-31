import { AppsList } from "../features/AppsList";
import { FocusMini } from "../features/FocusMini";
import { MiniTasks } from "../features/MiniTasks";
import { MiniTimetable } from "../features/MiniTimetable";
import { useTodayUsage, useUsageRange } from "../lib/hooks";
import { usePrefs } from "../lib/prefs";
import type { PageId } from "../shell/Sidebar";
import { BarChart } from "../ui/BarChart";
import { addDays, formatDateShort, todayIso } from "../utils/dates";
import { formatDuration, formatHeroDuration, formatHour, greetingFor, HOUR_LABELS, percentChange } from "../utils/format";

type HomeProps = {
  onNavigate: (page: PageId) => void;
};

export function Home({ onNavigate }: HomeProps) {
  const { prefs } = usePrefs();
  const usage = useTodayUsage();
  const lastWeekDay = addDays(todayIso(), -7);
  const lastWeek = useUsageRange(lastWeekDay, lastWeekDay);

  const total = usage.data?.total ?? 0;
  const apps = usage.data?.apps ?? [];
  const hourly = usage.data?.hourly ?? Array.from({ length: 24 }, (_, hour) => ({ hour, total: 0 }));
  const hero = formatHeroDuration(total);
  const change = lastWeek.data ? percentChange(total, lastWeek.data.totalSeconds) : null;
  const currentHour = new Date().getHours();

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">
            {greetingFor()}
            {prefs.nickname ? `, ${prefs.nickname}` : ""} <span aria-hidden="true">👋</span>
          </h1>
          <p className="page-sub">{formatDateShort(todayIso())}</p>
        </div>
      </header>

      <div className="home">
        <div className="home__col home__col--main">
          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Today&rsquo;s Screen Time</h2>
              <button className="btn btn--ghost btn--sm" type="button" onClick={() => onNavigate("screentime")}>
                Details
              </button>
            </div>
            <div className="stat-big">
              {hero.hours > 0 ? (
                <>
                  {hero.hours}hr {hero.minutes}m
                </>
              ) : (
                <>{hero.minutes}m</>
              )}
              {change !== null ? <sup>{change > 0 ? `+${change}` : change}%</sup> : null}
            </div>
            <p className="faint home__vs">
              {change === null
                ? "No data from the same day last week yet"
                : `${Math.abs(change)}% ${change <= 0 ? "less" : "more"} than last ${new Date().toLocaleDateString(undefined, { weekday: "long" })}`}
            </p>
            <div className="home__chart">
              <BarChart
                values={hourly.map((entry) => entry.total)}
                labels={HOUR_LABELS}
                labelEvery={2}
                height={150}
                highlight={currentHour}
                formatTip={(value, index) => `${formatHour(index)} · ${formatDuration(value)}`}
              />
            </div>
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Focus</h2>
            </div>
            <FocusMini onOpen={() => onNavigate("focus")} />
          </section>
        </div>

        <div className="home__col">
          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Most used apps</h2>
            </div>
            <AppsList apps={apps} limit={5} />
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Timetable</h2>
              <button className="btn btn--ghost btn--sm" type="button" onClick={() => onNavigate("timetable")}>
                Open
              </button>
            </div>
            <MiniTimetable onOpen={() => onNavigate("timetable")} />
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Tasks</h2>
              <button className="btn btn--ghost btn--sm" type="button" onClick={() => onNavigate("tasks")}>
                Open
              </button>
            </div>
            <MiniTasks onOpen={() => onNavigate("tasks")} />
          </section>
        </div>
      </div>
    </div>
  );
}
