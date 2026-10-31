import { useState } from "react";
import { ScheduleModal } from "../features/ScheduleModal";
import { TaskModal } from "../features/TaskModal";
import { usePoll, useTodos, useTodoSummary } from "../lib/hooks";
import type { PageId } from "../shell/Sidebar";
import type { Todo, TodoStatus, TodoTemplate } from "../types";
import { Check } from "../ui/Check";
import { Segmented, Toggle } from "../ui/Controls";
import { Icon } from "../ui/Icon";
import { describeDueDate } from "../utils/dates";
import { formatMinuteOfDay, formatMinutes } from "../utils/format";
import { call, errorMessage } from "../utils/ipc";
import { describeRecurrence } from "../utils/recurrence";

type View = "tasks" | "recurring";

const FILTERS: { value: TodoStatus; label: string }[] = [
  { value: "open", label: "Open" },
  { value: "today", label: "Today" },
  { value: "overdue", label: "Overdue" },
  { value: "completed", label: "Done" },
  { value: "all", label: "All" }
];

const EMPTY_TEXT: Record<TodoStatus, string> = {
  open: "Nothing left to do. Enjoy it.",
  today: "Nothing due today.",
  overdue: "Nothing overdue.",
  completed: "Finished tasks will show up here.",
  all: "No tasks yet. Add your first one."
};

function PriorityChip({ priority }: { priority: number }) {
  if (priority >= 3) return <span className="chip chip--danger">High</span>;
  if (priority === 2) return <span className="chip chip--warn">Medium</span>;
  if (priority === 1) return <span className="chip">Low</span>;
  return null;
}

type TasksProps = {
  onNavigate: (page: PageId) => void;
};

export function Tasks({ onNavigate }: TasksProps) {
  const [view, setView] = useState<View>("tasks");
  const [filter, setFilter] = useState<TodoStatus>("open");
  const [error, setError] = useState<string | null>(null);

  const todos = useTodos(filter);
  const summary = useTodoSummary();
  const templates = usePoll<TodoTemplate[]>(() => call<TodoTemplate[]>("get_todo_templates"), 4000, [], view === "recurring");

  const [editing, setEditing] = useState<{ todo?: Todo; template?: TodoTemplate } | "new" | null>(null);
  const [scheduling, setScheduling] = useState<Todo | null>(null);

  const refresh = () => {
    void todos.refresh();
    void summary.refresh();
    void templates.refresh();
  };

  const guard = async (action: () => Promise<unknown>) => {
    try {
      await action();
      setError(null);
      refresh();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const toggle = (todo: Todo, completed: boolean) =>
    guard(() => call("set_todo_completed", { todoId: todo.id, completed }));

  const remove = (todo: Todo) => guard(() => call("delete_todo_item", { todoId: todo.id }));

  const focusOn = async (todo: Todo) => {
    await guard(() => call("start_focus", { presetId: null, preset: null, todoId: todo.id, entryId: null }));
    onNavigate("focus");
  };

  const list = todos.data ?? [];
  const tiles = summary.data;

  return (
    <div className="page">
      <header className="page__head">
        <div>
          <h1 className="page-title">Tasks</h1>
          <p className="page-sub">Plan it, schedule it, focus on it.</p>
        </div>
        <div className="page__actions">
          <button className="btn btn--accent" type="button" onClick={() => setEditing("new")}>
            <Icon name="plus" size={16} /> New task
          </button>
        </div>
      </header>

      <div className="tiles" style={{ marginBottom: 16 }}>
        <div className="tile">
          <div className="faint tile__label">Open</div>
          <div className="tile__value display">{tiles?.open ?? 0}</div>
        </div>
        <div className="tile">
          <div className="faint tile__label">Due today</div>
          <div className="tile__value display">{tiles?.dueToday ?? 0}</div>
        </div>
        <div className="tile">
          <div className="faint tile__label">Overdue</div>
          <div className="tile__value display" style={{ color: tiles && tiles.overdue > 0 ? "var(--danger)" : undefined }}>
            {tiles?.overdue ?? 0}
          </div>
        </div>
        <div className="tile">
          <div className="faint tile__label">Done today</div>
          <div className="tile__value display">{tiles?.completedToday ?? 0}</div>
        </div>
      </div>

      <div className="toolbar">
        <Segmented
          value={view}
          onChange={setView}
          options={[
            { value: "tasks", label: "Tasks" },
            { value: "recurring", label: "Repeating" }
          ]}
        />
        {view === "tasks" ? <Segmented value={filter} onChange={setFilter} options={FILTERS} /> : null}
      </div>

      {error ? <p className="error-text" style={{ marginBottom: 10 }}>{error}</p> : null}

      {view === "tasks" ? (
        <div className="list">
          {!todos.loading && list.length === 0 ? <div className="empty">{EMPTY_TEXT[filter]}</div> : null}
          {list.map((todo) => (
            <article className={`item${todo.completed ? " is-done" : ""}`} key={todo.id}>
              <Check checked={todo.completed} label={`Complete ${todo.title}`} onChange={(next) => void toggle(todo, next)} />
              <div className="item__body">
                <div className="item__title">{todo.title}</div>
                <div className="item__meta">
                  {todo.overdue && !todo.completed ? <span className="chip chip--danger">Overdue</span> : null}
                  <PriorityChip priority={todo.priority} />
                  {todo.dueDate ? (
                    <span className="chip">
                      <Icon name="calendar" size={12} /> {describeDueDate(todo.dueDate)}
                      {todo.dueMinute !== null ? ` · ${formatMinuteOfDay(todo.dueMinute)}` : ""}
                    </span>
                  ) : null}
                  {todo.estimateMinutes ? (
                    <span className="chip">
                      <Icon name="clock" size={12} /> {formatMinutes(todo.estimateMinutes)}
                    </span>
                  ) : null}
                  {todo.templateId !== null ? (
                    <span className="chip">
                      <Icon name="repeat" size={12} /> Repeats
                    </span>
                  ) : null}
                </div>
                {todo.notes ? <div className="item__notes faint">{todo.notes}</div> : null}
              </div>
              <div className="item__actions">
                {!todo.completed ? (
                  <>
                    <button className="icon-btn" type="button" title="Focus on this" onClick={() => void focusOn(todo)}>
                      <Icon name="timer" size={17} />
                    </button>
                    <button className="icon-btn" type="button" title="Schedule" onClick={() => setScheduling(todo)}>
                      <Icon name="calendar" size={17} />
                    </button>
                  </>
                ) : null}
                <button className="icon-btn" type="button" title="Edit" onClick={() => setEditing({ todo })}>
                  <Icon name="edit" size={16} />
                </button>
                <button className="icon-btn icon-btn--danger" type="button" title="Delete" onClick={() => void remove(todo)}>
                  <Icon name="trash" size={16} />
                </button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="list">
          {(templates.data ?? []).length === 0 && !templates.loading ? (
            <div className="empty">
              <strong>No repeating tasks</strong>
              Create a task and switch on “Repeat” to see it here.
            </div>
          ) : null}
          {(templates.data ?? []).map((template) => (
            <article className={`item${template.active ? "" : " is-done"}`} key={template.id}>
              <div className="item__body">
                <div className="item__title">{template.title}</div>
                <div className="item__meta">
                  <span className="chip chip--accent">
                    <Icon name="repeat" size={12} /> {describeRecurrence(template.recurrence)}
                  </span>
                  <PriorityChip priority={template.priority} />
                  {template.dueMinute !== null ? <span className="chip">by {formatMinuteOfDay(template.dueMinute)}</span> : null}
                  {template.estimateMinutes ? <span className="chip">{formatMinutes(template.estimateMinutes)}</span> : null}
                </div>
              </div>
              <div className="item__actions">
                <Toggle
                  on={template.active}
                  label={`${template.title} active`}
                  onChange={(active) =>
                    void guard(() => call("set_todo_template_active", { templateId: template.id, active }))
                  }
                />
                <button className="icon-btn" type="button" title="Edit" onClick={() => setEditing({ template })}>
                  <Icon name="edit" size={16} />
                </button>
                <button
                  className="icon-btn icon-btn--danger"
                  type="button"
                  title="Delete"
                  onClick={() => void guard(() => call("delete_todo_template", { templateId: template.id }))}
                >
                  <Icon name="trash" size={16} />
                </button>
              </div>
            </article>
          ))}
        </div>
      )}

      {editing ? (
        <TaskModal
          todo={editing === "new" ? null : editing.todo}
          template={editing === "new" ? null : editing.template}
          onClose={() => setEditing(null)}
          onSaved={refresh}
        />
      ) : null}

      {scheduling ? <ScheduleModal todo={scheduling} onClose={() => setScheduling(null)} onDone={refresh} /> : null}
    </div>
  );
}
