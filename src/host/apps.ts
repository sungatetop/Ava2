// App 仓库客户端：安装 / 卸载 / 启停 / 静态产物 URL。
// 安装模型——App 是「预构建产物目录」，安装即由边车复制进
// `<ONION_HOME>/apps/<id>/` 并登记；宿主不参与构建，web-ui 与桌面端同一套 HTTP 面。
import type { AppManifest } from "./types";
import { getAgentBase } from "./agent";

/** 一条已安装 App：清单 + 启停态 + 安装时间 */
export interface InstalledApp {
  manifest: AppManifest;
  enabled: boolean;
  installedAtMs: number;
}

async function unwrap(res: Response): Promise<Response> {
  if (res.ok) return res;
  let detail = "";
  try {
    detail = await res.text();
  } catch {
    /* 忽略 */
  }
  throw new Error(`HTTP ${res.status}${detail ? `: ${detail}` : ""}`);
}

/** App 产物根 URL（相对路径解析基准，结尾必须带 `/`） */
export function appAssetUrl(appId: string, path = ""): string {
  return `${getAgentBase()}/v1/apps/${encodeURIComponent(appId)}/${path}`;
}

/** 已安装清单（边车登记表为唯一权威） */
export async function listInstalled(): Promise<InstalledApp[]> {
  const res = await unwrap(await fetch(`${getAgentBase()}/v1/apps`, { cache: "no-store" }));
  const data = (await res.json()) as {
    apps?: Array<{ manifest: AppManifest; enabled?: boolean; installed_at_ms?: number }>;
  };
  return (data.apps ?? []).map((a) => ({
    manifest: a.manifest,
    enabled: a.enabled ?? true,
    installedAtMs: a.installed_at_ms ?? 0,
  }));
}

/** 从本地目录安装（复制 + 校验 + 登记；overwrite 覆盖已装版本） */
export async function installFromDir(path: string, overwrite = false): Promise<InstalledApp> {
  const res = await unwrap(
    await fetch(`${getAgentBase()}/v1/apps/install`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path, overwrite }),
    }),
  );
  const data = (await res.json()) as { manifest: AppManifest; enabled: boolean; installed_at_ms: number };
  return { manifest: data.manifest, enabled: data.enabled, installedAtMs: data.installed_at_ms };
}

/** 卸载：移除目录与登记项（宿主侧由调用方卸载实例） */
export async function uninstall(appId: string): Promise<void> {
  await unwrap(
    await fetch(`${getAgentBase()}/v1/apps/${encodeURIComponent(appId)}`, { method: "DELETE" }),
  );
}

/** 启停（持久化到边车登记表，重启后保持） */
export async function setEnabled(appId: string, enabled: boolean): Promise<void> {
  await unwrap(
    await fetch(`${getAgentBase()}/v1/apps/${encodeURIComponent(appId)}/enabled`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ enabled }),
    }),
  );
}
