#!/usr/bin/env node
// ava2 App 契约级本地校验（自包含，零依赖，不依赖 ava2 源码仓库）。
// 用法：node validate-manifest.mjs <app-dir>
// 校验规则对齐 app-contract SPEC：manifest 字段 / 枚举 / 入口 / contributes 路径。
// 退出码：0 = 合规；1 = 存在错误。错误码与服务端安装期一致（app_*）。

import fs from "node:fs";
import path from "node:path";

const dir = process.argv[2];
if (!dir) {
  console.error("用法：node validate-manifest.mjs <app-dir>");
  process.exit(1);
}
const root = path.resolve(dir);

/** @type {string[]} */
const errors = [];
/** @type {string[]} */
const warnings = [];
const err = (code, msg) => errors.push(`[${code}] ${msg}`);
const warn = (msg) => warnings.push(msg);

/** 路径是否落在 App 根目录内（拒绝绝对路径与 ..）。返回解析后的绝对路径或 null。 */
function withinRoot(rel) {
  if (typeof rel !== "string" || rel.length === 0) return null;
  if (path.isAbsolute(rel) || rel.includes("..")) return null;
  const resolved = path.resolve(root, rel);
  return resolved.startsWith(root + path.sep) || resolved === root ? resolved : null;
}

// ---- manifest 存在与形态 ----
const manifestPath = path.join(root, "manifest.json");
if (!fs.existsSync(manifestPath)) {
  err("app_manifest_missing", `${manifestPath} 不存在`);
  report();
}
let m;
try {
  m = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
} catch (e) {
  err("app_manifest_invalid", `JSON 解析失败：${e.message}`);
  report();
}
if (typeof m !== "object" || m === null || Array.isArray(m)) {
  err("app_manifest_invalid", "manifest.json 顶层必须是 JSON 对象");
  report();
}

// ---- id / name / version / hostApi ----
if (!m.id) err("app_id_missing", "id 缺失");
else if (typeof m.id !== "string" || !/^[A-Za-z0-9._-]{1,64}$/.test(m.id))
  err("app_id_invalid", `id "${m.id}" 不合法（规则 ^[A-Za-z0-9._-]{1,64}$）`);
if (!m.name || typeof m.name !== "string") err("app_name_missing", "name 缺失或为空");
if (typeof m.version !== "string" || m.version.length === 0)
  warn("version 缺失或非法——运行副本按 ?v=<version> 做 ESM 缓存指纹，务必声明 semver");
if (m.hostApi !== 2) err("app_host_api_unsupported", `hostApi 必须 = 2，当前 ${JSON.stringify(m.hostApi)}`);

// ---- 入口 ----
const pages = Array.isArray(m.contributes?.apps) ? m.contributes.apps : [];
const entries = [];
if (typeof m.entry === "string") entries.push({ where: "entry", value: m.entry });
for (const p of pages) if (typeof p?.entry === "string") entries.push({ where: `contributes.apps[${p.id ?? "?"}].entry`, value: p.entry });
if (entries.length === 0) {
  err("app_entry_missing", "顶层 entry 与 contributes.apps[].entry 至少声明一个（纯能力 App 也需空 setup 入口）");
} else {
  for (const e of entries) {
    const resolved = withinRoot(e.value);
    if (!resolved) err("app_entry_invalid", `${e.where}: "${e.value}" 必须是 App 根目录内的相对路径（拒绝绝对路径与 ..）`);
    else if (!fs.existsSync(resolved)) err("app_entry_not_found", `${e.where}: "${e.value}" 指向的文件不存在`);
  }
}

// ---- inject / surfaces 枚举 ----
const INJECT = ["slots", "events", "agent", "storage"];
const SLOT_KEYS = ["surface.main", "surface.canvas", "nav.sidebar", "panel.sidebar", "panel.canvas", "user.menu"];
if (m.inject !== undefined) {
  if (!Array.isArray(m.inject) || m.inject.some((x) => !INJECT.includes(x)))
    err("app_manifest_invalid", `inject 必须是 ${JSON.stringify(INJECT)} 的子集`);
}
if (m.surfaces !== undefined) {
  if (!Array.isArray(m.surfaces) || m.surfaces.some((x) => !SLOT_KEYS.includes(x)))
    err("app_manifest_invalid", `surfaces 必须是 ${JSON.stringify(SLOT_KEYS)} 的子集`);
}

// ---- 展示字段 ----
for (const f of ["description", "author", "homepage", "license"]) {
  if (m[f] !== undefined && typeof m[f] !== "string") err("app_manifest_invalid", `${f} 必须是字符串`);
}
if (m.icon !== undefined) {
  const ok = typeof m.icon === "string" || (typeof m.icon === "object" && typeof m.icon?.d === "string");
  if (!ok) err("app_manifest_invalid", "icon 必须是 SVG path 字符串或 { d, fill? } 对象");
}

// ---- contributes 其它类别 ----
const contrib = m.contributes ?? {};

if (contrib.skills !== undefined) {
  const skills = Array.isArray(contrib.skills) ? contrib.skills : [contrib.skills];
  for (const s of skills) {
    const spec = typeof s === "string" ? s : s?.path;
    const resolved = spec ? withinRoot(spec) : null;
    if (!resolved) err("app_contributes_invalid", `skills "${spec}" 必须是 App 根目录内的相对路径`);
    else if (!fs.existsSync(resolved)) err("app_contributes_invalid", `skills "${spec}" 指向的路径不存在`);
  }
}

if (contrib.mcpServers !== undefined) {
  if (typeof contrib.mcpServers === "object" && !Array.isArray(contrib.mcpServers) && contrib.mcpServers.path) {
    const resolved = withinRoot(contrib.mcpServers.path);
    if (!resolved || !fs.existsSync(resolved)) err("app_contributes_invalid", `mcpServers.path "${contrib.mcpServers.path}" 必须落在 App 根内且存在`);
  } else if (Array.isArray(contrib.mcpServers)) {
    for (const s of contrib.mcpServers) {
      if (!s?.name) err("app_contributes_invalid", "mcpServers[] 每项必须有 name");
      else if (!s.command && !s.url) err("app_contributes_invalid", `mcpServers[${s.name}] 必须声明 command（stdio）或 url（streamable-http）其一`);
    }
  } else {
    err("app_contributes_invalid", "mcpServers 必须是数组或 { path }");
  }
}

if (contrib.hooks !== undefined) {
  if (typeof contrib.hooks === "object" && !Array.isArray(contrib.hooks) && contrib.hooks.path) {
    const resolved = withinRoot(contrib.hooks.path);
    if (!resolved || !fs.existsSync(resolved)) err("app_contributes_invalid", `hooks.path "${contrib.hooks.path}" 必须落在 App 根内且存在`);
  } else if (Array.isArray(contrib.hooks)) {
    for (const h of contrib.hooks) {
      if (!h?.event) err("app_contributes_invalid", "hooks[] 每项必须有 event");
      else if (!h?.handler) err("app_contributes_invalid", `hooks[${h.event}] 缺 handler（边车只执行 command 处理器）`);
    }
  } else {
    err("app_contributes_invalid", "hooks 必须是数组或 { path }");
  }
}

// ---- 多页面 id 唯一 ----
const pageIds = pages.map((p) => p?.id).filter(Boolean);
if (pages.length && pageIds.length !== pages.length) err("app_contributes_invalid", "contributes.apps[] 每页必须有 id");
const dup = pageIds.filter((id, i) => pageIds.indexOf(id) !== i);
if (dup.length) err("app_contributes_invalid", `contributes.apps 页 id 重复：${[...new Set(dup)].join(", ")}`);

// ---- 报告 ----
function report() {
  for (const line of errors) console.error(`✗ ${line}`);
  for (const line of warnings) console.warn(`⚠ ${line}`);
  console.error(errors.length ? `结果：${errors.length} error / ${warnings.length} warning——${root}` : `✓ 合规：${root}${warnings.length ? `（${warnings.length} warning）` : ""}`);
  process.exit(errors.length ? 1 : 0);
}
report();
