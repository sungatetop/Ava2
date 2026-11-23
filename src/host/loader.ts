// 发现层：边车 App 仓库（`GET /v1/apps`）是唯一来源。
// 安装式模型：
// - App 是预构建产物目录，安装由边车复制进 `<ONION_HOME>/apps/<id>/` 并登记；
// - 宿主不扫本机目录、不读 public/、不做构建，只按登记表装配；
// - 清单内容与启停态同源返回，产物由 `/v1/apps/{id}/{*path}` 提供。
import type { AppManifest } from "./types";
import { registry } from "./registry";
import { parseManifest } from "./manifest";
import { setAgentBase } from "./agent";
import { resolveBackendBase } from "./env";
import { appAssetUrl, listInstalled, type InstalledApp } from "./apps";

export interface BootInfo {
  /** 后端基址（onion-server） */
  backend: string;
  /** 后端健康检查结果；false 时仍尝试加载已装 App（产物也在边车，故通常同时失败） */
  backendReady: boolean;
  backendError?: string;
  /** 登记表中已安装的 App 数量 */
  appCount: number;
}

let bootInfo: BootInfo | undefined;

export function getBootInfo(): BootInfo | undefined {
  return bootInfo;
}

/** 把仓库条目装配成宿主清单：baseUrl 指向边车产物根，启停态来自登记表 */
export function toManifest(app: InstalledApp): AppManifest {
  return {
    ...app.manifest,
    baseUrl: appAssetUrl(app.manifest.id),
    enabled: app.enabled,
  };
}

/** 读取已安装清单（清单非法者登记为 failed，不静默降级） */
export async function fetchInstalledManifests(): Promise<AppManifest[]> {
  const installed = await listInstalled();
  const out: AppManifest[] = [];
  for (const app of installed) {
    const { manifest, issues } = parseManifest(app.manifest as unknown as Record<string, unknown>, app.manifest.id);
    if (issues.length > 0) {
      console.error(
        `[apphost] manifest 无效 App「${app.manifest.id}」：`,
        issues.map((i) => `${i.field} ${i.message}`).join("；"),
      );
      out.push({ ...toManifest(app), enabled: false });
      continue;
    }
    // 清单以边车登记的为准，但保留服务端返回的原始字段（contributes 等已由 parseManifest 归一）
    out.push(toManifest({ ...app, manifest: { ...app.manifest, ...manifest } }));
  }
  return out;
}

/** 健康检查：边车可达性只作展示，失败不阻断装配尝试 */
async function probeBackend(base: string): Promise<{ ready: boolean; error?: string }> {
  try {
    const res = await fetch(`${base}/v1/health`, { cache: "no-store" });
    if (!res.ok) return { ready: false, error: `HTTP ${res.status}` };
    return { ready: true };
  } catch (e) {
    return { ready: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** 启用者并行 load；disabled 登记 suspended 占位 */
async function boot(manifests: AppManifest[]): Promise<void> {
  await Promise.all(manifests.filter((m) => m.enabled !== false).map((m) => registry.load(m)));
  manifests.filter((m) => m.enabled === false).forEach((m) => registry.registerSuspended(m));
}

/** 宿主启动：解析后端基址 → 健康检查 → 读已安装清单 → 并行装配 */
export async function discoverAndBoot(): Promise<void> {
  const base = await resolveBackendBase();
  setAgentBase(base); // App 仓库与 Agent 通道共用同一后端基址
  const health = await probeBackend(base);
  let manifests: AppManifest[] = [];
  try {
    manifests = await fetchInstalledManifests();
  } catch (e) {
    console.error("[apphost] 读取已安装 App 失败：", e);
  }
  bootInfo = { backend: base, backendReady: health.ready, backendError: health.error, appCount: manifests.length };
  await boot(manifests);
  registry.poke(); // 注册表外快照（bootInfo）更新后驱动 UI
  console.info(`[apphost] boot: backend=${base} ready=${health.ready} apps=${manifests.length}`);
}

/** 安装完成后增量装配（不重启宿主）：登记表已由边车写入 */
export async function loadInstalled(app: InstalledApp): Promise<void> {
  await registry.load(toManifest(app));
}
