#!/usr/bin/env node
// App 契约校验（唯一 SPEC：contracts/app-contract.json）。
//
// 两段校验：
// 1) 防漂移：SPEC 的枚举 / 成员清单必须与实现单一来源逐项一致——
//    src/host/types.ts（类型）、src/host/manifest.ts 与
//    crates/onion-server/src/rest/apps.rs（hostApi 上限）；
// 2) 清单合规：apps/*/manifest.json 按 SPEC 的字段 / 枚举 / 路径 / 入口规则校验，
//    并校验 dist/manifest.json 与源码清单一致（构建型 App 的复制产物不得漂移）。
//
// 用法：node scripts/check-app-contract.mjs
// 退出码：0 = 全部通过（允许 warnings）；1 = 有 errors。
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SPEC_PATH = path.join(ROOT, "contracts", "app-contract.json");
const APPS_DIR = path.join(ROOT, "apps");

const errors = [];
const warnings = [];
const passes = [];

const fail = (scope, message) => errors.push(`[${scope}] ${message}`);
const warn = (scope, message) => warnings.push(`[${scope}] ${message}`);
const pass = (message) => passes.push(message);

// ---------------- 基础读取 ----------------

function readText(file) {
  return readFileSync(file, "utf8");
}

function readJson(file) {
  return JSON.parse(readText(file));
}

/** 稳定序列化（键递归排序），用于比较清单副本与源码清单。 */
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((k) => [k, canonical(value[k])]));
  }
  return value;
}

/** 提取 `export interface <name> { ... }` 的顶层成员名（2 空格缩进行）。 */
function interfaceMembers(source, name) {
  const block = new RegExp(`export interface ${name} \\{([\\s\\S]*?)\\n\\}`).exec(source);
  if (!block) return null;
  const members = [];
  for (const line of block[1].split("\n")) {
    const hit = /^ {2}(?:readonly )?"?([A-Za-z_][\w.]*)"?\??\s*[:(]/.exec(line);
    if (hit) members.push(hit[1]);
  }
  return members;
}

/** 提取 `export type <name> = "a" | "b"` 的字符串字面量成员。 */
function unionMembers(source, name) {
  const block = new RegExp(`export type ${name} =([\\s\\S]*?);`).exec(source);
  if (!block) return null;
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** 提取 `export const <name> ... = ["a", "b"]` 的字符串元素。 */
function constArray(source, name) {
  const block = new RegExp(`export const ${name}[^=]*=\\s*\\[([\\s\\S]*?)\\];`).exec(source);
  if (!block) return null;
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** 集合比较：缺失 / 多余分别报错，顺序无关。 */
function compareMembers(scope, label, actual, expected) {
  if (!actual) {
    fail(scope, `无法从实现中解析 ${label}（解析规则可能已过期）`);
    return false;
  }
  const a = new Set(actual);
  const e = new Set(expected);
  const missing = [...e].filter((x) => !a.has(x));
  const extra = [...a].filter((x) => !e.has(x));
  if (missing.length || extra.length) {
    if (missing.length) fail(scope, `${label} 在 SPEC 中缺失：${missing.join(", ")}`);
    if (extra.length) fail(scope, `${label} 在 SPEC 中多出：${extra.join(", ")}`);
    return false;
  }
  return true;
}

// ---------------- 1. SPEC ↔ 实现 防漂移 ----------------

const spec = readJson(SPEC_PATH);
const typesPath = path.join(ROOT, spec.types);
const typesSrc = readText(typesPath);
const manifestSrc = readText(path.join(ROOT, "src", "host", "manifest.ts"));
const appsRouteSrc = readText(path.join(ROOT, "crates", "onion-server", "src", "rest", "apps.rs"));

const driftErrors = errors.length;

compareMembers("types", "inject 枚举", unionMembers(typesSrc, "InjectKey"), spec.enums.inject);
compareMembers("types", "slot key 白名单（SLOT_KEYS）", constArray(typesSrc, "SLOT_KEYS"), spec.enums.slotKeys);
compareMembers("types", "SlotCatalog 键", interfaceMembers(typesSrc, "SlotCatalog"), spec.enums.slotKeys);
compareMembers("types", "hook 事件名", unionMembers(typesSrc, "HookEventName"), spec.enums.hookEvents);
compareMembers("types", "AppContext 能力面", interfaceMembers(typesSrc, "AppContext"), spec.ctx.surfaces);
compareMembers("types", "AgentChannel 方法", interfaceMembers(typesSrc, "AgentChannel"), spec.agentChannel.methods);
compareMembers(
  "types",
  "AppContributes 类别",
  interfaceMembers(typesSrc, "AppContributes"),
  Object.keys(spec.contributes.categories),
);
compareMembers(
  "types",
  "AppManifest 字段",
  interfaceMembers(typesSrc, "AppManifest"),
  spec.manifest.fields.map((f) => f.name),
);

// hostApi 上限三处一致：SPEC / 前端 manifest.ts / 边车 rest/apps.rs
const tsHostApi = Number(/export const HOST_API_SUPPORTED = (\d+);/.exec(manifestSrc)?.[1]);
const sidecarHostApi = Number(/const HOST_API_SUPPORTED: u64 = (\d+);/.exec(appsRouteSrc)?.[1]);
if (!Number.isInteger(tsHostApi)) fail("hostApi", "无法从 src/host/manifest.ts 解析 HOST_API_SUPPORTED");
if (!Number.isInteger(sidecarHostApi)) {
  fail("hostApi", "无法从 crates/onion-server/src/rest/apps.rs 解析 HOST_API_SUPPORTED");
}
for (const [where, value] of [
  ["SPEC", spec.hostApi.supported],
  ["src/host/manifest.ts", tsHostApi],
  ["crates/onion-server/src/rest/apps.rs", sidecarHostApi],
]) {
  if (Number.isInteger(value) && value !== spec.hostApi.supported) {
    fail("hostApi", `${where} 的 hostApi 上限为 ${value}，与 SPEC 的 ${spec.hostApi.supported} 不一致`);
  }
}
if (errors.length === driftErrors) pass(`SPEC ↔ 实现一致（${spec.types} / manifest.ts / rest/apps.rs）`);

// ---------------- 2. apps/*/manifest.json 合规 ----------------

const KNOWN_FIELDS = new Set(spec.manifest.fields.map((f) => f.name));

/** 相对路径必须落在 App 根目录内，返回绝对路径；非法返回 undefined。 */
function resolveInside(dir, rel) {
  if (typeof rel !== "string" || rel.trim() === "") return undefined;
  const normalized = rel.replace(/\\/g, "/");
  if (normalized.startsWith("/") || /^[A-Za-z]:/.test(normalized) || normalized.split("/").includes("..")) {
    return undefined;
  }
  return path.join(dir, normalized);
}

/** 入口解析：源码目录或构建产物目录（dist/）。 */
function resolveEntry(dir, entry) {
  const inside = resolveInside(dir, entry);
  if (!inside) return { ok: false, reason: "入口必须是 App 根目录内的相对路径" };
  if (existsSync(inside) && statSync(inside).isFile()) return { ok: true, file: inside };
  const inDist = path.join(dir, "dist", entry.replace(/\\/g, "/"));
  if (existsSync(inDist) && statSync(inDist).isFile()) return { ok: true, file: inDist };
  return { ok: false, reason: `入口文件 ${entry} 不存在（构建型 App 请先构建，产物应在 dist/ 内）` };
}

function checkConfig(scope, config) {
  if (config.type !== "object") fail(scope, "config.type 必须是 object");
  if (typeof config.properties !== "object" || config.properties === null) {
    fail(scope, "config.properties 必须是对象");
    return;
  }
  for (const [key, prop] of Object.entries(config.properties)) {
    const field = `config.properties.${key}`;
    if (!prop || typeof prop !== "object") {
      fail(scope, `${field} 必须是对象`);
      continue;
    }
    if (!["string", "number", "boolean"].includes(prop.type)) {
      fail(scope, `${field}.type 必须是 string/number/boolean（当前 ${JSON.stringify(prop.type)}）`);
    }
    if (prop.enum !== undefined && (!Array.isArray(prop.enum) || prop.enum.length === 0)) {
      fail(scope, `${field}.enum 必须是非空数组`);
    }
    if (prop.default !== undefined && prop.type !== undefined && typeof prop.default !== prop.type) {
      fail(scope, `${field}.default 类型必须是 ${prop.type}`);
    }
  }
}

function checkContributes(scope, dir, contributes) {
  const categories = spec.contributes.categories;
  for (const key of Object.keys(contributes)) {
    if (!(key in categories)) {
      fail(scope, `contributes.${key} 不是合法类别（合法值 ${Object.keys(categories).join(" / ")}）`);
    }
  }
  // apps：UI 页
  if (contributes.apps !== undefined) {
    if (!Array.isArray(contributes.apps)) {
      fail(scope, "contributes.apps 必须是数组");
    } else {
      const ids = new Set();
      for (const [index, page] of contributes.apps.entries()) {
        const pageScope = `contributes.apps[${index}]`;
        if (!page || typeof page !== "object") {
          fail(scope, `${pageScope} 必须是对象`);
          continue;
        }
        if (typeof page.id !== "string" || page.id === "") fail(scope, `${pageScope}.id 必填`);
        else if (ids.has(page.id)) fail(scope, `${pageScope}.id「${page.id}」在同一 App 内重复`);
        else ids.add(page.id);
        if (typeof page.entry !== "string" || page.entry === "") fail(scope, `${pageScope}.entry 必填`);
        else {
          const resolved = resolveEntry(dir, page.entry);
          if (!resolved.ok) fail(scope, `${pageScope}.entry ${resolved.reason}`);
        }
        if (page.hostApi !== undefined && !(Number.isInteger(page.hostApi) && page.hostApi <= spec.hostApi.supported)) {
          fail(scope, `${pageScope}.hostApi 必须是不超过 ${spec.hostApi.supported} 的整数`);
        }
        if (page.inject !== undefined) {
          const bad = page.inject.filter((k) => !spec.enums.inject.includes(k));
          if (!Array.isArray(page.inject) || bad.length) {
            fail(scope, `${pageScope}.inject 含未知能力面：${bad.join(", ") || "（非数组）"}`);
          }
        }
        if (page.surfaces !== undefined) {
          const bad = page.surfaces.filter((k) => !spec.enums.slotKeys.includes(k));
          if (!Array.isArray(page.surfaces) || bad.length) {
            fail(scope, `${pageScope}.surfaces 含未知 slot key：${bad.join(", ") || "（非数组）"}`);
          }
        }
      }
    }
  }
  // skills / agents：路径引用
  for (const key of ["skills", "agents"]) {
    const value = contributes[key];
    if (value === undefined) continue;
    if (!Array.isArray(value)) {
      fail(scope, `contributes.${key} 必须是数组`);
      continue;
    }
    for (const [index, item] of value.entries()) {
      const rel = typeof item === "string" ? item : item?.path;
      const inside = resolveInside(dir, rel);
      if (!inside) {
        fail(scope, `contributes.${key}[${index}] 路径必须在 App 根目录内（当前 ${JSON.stringify(item)}）`);
        continue;
      }
      if (!existsSync(inside)) {
        fail(scope, `contributes.${key}[${index}] 路径不存在：${rel}`);
        continue;
      }
      if (key === "skills" && statSync(inside).isDirectory() && !hasSkillFile(inside)) {
        warn(scope, `contributes.skills[${index}] 目录 ${rel} 下没有 SKILL.md`);
      }
    }
  }
  // mcpServers：内联数组或 { path }
  const mcp = contributes.mcpServers;
  if (mcp !== undefined) {
    if (!Array.isArray(mcp) && typeof mcp !== "object") {
      fail(scope, "contributes.mcpServers 必须是数组或 { path }");
    } else if (!Array.isArray(mcp)) {
      const inside = resolveInside(dir, mcp.path);
      if (!inside || !existsSync(inside)) fail(scope, `contributes.mcpServers.path 不存在或越界：${mcp.path}`);
    } else {
      mcp.forEach((item, index) => {
        if (typeof item?.name !== "string" || item.name === "") {
          fail(scope, `contributes.mcpServers[${index}].name 必填`);
        }
        if (typeof item?.command !== "string" && typeof item?.url !== "string") {
          fail(scope, `contributes.mcpServers[${index}] 需要 command（stdio）或 url（http）之一`);
        }
      });
    }
  }
  // hooks：内联数组或 { path }
  const hooks = contributes.hooks;
  if (hooks !== undefined) {
    if (!Array.isArray(hooks) && typeof hooks !== "object") {
      fail(scope, "contributes.hooks 必须是数组或 { path }");
    } else if (!Array.isArray(hooks)) {
      const inside = resolveInside(dir, hooks.path);
      if (!inside || !existsSync(inside)) {
        fail(scope, `contributes.hooks.path 不存在或越界：${hooks.path}`);
      } else {
        checkHooksFile(scope, inside);
      }
    } else {
      hooks.forEach((item, index) => {
        if (!spec.enums.hookEvents.includes(item?.event)) {
          fail(scope, `contributes.hooks[${index}].event 非法：${JSON.stringify(item?.event)}`);
        }
        const type = item?.handler?.type;
        if (type === "command") {
          if (typeof item.handler.command !== "string" || item.handler.command === "") {
            fail(scope, `contributes.hooks[${index}].handler.command 必填`);
          }
        } else if (type === "bus") {
          warn(scope, `contributes.hooks[${index}] 为 bus 处理器，边车装配期会跳过`);
        } else {
          fail(scope, `contributes.hooks[${index}].handler.type 必须是 command 或 bus`);
        }
      });
    }
  }
}

/** hooks 配置文件（Claude Code 兼容格式）：`{ hooks: { Event: [...] } }` 或裸事件 map。
 *  每个事件名必须 ∈ SPEC 枚举；组内 hooks[].type 必须是 command（command 非空）或 bus（边车跳过）。 */
function checkHooksFile(scope, file) {
  const name = path.basename(file);
  let raw;
  try {
    raw = JSON.parse(readText(file));
  } catch (error) {
    fail(scope, `hooks 文件 ${name} 不是合法 JSON：${error.message}`);
    return;
  }
  const events =
    raw && typeof raw === "object" && !Array.isArray(raw) ? (raw.hooks !== undefined ? raw.hooks : raw) : null;
  if (!events || typeof events !== "object" || Array.isArray(events)) {
    fail(scope, `hooks 文件 ${name} 顶层必须是事件 map 或 { hooks: { Event: [...] } }`);
    return;
  }
  for (const [event, groups] of Object.entries(events)) {
    if (!spec.enums.hookEvents.includes(event)) {
      fail(scope, `hooks 文件 ${name} 事件名非法：${event}（合法值 ${spec.enums.hookEvents.join(" / ")}）`);
      continue;
    }
    if (!Array.isArray(groups)) {
      fail(scope, `hooks 文件 ${name}.${event} 必须是 matcher 组数组`);
      continue;
    }
    groups.forEach((group, gi) => {
      const entries = group?.hooks;
      if (!Array.isArray(entries)) {
        fail(scope, `hooks 文件 ${name}.${event}[${gi}].hooks 必须是数组`);
        return;
      }
      entries.forEach((hook, hi) => {
        const type = hook?.type;
        if (type === "command") {
          if (typeof hook.command !== "string" || hook.command === "") {
            fail(scope, `hooks 文件 ${name}.${event}[${gi}].hooks[${hi}].command 必填`);
          }
        } else if (type === "bus") {
          warn(scope, `hooks 文件 ${name}.${event}[${gi}].hooks[${hi}] 为 bus 处理器，边车装配期会跳过`);
        } else {
          fail(scope, `hooks 文件 ${name}.${event}[${gi}].hooks[${hi}].type 必须是 command 或 bus`);
        }
      });
    });
  }
}

function hasSkillFile(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory() && hasSkillFile(path.join(dir, entry.name))) return true;
    if (entry.isFile() && entry.name === "SKILL.md") return true;
  }
  return false;
}

function checkManifest(dirName, dir, label) {
  const scope = label;
  const manifest = readJson(path.join(dir, "manifest.json"));
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    fail(scope, "manifest 顶层必须是对象");
    return;
  }
  for (const key of Object.keys(manifest)) {
    if (!KNOWN_FIELDS.has(key)) warn(scope, `未知字段 ${key}（宿主会忽略，确认不是拼写错误）`);
  }
  for (const field of spec.manifest.fields) {
    const value = manifest[field.name];
    if (value === undefined) {
      if (field.required) fail(scope, `缺少必填字段 ${field.name}`);
      continue;
    }
    const typeOk = {
      string: () => typeof value === "string",
      integer: () => Number.isInteger(value),
      boolean: () => typeof value === "boolean",
      "string[]": () => Array.isArray(value) && value.every((v) => typeof v === "string"),
      ConfigSchema: () => typeof value === "object" && value !== null && !Array.isArray(value),
      AppContributes: () => typeof value === "object" && value !== null && !Array.isArray(value),
      // AppIcon：string（SVG path d）或 { d: string | string[]; fill?: boolean }
      AppIcon: () =>
        typeof value === "string" ||
        (typeof value === "object" &&
          value !== null &&
          !Array.isArray(value) &&
          (typeof value.d === "string" || (Array.isArray(value.d) && value.d.every((v) => typeof v === "string")))),
      // NetAuth：联网服务授权 { allow?: string[] }（裸域名清单）
      NetAuth: () =>
        typeof value === "object" &&
        value !== null &&
        !Array.isArray(value) &&
        (value.allow === undefined ||
          (Array.isArray(value.allow) && value.allow.every((v) => typeof v === "string" && v.trim() !== ""))),
    }[field.type]?.();
    if (!typeOk) {
      fail(scope, `${field.name} 类型必须是 ${field.type}（当前 ${JSON.stringify(value)}）`);
      continue;
    }
    if (field.pattern && !new RegExp(field.pattern).test(value)) {
      fail(scope, `${field.name} 不匹配 ${field.pattern}（当前 ${JSON.stringify(value)}）`);
    }
    if (field.minLength !== undefined && value.length < field.minLength) {
      fail(scope, `${field.name} 不能为空`);
    }
    if (field.minimum !== undefined && value < field.minimum) fail(scope, `${field.name} 不得小于 ${field.minimum}`);
    if (field.maximum !== undefined && value > field.maximum) fail(scope, `${field.name} 不得大于 ${field.maximum}`);
    if (field.enum && Array.isArray(value)) {
      const bad = value.filter((v) => !spec.enums[field.enum].includes(v));
      if (bad.length) fail(scope, `${field.name} 含未知取值：${bad.join(", ")}`);
    } else if (field.enum && !spec.enums[field.enum].includes(value)) {
      fail(scope, `${field.name} 只能是 ${spec.enums[field.enum].join(" / ")}`);
    }
    if (field.name === "config" && value !== undefined) checkConfig(scope, value);
    if (field.name === "contributes" && value !== undefined) checkContributes(scope, dir, value);
  }
  if (manifest.id !== dirName) {
    fail(scope, `manifest.id（${manifest.id}）必须与目录名（${dirName}）一致`);
  }
  // 入口：顶层 entry 或 contributes.apps[].entry 至少一个
  const pageEntries = Array.isArray(manifest.contributes?.apps)
    ? manifest.contributes.apps.map((p) => p?.entry).filter((e) => typeof e === "string" && e !== "")
    : [];
  if (typeof manifest.entry === "string" && manifest.entry !== "") {
    const resolved = resolveEntry(dir, manifest.entry);
    if (!resolved.ok) fail(scope, `entry：${resolved.reason}`);
  } else if (pageEntries.length === 0) {
    fail(scope, "需声明 entry 或 contributes.apps[].entry（边车安装期会拒绝 app_entry_missing）");
  }
  // 构建型 App 的产物清单必须与源码清单一致
  const distManifest = path.join(dir, "dist", "manifest.json");
  if (existsSync(distManifest)) {
    const sourceCanonical = JSON.stringify(canonical(manifest));
    const distCanonical = JSON.stringify(canonical(readJson(distManifest)));
    if (sourceCanonical !== distCanonical) {
      fail(scope, "dist/manifest.json 与源码 manifest.json 不一致（重新构建以同步产物清单）");
    }
  }
}

const appDirs = existsSync(APPS_DIR)
  ? readdirSync(APPS_DIR, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && existsSync(path.join(APPS_DIR, entry.name, "manifest.json")))
      .map((entry) => entry.name)
      .sort()
  : [];

for (const id of appDirs) {
  const before = errors.length;
  const dir = path.join(APPS_DIR, id);
  try {
    checkManifest(id, dir, `apps/${id}`);
  } catch (error) {
    fail(`apps/${id}`, `读取 / 解析失败：${error.message}`);
  }
  if (errors.length === before) pass(`apps/${id}/manifest.json 合规`);
}

// ---------------- 结果 ----------------

console.log(`App 契约校验 · spec ${spec.specVersion}（hostApi ${spec.hostApi.supported}）`);
console.log(`SPEC：contracts/app-contract.json`);
for (const line of passes) console.log(`  ✓ ${line}`);
for (const line of warnings) console.log(`  ! ${line}`);
for (const line of errors) console.log(`  ✗ ${line}`);
console.log(`\n结果：${errors.length} error / ${warnings.length} warning（覆盖 ${appDirs.length} 个 App）`);
process.exit(errors.length ? 1 : 0);
