// 创建表单（执行模式 + 频率，双态对齐 ava-app 任务中心）。
// 全部能力经 props 注入（agent 由 setup 闭包传入），组件自身不引用 ctx。
import { useEffect, useState } from "react";
import type { AgentChannel, CreateTaskInput, SessionListItem } from "@ava-ai/app-sdk";
import { freqComplete, frequencyOf, FrequencyFields, type FreqState } from "./frequency-fields";
import { BTN, BTN_PRIMARY, FIELD, INPUT, LABEL, TEXTAREA } from "./styles";

interface CreateFormProps {
  agent: AgentChannel;
  onSubmit: (input: CreateTaskInput) => void;
  onCancel: () => void;
}

type SessionMode = "independent" | "single";

export function CreateForm({ agent, onSubmit, onCancel }: CreateFormProps) {
  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState<SessionMode>("independent");
  const [sessionId, setSessionId] = useState("");
  const [freq, setFreq] = useState<FreqState>({ kind: "at", at: "", minutes: 30, dailyTime: "", cron: "" });
  const [sessionOptions, setSessionOptions] = useState<SessionListItem[] | null>(null);

  // 绑定会话模式：加载会话列表（一次）
  useEffect(() => {
    if (mode !== "single" || sessionOptions) return;
    agent
      .listSessions()
      .then((list) => {
        setSessionOptions(list);
        if (list.length > 0) setSessionId((cur) => cur || list[0].id);
      })
      .catch(() => setSessionOptions([]));
  }, [mode, sessionOptions, agent]);

  const freqValid = freqComplete(freq);
  const valid = title.trim() !== "" && prompt.trim() !== "" && freqValid && (mode === "independent" || sessionId !== "");

  return (
    <div
      style={{
        border: "0.5px solid var(--ava-border-l2)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-3, 10px)",
      }}
    >
      <b style={{ fontSize: 13 }}>新建定时任务</b>
      <label style={FIELD}>
        <span style={LABEL}>标题</span>
        <input style={INPUT} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="如：每天早上汇报进度" />
      </label>
      <label style={FIELD}>
        <span style={LABEL}>Prompt（到期投递给会话执行）</span>
        <textarea
          style={TEXTAREA}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="要 agent 执行的指令…"
        />
      </label>
      {/* 执行模式 */}
      <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
        <span style={LABEL}>执行模式</span>
        <button
          style={{
            ...BTN,
            ...(mode === "independent"
              ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
              : {}),
          }}
          onClick={() => setMode("independent")}
        >
          独立会话（到期新建）
        </button>
        <button
          style={{
            ...BTN,
            ...(mode === "single"
              ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
              : {}),
          }}
          onClick={() => setMode("single")}
        >
          绑定会话
        </button>
        {mode === "single" ? (
          sessionOptions === null ? (
            <span style={LABEL}>会话加载中…</span>
          ) : (
            <select style={{ ...INPUT, width: 260 }} value={sessionId} onChange={(e) => setSessionId(e.target.value)}>
              {sessionOptions.map((s) => (
                <option key={s.id} value={s.id}>
                  {`${s.title || s.id}`.slice(0, 40)}
                </option>
              ))}
            </select>
          )
        ) : null}
      </div>
      <FrequencyFields freq={freq} setFreq={setFreq} />
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
              ...(mode === "independent" ? { sessionMode: "independent" } : { sessionId }),
              frequency: frequencyOf(freq),
            })
          }
        >
          创建
        </button>
      </div>
    </div>
  );
}
