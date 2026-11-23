// 频率表单（四种模式，对齐 ava-app 任务中心）：at / every / daily / cron。
import type { CreateTaskFrequency } from "@ava-ai/app-sdk";
import { localToRfc3339, TZ } from "./format";
import { BTN, FIELD, INPUT, LABEL } from "./styles";

/** 表单态频率（构建入参时映射为边车 CreateTaskFrequency） */
export interface FreqState {
  kind: "at" | "every" | "daily" | "cron";
  at: string;
  minutes: number;
  dailyTime: string;
  cron: string;
}

/** 表单态 → 边车频率入参 */
export function frequencyOf(freq: FreqState): CreateTaskFrequency {
  if (freq.kind === "at") return { kind: "at", instant: localToRfc3339(freq.at) };
  if (freq.kind === "every") return { kind: "every", seconds: Math.max(60, (freq.minutes || 1) * 60) };
  if (freq.kind === "daily") return { kind: "daily", time: `${freq.dailyTime || "09:00"}:00`, time_zone: TZ };
  return { kind: "cron", expression: freq.cron.trim(), time_zone: TZ };
}

/** 表单态是否已填完整（用于提交按钮可用性） */
export function freqComplete(freq: FreqState): boolean {
  if (freq.kind === "at") return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(freq.at);
  if (freq.kind === "every") return (freq.minutes || 0) >= 1;
  if (freq.kind === "daily") return /^\d{2}:\d{2}$/.test(freq.dailyTime);
  return freq.cron.trim().length > 0;
}

interface FrequencyFieldsProps {
  freq: FreqState;
  setFreq: (next: FreqState) => void;
}

interface FieldFProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  inputType?: string;
}

function FieldF({ label, value, onChange, placeholder, inputType }: FieldFProps) {
  return (
    <label style={FIELD}>
      <span style={LABEL}>{label}</span>
      <input
        style={INPUT}
        value={value}
        placeholder={placeholder}
        type={inputType || "text"}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

export function FrequencyFields({ freq, setFreq }: FrequencyFieldsProps) {
  const set = (patch: Partial<FreqState>) => setFreq({ ...freq, ...patch });
  const kind = freq.kind;
  return (
    <div style={{ display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap", alignItems: "flex-end" }}>
      <div style={{ display: "flex", gap: 6 }}>
        {(["at", "every", "daily", "cron"] as const).map((k) => (
          <button
            key={k}
            style={{
              ...BTN,
              ...(kind === k
                ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
                : {}),
            }}
            onClick={() => set({ kind: k })}
          >
            {{ at: "一次性", every: "每隔", daily: "每天", cron: "Cron" }[k]}
          </button>
        ))}
      </div>
      {kind === "at" ? (
        <FieldF label="目标时间" value={freq.at} onChange={(v) => set({ at: v })} inputType="datetime-local" />
      ) : null}
      {kind === "every" ? (
        <FieldF
          label="间隔（分钟，≥1）"
          value={String(freq.minutes ?? 30)}
          onChange={(v) => set({ minutes: Math.max(1, Math.floor(Number(v) || 1)) })}
          inputType="number"
        />
      ) : null}
      {kind === "daily" ? (
        <FieldF label="时间（HH:mm）" value={freq.dailyTime} onChange={(v) => set({ dailyTime: v })} inputType="time" />
      ) : null}
      {kind === "cron" ? (
        <FieldF label="Cron 表达式（5/6 字段）" value={freq.cron} onChange={(v) => set({ cron: v })} placeholder="0 9 * * *" />
      ) : null}
    </div>
  );
}
