// 编辑表单：改标题 / 指令；计划时间默认保持不变，可切换为修改频率。
// 绑定关系（会话 / 执行模式）创建后不可改，表单只读展示。
import { useState } from "react";
import type { TaskInfo, UpdateTaskInput } from "@ava-ai/app-sdk";
import { freqComplete, frequencyOf, FrequencyFields, type FreqState } from "./frequency-fields";
import { fmtWhen } from "./format";
import { BTN, BTN_PRIMARY, FIELD, INPUT, LABEL, TEXTAREA } from "./styles";

interface EditFormProps {
  task: TaskInfo;
  onSubmit: (input: UpdateTaskInput) => void;
  onCancel: () => void;
}

export function EditForm({ task, onSubmit, onCancel }: EditFormProps) {
  const [title, setTitle] = useState(task.title);
  const [prompt, setPrompt] = useState(task.prompt);
  const [editWhen, setEditWhen] = useState(false);
  const [freq, setFreq] = useState<FreqState>({ kind: "every", at: "", minutes: 30, dailyTime: "", cron: "" });

  const valid =
    title.trim() !== "" && prompt.trim() !== "" && (!editWhen || freqComplete(freq));

  return (
    <div
      style={{
        border: "0.5px solid var(--ava-border-l3)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-3, 10px)",
      }}
    >
      <b style={{ fontSize: 13 }}>编辑定时任务</b>
      <label style={FIELD}>
        <span style={LABEL}>标题</span>
        <input style={INPUT} value={title} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label style={FIELD}>
        <span style={LABEL}>指令（到期投递给会话执行）</span>
        <textarea style={TEXTAREA} value={prompt} onChange={(e) => setPrompt(e.target.value)} />
      </label>
      <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
        <span style={LABEL}>计划时间</span>
        <button
          style={{
            ...BTN,
            ...(editWhen
              ? {}
              : { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }),
          }}
          onClick={() => setEditWhen(false)}
        >
          保持不变（下次 {fmtWhen(task.scheduledAt)}）
        </button>
        <button
          style={{
            ...BTN,
            ...(editWhen
              ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
              : {}),
          }}
          onClick={() => setEditWhen(true)}
        >
          修改计划
        </button>
      </div>
      {editWhen ? <FrequencyFields freq={freq} setFreq={setFreq} /> : null}
      <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)" }}>
        执行模式与绑定会话创建后不可修改：{task.sessionMode === "independent" ? "独立会话（到期新建）" : "绑定会话 " + task.sessionId.slice(0, 12) + "…"}
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: "var(--ava-space-2, 8px)" }}>
        <button style={BTN} onClick={onCancel}>
          取消
        </button>
        <button
          style={BTN_PRIMARY}
          disabled={!valid}
          onClick={() =>
            onSubmit({
              title: title.trim(),
              prompt: prompt.trim(),
              ...(editWhen ? { frequency: frequencyOf(freq) } : {}),
            })
          }
        >
          保存
        </button>
      </div>
    </div>
  );
}
