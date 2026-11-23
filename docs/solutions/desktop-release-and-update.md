---
title: ava2 桌面端打包发布与自动更新（OSS 通道）
description: ava2 桌面壳的版本管理、签名打包、OSS 上传与客户端自动更新链路：publish.ps1 八步流程、ava2/ 前缀隔离布局、update.json 清单格式、Rust 侧 updater 集成与前端横幅交互。
keywords: ava2, 发布, publish.ps1, OSS, ossutil, 自动更新, tauri-plugin-updater, update.json, minisign, 版本管理
author: AI 助手
date: 2026-11-12
status: done
category: solutions
---

# ava2 桌面端打包发布与自动更新（OSS 通道）

## 1. 参考与差异

参考 `ava/ava-app/scripts/publish.ps1`（同名流程、同 OSS bucket、同为 NSIS 作更新包），差异在于：

| 维度 | ava | ava2 |
| --- | --- | --- |
| OSS 前缀 | `releases/<channel>/...`（根级） | **`ava2/releases/<channel>/...`**（独立前缀） |
| 冲突风险 | — | 两产品同 bucket，若共用根前缀，同版本号会互相覆盖；故 ava2 全部落在 `ava2/` 下 |
| 签名密钥 | `scripts/keys/ava-updater.key` | **独立密钥** `scripts/keys/ava2-updater.key`（客户端公钥不同，不可混用） |
| 默认 bundle | nsis（+可 msi） | `nsis,msi`（自动更新依赖 nsis） |

## 2. 版本管理

- **唯一版本源**：`ava2/package.json` 的 `version`。
- `scripts/version.mjs` 负责同步：`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（`[package] version`）、`src-tauri/Cargo.lock`（包 ava2 条目）。
- 命令：`npm run version:check`（校验一致）、`npm run version:set -- 0.3.0`、`node scripts/version.mjs --bump minor`。
- **App 版本独立**：`apps/<id>/package.json` 各自维护（如 chatapp 0.9.6），不经 version.mjs；下发靠 `POST /v1/apps/install`，与桌面壳版本解耦。
- 发布脚本支持 `-Version x.y.z` / `-Bump minor|major|patch`，会在构建前同步并复检。

## 3. 发布流程（`scripts/publish.ps1`，8 步）

1. **环境检查**：cargo / npm / node 可用。
2. **默认 App 打包**：`chatapp` / `file-explorer` / `file-viewer` 各自 `node build.mjs`，`dist/` 复制进 `src-tauri/resources/apps/<id>/`（随安装包分发，边车首次启动种子安装）。
3. **边车 release 构建**：`cargo build -p onion-server -p onion-sandbox --release`（沙箱缺件会让 workspace-write 降级为逐次审批）。
4. **拷贝边车**：`Copy-ExeTolerant` 先改名再复制——**运行中的 exe 无法覆盖，但可重命名**；这一步解决了「开发实例占着 resources 里的 onion-server.exe 导致发布失败」的真实故障。
5. **Tauri 构建（带签名）**：注入 `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`，`npm run tauri build -- --bundles nsis,msi`，产出 `*-setup.exe` + `.exe.sig`、`*.msi` + `.msi.sig`（Tauri v2 直接签原始产物，无 zip 包裹）。
6. **收集产物**：只收版本号严格匹配的文件（防止捞到历史产物），`.sig` 一并收集。
7. **生成元数据**：`update.json`（updater 清单）、`manifest.json`（版本/渠道/提交/产物哈希）、`sha256.txt`。
8. **上传 OSS**：版本目录下全部产物 → `ava2/releases/...`；`update.json` → `ava2/channels/...`。

开关：`-SkipBuild`（复用产物）、`-SkipUpload`（只本地打包）、`-DryRun`（打印上传命令）、`-Channel beta|dev`、`-Notes`。

## 4. OSS 布局与清单

```
oss://scalingagentlab/
├── ava2/releases/<channel>/windows-x86_64/<version>/
│     ava2_<version>_x64-setup.exe (+ .exe.sig)
│     ava2_<version>_x64_en-US.msi (+ .msi.sig)
│     manifest.json  sha256.txt
└── ava2/channels/<channel>/windows-x86_64/update.json      # 客户端检查点
```

客户端 endpoint（编译进 `tauri.conf.json` 的 `plugins.updater.endpoints`）：

```
https://scalingagentlab.oss-cn-beijing.aliyuncs.com/ava2/channels/stable/windows-x86_64/update.json
```

update.json（Tauri v2 格式）：

```json
{
  "version": "0.2.0",
  "notes": "...",
  "pub_date": "2026-11-12T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<*-setup.exe.sig 的内容>",
      "url": "https://.../ava2/releases/stable/windows-x86_64/0.2.0/ava2_0.2.0_x64-setup.exe"
    }
  }
}
```

无 NSIS 产物时脚本自动**回退用 MSI 作更新包**（Tauri v2 的 Windows updater 同时支持 NSIS 与 MSI），`platforms` 条目指向实际产物的 `.sig`；两者都缺失或未签名才告警并留空（此时 `msi` 仅供手动分发）。

> 实测：本机 GitHub 下载被网络阻断（`Downloading .../nsis-3.13.zip` 后挂起），NSIS 工具链无法获取，故 v0.2.0 采用 MSI 作为更新包。需要 NSIS 时的解法：给构建机配代理，或手动把 `nsis-3.13.zip` 放入 `%LOCALAPPDATA%\tauri` 对应缓存目录。

## 5. 客户端更新链路

**Rust**（`src-tauri/src/update.rs`，插件 `tauri-plugin-updater = 2.11.0`）：

| 能力 | 说明 |
| --- | --- |
| `spawn_check(app)` | 启动后静默检查；有新版则 `emit("ava2://update-available", UpdateInfo)`。离线/OSS 不可达只写 stderr，不打扰用户 |
| `check_update` 命令 | 前端主动查询，返回 `UpdateInfo \| null` |
| `install_update` 命令 | 下载 → 插件校验 minisign 签名 → **先 `sidecar.stop()`**（否则安装器无法覆盖运行中的 `onion-server.exe`）→ 安装 → `app.restart()` |

**前端**（`src/host/updater.ts` + `App.tsx` 横幅）：`useUpdateNotice()` 动态 `import("@tauri-apps/api/event")` 订阅事件；仅在 Tauri 内生效（`__TAURI_INTERNALS__` 守卫），浏览器模式永不出现。横幅显示「发现新版本 vX（当前 vY）」+ 立即更新 + 稍后再说。

**契约强耦合**：`tauri.conf.json` 的 `pubkey` 必须对应 `scripts/keys/ava2-updater.key`；endpoint 路径必须与 `publish.ps1` 的 OSS 路径一致；改任一侧都要重新发布，否则全量客户端更新失败。

## 6. 验收

- `cargo check --manifest-path src-tauri/Cargo.toml`：updater 插件接入编译通过。
- `npx tsc --noEmit`（ava2 根）：0 错。
- `ossutil ls oss://scalingagentlab/ava2/`：凭据有效、前缀为空（与 ava 无冲突）。
- 端到端（自动更新实际生效）需在已安装旧版本机器上验证：bump 版本 → 发布 → 旧客户端启动收到横幅 → 立即更新 → 重启后版本变化。

## 7. 已知边界与风险

- **凭据硬编码**：OSS AccessKey 与签名私钥密码当前硬编码在 `publish.ps1`（沿用 ava 当前阶段决策）。转生产前须改环境变量注入并轮换；私钥 `scripts/keys/` 已 gitignore。
- **密钥不可轮换**：公钥编进客户端，一旦有存量用户，更换私钥将使旧版无法校验新更新。ava2 尚无用户，此刻是唯一安全窗口。
- **NSIS 工具链**：首次打包需从 GitHub 下载 NSIS，网络受限时会失败；失败则只能出 msi（客户端无法自动更新）。
- **仅 Windows**：macOS/Linux 无法在 Windows 上交叉编译，需另建 runner 与对应 `platforms` 键。
- **无增量更新**：整包下载（NSIS ~数十 MB）。
