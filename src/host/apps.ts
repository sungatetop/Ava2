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

/** App Hub 中的一条可安装 App（远程仓库 apps/ 目录） */
export interface HubApp {
  manifest: AppManifest;
  /** 登记表口径：本机已安装 */
  installed: boolean;
}

/** App Hub 列表结果 */
export interface HubList {
  /** 仓库坐标（`owner/repo@branch`） */
  source: string;
  apps: HubApp[];
}

/** 远程可安装 App 列表（关键字可选，服务端按 name/id/description 过滤） */
export async function listHub(query = ""): Promise<HubList> {
  const suffix = query.trim() ? `?q=${encodeURIComponent(query.trim())}` : "";
  const res = await unwrap(
    await fetch(`${getAgentBase()}/v1/apps/hub${suffix}`, { cache: "no-store" }),
  );
  const data = (await res.json()) as { source?: string; apps?: HubApp[] };
  return { source: data.source ?? "", apps: data.apps ?? [] };
}

/** 从 App Hub 下载并安装（边车按仓库文件树逐文件下载 → 校验 → 复制 → 登记） */
export async function installFromHub(id: string, overwrite = false): Promise<InstalledApp> {
  const res = await unwrap(
    await fetch(`${getAgentBase()}/v1/apps/hub/install`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id, overwrite }),
    }),
  );
  const data = (await res.json()) as { manifest: AppManifest; enabled: boolean; installed_at_ms: number };
  return { manifest: data.manifest, enabled: data.enabled, installedAtMs: data.installed_at_ms };
}
