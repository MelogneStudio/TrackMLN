import { useMemo, useState } from "react";
import { useTodos } from "../lib/hooks";
import type { Todo } from "../types";
import { formatMinuteOfDay } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";
import { Check } from "../ui/Check";
import { Ring } from "../ui/Ring";

type MiniTasksProps = {
  limit?: number;
  enabled?: boolean;
  onOpen?: () => void;
};

export function MiniTasks({ limit = 4, enabled = true, onOpen }: MiniTasksProps) {
  const today = useTodos("today", enabled);
  const overdue = useTodos("overdue", enabled);
  const open = useTodos("open", enabled);
  const [error, setError] = useState<string | null>(null);

  const { list, done, total } = useMemo(() => {
    const dueToday = today.data ?? [];
    const late = overdue.data ?? [];
    const seen = new Set<number>();
    const merged: Todo[] = [];
    for (const todo of [...late, ...dueToday]) {
      if (!seen.has(todo.id)) {
        seen.add(todo.id);
        merged.push(todo);
      }
    }
    const pool = merged.length > 0 ? merged : (open.data ?? []);
    const ordered = [...pool].sort((a, b) => Number(a.completed) - Number(b.completed));
    return {
      list: ordered.slice(0, limit),
      done: dueToday.filter((todo) => todo.completed).length,
      total: dueToday.length
    };
  }, [today.data, overdue.data, open.data, limit]);

  const refreshAll = () => {
    void today.refresh();
    void overdue.refresh();
    void open.refresh();
  };

  const toggle = async (todo: Todo, completed: boolean) => {
    try {
      await call("set_todo_completed", { todoId: todo.id, completed });
      setError(null);
      refreshAll();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  return (
    <div className="mini-tasks">
      <div className="mini-tasks__head">
        <button className="mini-tasks__ring" type="button" onClick={onOpen} disabled={!onOpen}>
          <Ring progress={total > 0 ? done / total : 0} size={64} stroke={7}>
            <span className="display mini-tasks__pct">{total > 0 ? Math.round((done / total) * 100) : 0}%</span>
          </Ring>
        </button>
        <div className="faint mini-tasks__cap">
          {total > 0 ? `${done} of ${total} due today done` : "Nothing due today"}
        </div>
      </div>

      <div className="mini-tasks__list">
        {list.length === 0 ? <div className="empty">No tasks yet</div> : null}
        {list.map((todo) => (
          <div className={`mini-task${todo.completed ? " is-done" : ""}`} key={todo.id}>
            <Check checked={todo.completed} label={`Complete ${todo.title}`} onChange={(next) => void toggle(todo, next)} />
            <span className="mini-task__title">{todo.title}</span>
            {todo.overdue && !todo.completed ? (
              <span className="chip chip--danger">Overdue</span>
            ) : todo.dueMinute !== null && !todo.completed ? (
              <span className="mini-task__time faint">{formatMinuteOfDay(todo.dueMinute)}</span>
            ) : null}
          </div>
        ))}
      </div>
      {error ? <div className="error-text">{error}</div> : null}
    </div>
  );
}
