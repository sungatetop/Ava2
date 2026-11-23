// 桌面端更新提示：订阅主进程（src-tauri/src/update.rs）推送的发现新版事件。
// 浏览器模式下无 Tauri 运行时，hook 恒返回空态（不弹任何横幅）。

import { useCallback, useEffect, useState } from "react";

/** 与 `src-tauri/src/update.rs` 的 `UPDATE_EVENT` 保持一致 */
const UPDATE_EVENT = "ava2://update-available";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes?: string;
}

export interface UpdateNotice {
  info: UpdateInfo | null;
  installing: boolean;
  error: string | null;
  install: () => Promise<void>;
  dismiss: () => void;
}

/** 是否运行在 Tauri 壳内 */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function useUpdateNotice(): UpdateNotice {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!inTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen<UpdateInfo>(UPDATE_EVENT, (event) => {
        if (!disposed) setInfo(event.payload);
      });
      if (disposed) un();
      else unlisten = un;
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const install = useCallback(async () => {
    setInstalling(true);
    setError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      // 主进程：下载 → 校验签名 → 停边车 → 安装 → 重启（成功时不返回）
      await invoke("install_update");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setInstalling(false);
    }
  }, []);

  const dismiss = useCallback(() => setInfo(null), []);

  return { info, installing, error, install, dismiss };
}
