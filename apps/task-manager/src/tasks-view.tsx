// 定时任务主视图：任务列表（5s 轮询）、创建、启停、立即执行、删除（气泡确认可关）。
// 调度由边车后台循环驱动（10s tick）；本视图只做 CRUD 展示，不本地 setTimeout。
import { useCallback, useEffect, useState } from "react";
import type { AgentChannel, CreateTaskInput, TaskInfo } from "@ava-ai/app-sdk";
import { readConfig } from "./config";
import { fmtWhen, KIND_LABEL } from "./format";
import { CreateForm } from "./create-form";
import { EditForm } from "./edit-form";
import { Popconfirm } from "./popconfirm";
import { BTN, BTN_DANGER, BTN_PRIMARY, CHIP } from "./styles";

interface TasksViewProps {
  agent: AgentChannel;
}

export function TasksView({ agent }: TasksViewProps) {
  const [confirmDelete, setConfirmDelete] = useState(() => readConfig().confirmDelete);
  const [tasks, setTasks] = useState<TaskInfo[] | null>(null);
  const [err, setErr] = useState("");
  const [note, setNote] = useState("");
  const [confirmId, setConfirmId] = useState("");
  const [editingId, setEditingId] = useState("");
  const [creating, setCreating] = useState(false);
  const [busyId, setBusyId] = useState("");

  const load = useCallback(() => {
    agent
      .listTasks()
      .then((r) => setTasks(Array.isArray(r.tasks) ? r.tasks : []))
      .catch((e) => setErr(e.message || String(e)));
  }, [agent]);

  useEffect(() => {
    load();
    // 状态 5s 轮询（调度在边车，前端只刷新视图）；同时拾取设置面板的最新配置
    const timer = setInterval(() => {
      setConfirmDelete(readConfig().confirmDelete);
      load();
    }, 5000);
    return () => clearInterval(timer);
  }, [load]);

  const run = async (fn: () => Promise<unknown>, okNote?: string): Promise<boolean> => {
    setErr("");
    setNote("");
    try {
      await fn();
      if (okNote) setNote(okNote);
      load();
      return true;
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
      return false;
    }
  };

  const toggle = (task: TaskInfo) => {
    void run(
      () => agent.toggleTask(task.id, !task.enabled),
      task.enabled ? `已停用「${task.title}」` : `已启用「${task.title}」`,
    );
  };

  const runNow = async (task: TaskInfo) => {
    setBusyId(task.id);
    await run(() => agent.runTask(task.id), `「${task.title}」已投递`);
    setBusyId("");
  };

  const removeTask = (task: TaskInfo) => {
    void run(() => agent.deleteTask(task.id), `已删除「${task.title}」`);
  };

  if (err && !tasks) {
    return (
      <div style={{ padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }}>
        加载失败：{err}
        <button style={{ ...BTN, marginLeft: 12 }} onClick={load}>
          重试
        </button>
      </div>
    );
  }
  if (!tasks) {
    return <div style={{ padding: "var(--ava-space-5, 24px)", color: "var(--ava-label-tertiary)", fontSize: 13 }}>加载中…</div>;
  }

  const deleteGuard = (task: TaskInfo) => (confirmDelete ? () => setConfirmId(task.id) : () => removeTask(task));

  return (
    <div
      style={{
        padding: "var(--ava-space-5, 24px)",
        overflow: "auto",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-4, 16px)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)" }}>
        <h2 style={{ margin: 0, fontSize: 16 }}>定时任务</h2>
        <span style={{ fontSize: 12, color: "var(--ava-label-tertiary)" }}>{tasks.length} 个任务 · 5s 自动刷新</span>
        <span style={{ flex: 1 }} />
        <button style={BTN_PRIMARY} onClick={() => setCreating(!creating)}>
          {creating ? "收起" : "+ 新建任务"}
        </button>
      </div>
      {note || err ? (
        <div style={{ fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }}>{err || note}</div>
      ) : null}
      {creating ? (
        <CreateForm
          agent={agent}
          onSubmit={async (fields) => {
            const ok = await run(() => agent.createTask(fields), `已创建「${fields.title}」`);
            if (ok) setCreating(false);
          }}
          onCancel={() => setCreating(false)}
        />
      ) : null}
      {tasks.map((task) => (
        <div
          key={task.id}
          style={{
            border: "0.5px solid var(--ava-border-l2)",
            borderRadius: "var(--ava-radius-lg, 16px)",
            padding: "var(--ava-space-4, 16px)",
            background: "var(--ava-bg-layer-1)",
            display: "flex",
            flexDirection: "column",
            gap: 6,
            opacity: task.enabled ? 1 : 0.55,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }}>
            <b style={{ fontSize: 13 }}>{task.title}</b>
            <span style={CHIP(task.enabled ? "var(--ava-state-success)" : "var(--ava-label-tertiary)")}>
              {task.enabled ? "启用中" : "已停用"}
            </span>
            <span style={CHIP()}>{KIND_LABEL[task.kind] || task.kind}</span>
            <span style={CHIP()}>{task.sessionMode === "independent" ? "独立会话" : "绑定会话"}</span>
            <span style={{ flex: 1 }} />
            <button
              style={BTN}
              onClick={() => setEditingId(editingId === task.id ? "" : task.id)}
            >
              {editingId === task.id ? "收起" : "编辑"}
            </button>
            <button style={BTN} disabled={busyId === task.id} onClick={() => void runNow(task)}>
              {busyId === task.id ? "执行中…" : "立即执行"}
            </button>
            <button style={BTN} onClick={() => toggle(task)}>
              {task.enabled ? "停用" : "启用"}
            </button>
            <span style={{ position: "relative" }}>
              <button style={BTN_DANGER} onClick={deleteGuard(task)}>
                删除
              </button>
              {confirmId === task.id ? (
                <Popconfirm
                  title={`删除任务「${task.title}」？`}
                  onCancel={() => setConfirmId("")}
                  onConfirm={() => removeTask(task)}
                />
              ) : null}
            </span>
          </div>
          <div
            style={{
              fontSize: 12,
              color: "var(--ava-label-secondary)",
              lineHeight: 1.6,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              display: "-webkit-box",
              WebkitLineClamp: 2,
              WebkitBoxOrient: "vertical",
              overflow: "hidden",
            }}
          >
            {task.prompt}
          </div>
          <div
            style={{
              display: "flex",
              gap: "var(--ava-space-4, 16px)",
              fontSize: 11,
              color: "var(--ava-label-tertiary)",
              flexWrap: "wrap",
            }}
          >
            <span>
              下次触发：
              <b
                style={{
                  fontFamily: "var(--ava-font-mono, monospace)",
                  fontWeight: 500,
                  color: task.enabled ? "var(--ava-label-secondary)" : "var(--ava-label-tertiary)",
                }}
              >
                {fmtWhen(task.scheduledAt)}
              </b>
            </span>
            {task.sessionMode !== "independent" && task.sessionId ? (
              <span style={{ fontFamily: "var(--ava-font-mono, monospace)" }}>会话 {task.sessionId.slice(0, 12)}…</span>
            ) : null}
            {task.lastDeliveredAt ? <span>上次递送：{fmtWhen(task.lastDeliveredAt)}</span> : null}
          </div>
          {editingId === task.id ? (
            <EditForm
              task={task}
              onCancel={() => setEditingId("")}
              onSubmit={async (input) => {
                const ok = await run(() => agent.updateTask(task.id, input), `已保存「${input.title ?? task.title}」`);
                if (ok) setEditingId("");
              }}
            />
          ) : null}
        </div>
      ))}
      {tasks.length === 0 && !creating ? (
        <div style={{ padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }}>
          尚无定时任务——右上角「+ 新建任务」开始创建。
        </div>
      ) : null}
    </div>
  );
}
