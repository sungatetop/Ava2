#!/usr/bin/env node
/**
 * ava2 版本管理（单一版本源 + 同步校验）。
 *
 * 版本源：`package.json` 的 `version`。以下文件必须与之保持一致：
 *   - `src-tauri/tauri.conf.json`（Tauri 打包/安装包版本）
 *   - `src-tauri/Cargo.toml` 的 `[package] version`（壳二进制版本）
 *   - `src-tauri/Cargo.lock` 中 `name = "ava2"` 条目的 `version`
 *
 * 用法：
 *   node scripts/version.mjs               # 校验一致性（同 --check）
 *   node scripts/version.mjs --check       # 校验一致性，不一致退出码 1
 *   node scripts/version.mjs --get         # 打印当前版本
 *   node scripts/version.mjs --set 0.2.0   # 写入指定版本到全部文件
 *   node scripts/version.mjs --bump patch  # patch | minor | major 自增后写入
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PKG = join(ROOT, "package.json");
const TAURI_CONF = join(ROOT, "src-tauri", "tauri.conf.json");
const TAURI_CARGO = join(ROOT, "src-tauri", "Cargo.toml");
const TAURI_LOCK = join(ROOT, "src-tauri", "Cargo.lock");

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** 读取 package.json 的版本（唯一版本源）。 */
function readSourceVersion() {
  const pkg = JSON.parse(readFileSync(PKG, "utf8"));
  if (!SEMVER.test(pkg.version ?? "")) {
    throw new Error(`package.json 的 version 不是合法 semver: ${pkg.version}`);
  }
  return pkg.version;
}

/** 从 tauri.conf.json 读取 version 字段。 */
function readTauriConfVersion() {
  const conf = JSON.parse(readFileSync(TAURI_CONF, "utf8"));
  return conf.version ?? "";
}

/** 从 src-tauri/Cargo.toml 的 [package] 段读取第一个 version。 */
function readCargoTomlVersion() {
  const text = readFileSync(TAURI_CARGO, "utf8");
  const m = text.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : "";
}

/** 从 src-tauri/Cargo.lock 的 ava2 条目读取 version。 */
function readCargoLockVersion() {
  const text = readFileSync(TAURI_LOCK, "utf8");
  const m = text.match(/^name = "ava2"\nversion = "([^"]+)"/m);
  return m ? m[1] : "";
}

/** 收集四处版本，用于校验与展示。 */
function collect() {
  return [
    { label: "package.json", version: readSourceVersion() },
    { label: "src-tauri/tauri.conf.json", version: readTauriConfVersion() },
    { label: "src-tauri/Cargo.toml", version: readCargoTomlVersion() },
    { label: "src-tauri/Cargo.lock (ava2)", version: readCargoLockVersion() },
  ];
}

/** 逐文件原地替换版本号，保留原有格式。 */
function writeVersion(version) {
  // package.json：顶层 version（首个匹配，位于文件开头）。
  const pkgText = readFileSync(PKG, "utf8");
  writeFileSync(PKG, pkgText.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`), "utf8");

  // tauri.conf.json：顶层 version（首个匹配）。
  const confText = readFileSync(TAURI_CONF, "utf8");
  writeFileSync(
    TAURI_CONF,
    confText.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`),
    "utf8",
  );

  // Cargo.toml：[package] 段的首个 version（文件内第一个 `version = "..."`）。
  const cargoText = readFileSync(TAURI_CARGO, "utf8");
  writeFileSync(
    TAURI_CARGO,
    cargoText.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`),
    "utf8",
  );

  // Cargo.lock：仅替换 ava2 条目，避免误改依赖版本。
  const lockText = readFileSync(TAURI_LOCK, "utf8");
  writeFileSync(
    TAURI_LOCK,
    lockText.replace(/^(name = "ava2"\nversion = ")[^"]+(")/m, `$1${version}$2`),
    "utf8",
  );
}

/** 按 bump 类型自增版本。 */
function bump(current, kind) {
  const [, major, minor, patch] = current.match(SEMVER);
  switch (kind) {
    case "major":
      return `${Number(major) + 1}.0.0`;
    case "minor":
      return `${major}.${Number(minor) + 1}.0`;
    case "patch":
      return `${major}.${minor}.${Number(patch) + 1}`;
    default:
      throw new Error(`未知的 bump 类型: ${kind}（可选 patch | minor | major）`);
  }
}

/** 校验四处版本一致，返回是否一致。 */
function check() {
  const rows = collect();
  const source = rows[0].version;
  const width = Math.max(...rows.map((r) => r.label.length));
  let ok = true;
  for (const row of rows) {
    const same = row.version === source;
    if (!same) ok = false;
    const mark = same ? "OK  " : "DIFF";
    console.log(`${mark} ${row.label.padEnd(width)}  ${row.version || "<缺失>"}`);
  }
  if (!ok) {
    console.error(`\n版本不一致：以 package.json(${source}) 为源，执行 npm run version:set -- <x.y.z> 同步`);
  }
  return ok;
}

const args = process.argv.slice(2);
const [command, operand] = args;

try {
  switch (command) {
    case undefined:
    case "--check": {
      process.exitCode = check() ? 0 : 1;
      break;
    }
    case "--get": {
      console.log(readSourceVersion());
      break;
    }
    case "--set": {
      if (!SEMVER.test(operand ?? "")) throw new Error(`--set 需要合法 semver，收到: ${operand}`);
      writeVersion(operand);
      console.log(`版本已写入 ${operand}`);
      check();
      break;
    }
    case "--bump": {
      const next = bump(readSourceVersion(), operand);
      writeVersion(next);
      console.log(`版本已自增 -> ${next}`);
      check();
      break;
    }
    default: {
      throw new Error(`未知参数: ${command}`);
    }
  }
} catch (err) {
  console.error(`错误: ${err.message}`);
  process.exitCode = 1;
}
