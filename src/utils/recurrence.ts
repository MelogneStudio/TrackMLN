import type { Recurrence } from "../types";
import { formatDayMonth, fromIso, WEEKDAYS_SHORT, weekdayIndex } from "./dates";

export function newRecurrence(startDate: string): Recurrence {
  return {
    kind: "weekly",
    interval: 1,
    weekdays: [weekdayIndex(startDate)],
    monthDay: null,
    startDate,
    endDate: null
  };
}

function ordinal(day: number): string {
  const rest = day % 100;
  if (rest >= 11 && rest <= 13) return `${day}th`;
  switch (day % 10) {
    case 1:
      return `${day}st`;
    case 2:
      return `${day}nd`;
    case 3:
      return `${day}rd`;
    default:
      return `${day}th`;
  }
}

/** "Every day", "Every 2 weeks on Mon, Wed", "Monthly on the 15th · until 3 Dec" */
export function describeRecurrence(recurrence: Recurrence): string {
  const every = recurrence.interval > 1 ? recurrence.interval : 0;
  let text: string;

  switch (recurrence.kind) {
    case "daily":
      text = every ? `Every ${every} days` : "Every day";
      break;
    case "weekly": {
      const days = [...recurrence.weekdays].sort((a, b) => a - b).map((day) => WEEKDAYS_SHORT[day]);
      const base = every ? `Every ${every} weeks` : "Every week";
      text = days.length > 0 ? `${base} on ${days.join(", ")}` : base;
      break;
    }
    default: {
      const day = recurrence.monthDay ?? fromIso(recurrence.startDate).getDate();
      text = every ? `Every ${every} months on the ${ordinal(day)}` : `Monthly on the ${ordinal(day)}`;
    }
  }

  return recurrence.endDate ? `${text} · until ${formatDayMonth(recurrence.endDate)}` : text;
}
