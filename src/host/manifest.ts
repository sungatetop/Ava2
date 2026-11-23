// manifest 解析与校验
// 职责：
// 1) 校验 manifest 字段（hostApi 版本 / inject 键 / surfaces 越权）；
// 2) 按 hostApi 解析实际注出面（hostApi 0 = 旧契约全量注入，向后兼容）。
// App 配置不在 manifest 契约内（宿主零策略）：如何存储、如何校验由 App 自行决定，
// 宿主只提供设置入口机制（ctx.registerSettingsPanel，见 types.ts）。
// 校验一律"响亮报错"：错误信息带字段路径，便于定位。
import { SLOT_KEYS, type AppManifest, type InjectKey, type SlotKey } from "./types";

/** 注出键全集（manifest.inject 的合法取值） */
export const INJECT_KEYS: readonly InjectKey[] = ["slots", "events", "agent", "storage", "net"];

/** 宿主当前支持的最高 hostApi 版本（2 = 含能力贡献 contributes） */
export const HOST_API_SUPPORTED = 2;

export interface ManifestIssue {
  field: string;
  message: string;
}

export interface ParsedManifest {
  manifest: AppManifest;
  issues: ManifestIssue[];
}

/** manifest 校验：字段级问题收集（不抛异常，由调用方决定 fail-loud 落地方式） */
export function parseManifest(raw: unknown, fallbackId?: string): ParsedManifest {
  const issues: ManifestIssue[] = [];
  const m = (raw ?? {}) as Partial<AppManifest> & Record<string, unknown>;
  const manifest: AppManifest = {
    id: typeof m.id === "string" && m.id ? m.id : (fallbackId ?? ""),
    name: typeof m.name === "string" ? m.name : "",
    version: typeof m.version === "string" ? m.version : "0.0.0",
    entry: typeof m.entry === "string" ? m.entry : "",
  };
  if (!manifest.id) issues.push({ field: "id", message: "缺少 id" });
  if (!manifest.name) issues.push({ field: "name", message: "缺少 name" });
  if (typeof m.description === "string" && m.description.trim()) {
    manifest.description = m.description.trim();
  }
  const hostApi = m.hostApi ?? 0;
  if (typeof hostApi !== "number" || !Number.isInteger(hostApi) || hostApi < 0) {
    issues.push({ field: "hostApi", message: `hostApi 必须是非负整数（当前 ${JSON.stringify(m.hostApi)}）` });
  } else if (hostApi > HOST_API_SUPPORTED) {
    issues.push({
      field: "hostApi",
      message: `hostApi ${hostApi} 超出宿主支持上限 ${HOST_API_SUPPORTED}（请升级宿主）`,
    });
  }
  manifest.hostApi = hostApi;

  if (m.permissions !== undefined) {
    if (!Array.isArray(m.permissions) || m.permissions.some((p) => typeof p !== "string")) {
      issues.push({ field: "permissions", message: "permissions 必须是字符串数组" });
    } else {
      manifest.permissions = m.permissions as string[];
    }
  }

  if (m.inject !== undefined) {
    if (!Array.isArray(m.inject)) {
      issues.push({ field: "inject", message: "inject 必须是数组" });
    } else {
      const bad = m.inject.filter((k) => !INJECT_KEYS.includes(k as InjectKey));
      if (bad.length) issues.push({ field: "inject", message: `未知注出键：${bad.join(", ")}（合法值 ${INJECT_KEYS.join("/")}）` });
      manifest.inject = m.inject as InjectKey[];
    }
  }

  // net.allow：联网域白名单（host 名或父域，无 scheme/路径）。仅校验形态；
  // 匹配语义（host 后缀匹配）在 registry 的 net 能力面。
  if (m.net !== undefined) {
    const net = m.net as { allow?: unknown };
    if (typeof net !== "object" || net === null) {
      issues.push({ field: "net", message: "net 必须是对象（{ allow?: string[] }）" });
    } else if (net.allow !== undefined) {
      if (!Array.isArray(net.allow)) {
        issues.push({ field: "net", message: "net.allow 必须是字符串数组" });
      } else {
        const bad = net.allow.filter(
          (d) => typeof d !== "string" || d.trim() === "" || d.includes("://") || d.includes("/"),
        );
        if (bad.length) {
          issues.push({ field: "net", message: `net.allow 含非法域项：${bad.map(String).join(", ")}（应为裸域名，如 api.skillhub.cn）` });
        } else {
          manifest.net = { allow: net.allow as string[] };
        }
      }
    }
  }

  if (m.surfaces !== undefined) {
    if (!Array.isArray(m.surfaces)) {
      issues.push({ field: "surfaces", message: "surfaces 必须是数组" });
    } else {
      const bad = m.surfaces.filter((k) => !SLOT_KEYS.includes(k as SlotKey));
      if (bad.length) issues.push({ field: "surfaces", message: `未知 slot key：${bad.join(", ")}（合法值 ${SLOT_KEYS.join("/")}）` });
      manifest.surfaces = m.surfaces as SlotKey[];
    }
  }

  if (m.kind !== undefined) {
    if (m.kind !== "preset" && m.kind !== "demo") {
      issues.push({ field: "kind", message: `kind 只能是 preset/demo（当前 ${JSON.stringify(m.kind)}）` });
    } else {
      manifest.kind = m.kind;
    }
  }
  if (m.enabled !== undefined) {
    if (typeof m.enabled !== "boolean") issues.push({ field: "enabled", message: "enabled 必须是布尔值" });
    else manifest.enabled = m.enabled;
  }
  // 贡献声明归一：顶层 contributes 为唯一权威声明处；旧形态（skills/mcpServers/mcp/hooks/
  // agents/apps 写在清单顶层）在 contributes 缺该类时补齐映射。
  const normalized = normalizeContributes(m);
  if (normalized !== undefined) manifest.contributes = normalized as AppManifest["contributes"];
  // entry 为兼容位：权威入口在 contributes.apps[].entry（多页面）。
  // - 有 contributes 且声明了 apps → entry 取首个 UI 页入口；
  // - 有 contributes 但未声明 apps → 纯能力 plugin（无 UI 页），entry 允许为空；
  // - 无 contributes（旧形态单页 App）→ entry 必填。
  if (!manifest.entry) {
    const pages = manifest.contributes?.apps;
    const first = Array.isArray(pages) ? pages[0] : undefined;
    if (first && typeof first.entry === "string") manifest.entry = first.entry;
    else if (!manifest.contributes) {
      issues.push({ field: "entry", message: "缺少 entry（或 contributes.apps[0].entry）" });
    }
  }
  return { manifest, issues };
}

/**
 * contributes 兼容归一：
 * - 新形态：顶层 `contributes`（skills / mcpServers / agents / hooks / apps）；
 * - 旧形态：清单顶层同名字段（按各类别的形态规则区分）；
 * - 合并规则：`contributes` 存在时以其为准，仅补齐它未声明的类别。
 */
export function normalizeContributes(
  m: Record<string, unknown>,
): Record<string, unknown> | undefined {
  const raw = m.contributes;
  if (raw !== undefined && (typeof raw !== "object" || raw === null || Array.isArray(raw))) {
    return raw as Record<string, unknown>;
  }
  const base = (raw as Record<string, unknown> | undefined) ?? undefined;
  const legacy: Record<string, unknown> = {};
  const pick = (target: string, ...sources: string[]): void => {
    if (base && base[target] !== undefined) return;
    for (const s of sources) {
      const v = m[s];
      if (v === undefined) continue;
      const normalized = normalizeLegacy(target, v);
      if (normalized !== undefined) {
        legacy[target] = normalized;
        return;
      }
    }
  };
  pick("skills", "skills");
  pick("mcpServers", "mcpServers", "mcp");
  pick("agents", "agents");
  pick("hooks", "hooks");
  pick("apps", "apps");
  if (!base && Object.keys(legacy).length === 0) return undefined;
  return { ...(base ?? {}), ...legacy };
}

/** 旧形态归一：按类别形态规则转换；返回 undefined 表示「该形态不归入 contributes」。 */
function normalizeLegacy(key: string, value: unknown): unknown {
  switch (key) {
    case "skills":
      if (typeof value === "string") return [value];
      return Array.isArray(value) ? value : undefined;
    case "mcpServers":
      if (typeof value === "string") return { path: value };
      return Array.isArray(value) ? value : undefined;
    case "agents":
      if (typeof value === "string") return { path: value };
      return Array.isArray(value) ? value : undefined;
    case "hooks":
      return Array.isArray(value) ? value : undefined;
    case "apps":
      if (typeof value === "object" && value !== null && !Array.isArray(value)) return [value];
      return Array.isArray(value) ? value : undefined;
    default:
      return undefined;
  }
}
