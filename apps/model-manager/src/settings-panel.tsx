// 设置面板（宿主只提供入口：ctx.registerSettingsPanel；表单/存储/校验由本 App 自理）。
import { useState } from "react";
import { readConfig, writeConfig } from "./config";

export default function SettingsPanel() {
  const [confirmDelete, setConfirmDelete] = useState(() => readConfig().confirmDelete);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10, padding: "4px 0", fontSize: 13 }}>
      <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
        <input
          type="checkbox"
          checked={confirmDelete}
          onChange={(e) => {
            setConfirmDelete(e.target.checked);
            writeConfig({ confirmDelete: e.target.checked });
          }}
        />
        删除前弹出气泡确认
      </label>
      <div style={{ fontSize: 12, color: "var(--ava-label-tertiary)" }}>
        配置由本 App 自行存储（localStorage），即时生效。
      </div>
    </div>
  );
}
