// 双端适配层：AppHost 同时跑在桌面端（Tauri）与浏览器（web-ui）。
// - 桌面端：边车端口由壳动态分配，经 invoke("backend_base") 取回；
// - web-ui：默认 http://127.0.0.1:9800，可用 ?backend= 覆盖。
// App 一律经 AgentChannel 访问边车，禁止自行判断运行环境或使用 Tauri 特权能力。

/** 边车默认端口（与 src-tauri/src/sidecar.rs 的 DEFAULT_PORT 保持一致） */
const DEFAULT_PORT = 9800;

/** 当前是否运行在 Tauri 桌面壳内（Tauri v2 注入 __TAURI_INTERNALS__） */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** URL 参数覆盖（web-ui 与桌面端调试均可用）：?backend=http://host:port */
function backendOverride(): string | undefined {
  const raw = new URLSearchParams(location.search).get("backend");
  return raw ? raw.replace(/\/$/, "") : undefined;
}

let resolved: Promise<string> | undefined;

/** 解析后端基址（一次性，进程内缓存）：桌面端优先，其次 URL 参数，最后默认端口 */
export function resolveBackendBase(): Promise<string> {
  return (resolved ??= (async () => {
    const override = backendOverride();
    if (isDesktop()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const base = await invoke<string>("backend_base");
        if (base) return base.replace(/\/$/, "");
      } catch (e) {
        console.warn("[apphost] 桌面端取后端基址失败，回落到默认端口：", e);
      }
    }
    return override ?? `http://127.0.0.1:${DEFAULT_PORT}`;
  })());
}
