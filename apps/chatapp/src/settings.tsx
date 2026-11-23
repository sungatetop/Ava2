// chatapp 设置面板（v3 机制示例）：宿主零策略——表单 UI、存储、校验全部由本 App 自理。
// App 与宿主同源，配置直接存 localStorage（自定键名，避免与其他 App 冲突）。
// 本面板不消费任何 ctx 能力面，仅演示「设置」入口的最小实现。
import { useState } from "react";

/** 配置键（App 自定命名空间） */
const CFG_KEY = "chatapp:cfg";

interface ChatAppConfig {
  nickname?: string;
  /** 强调色（纯演示：写入后由 App 自行决定如何消费） */
  accent?: "blue" | "green" | "orange";
}

function readConfig(): ChatAppConfig {
  try {
    return JSON.parse(localStorage.getItem(CFG_KEY) || "{}") as ChatAppConfig;
  } catch {
    return {};
  }
}

const row: React.CSSProperties = { display: "flex", alignItems: "center", gap: 10, fontSize: 13 };
const input: React.CSSProperties = {
  padding: "5px 10px",
  border: "0.5px solid var(--ava-border-l2)",
  borderRadius: 8,
  background: "var(--ava-bg-base)",
  color: "var(--ava-label-primary)",
  font: "inherit",
};

/** 设置面板：应用市场「设置」入口点击后由宿主惰性挂载（ErrorBoundary 隔离） */
export default function SettingsPanel() {
  const [cfg, setCfg] = useState<ChatAppConfig>(readConfig);
  const save = (patch: Partial<ChatAppConfig>) => {
    const next = { ...readConfig(), ...patch };
    localStorage.setItem(CFG_KEY, JSON.stringify(next));
    setCfg(next);
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12, padding: "4px 0" }}>
      <label style={row}>
        <span style={{ width: 64 }}>昵称</span>
        <input
          style={{ ...input, flex: 1 }}
          value={cfg.nickname ?? ""}
          placeholder="给自己起个名字"
          onChange={(e) => save({ nickname: e.target.value || undefined })}
        />
      </label>
      <label style={row}>
        <span style={{ width: 64 }}>强调色</span>
        <select
          style={{ ...input, flex: 1 }}
          value={cfg.accent ?? ""}
          onChange={(e) => save({ accent: (e.target.value || undefined) as ChatAppConfig["accent"] })}
        >
          <option value="">跟随宿主</option>
          <option value="blue">蓝</option>
          <option value="green">绿</option>
          <option value="orange">橙</option>
        </select>
      </label>
      <div style={{ fontSize: 12, color: "var(--ava-label-tertiary)" }}>
        配置由本 App 自行存储（localStorage），即时生效；宿主不参与存储与校验。
      </div>
    </div>
  );
}
