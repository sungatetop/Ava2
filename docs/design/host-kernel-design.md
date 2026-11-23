---
title: 宿主内核设计
description: ava2 AppHost 内核（src/host/）的模块职责、数据流与不变量：清单解析、slot 注册表、事件总线、Agent 通道、装配生命周期与 React 绑定层。
keywords: AppHost, 内核, registry, EventBus, AgentChannel, lazy-cache, 生命周期, 不变量
author: AI 助手
date: 2026-11-08
updated: 2026-11-08
status: done
category: design
---

# 宿主内核设计

`src/host/` 是 AppHost 的内核层，**框架无关**（React 绑定独立在 `src/host/AppHost.tsx`）。

## 1. 模块地图

```
src/host/
├── types.ts        # 契约类型（L0 manifest / L1 slot 目录 / L3 AppContext / L4 AgentChannel）+ 白名单常量
├── manifest.ts     # 清单解析：parseManifest（校验+归一）、INJECT_KEYS、validateConfigValues
├── config.ts       # 声明式配置读取（readRawConfig）
├── env.ts          # 双端基址解析（桌面 invoke backend_base → ?backend= → 默认 9800）
├── registry.ts     # 内核：EventBus + slot 注册表 + App 状态机 + AppContext 工厂
├── loader.ts       # 发现层：边车仓库 → 健康检查 → 装配（boot 唯一入口）
├── agent.ts        # L4 Agent 通道平台服务单例（REST + 每会话一条 WS 多路复用）
├── apps.ts         # App 仓库客户端（install/uninstall/enabled/静态产物 URL）
├── lazy-cache.ts   # React.lazy 实例缓存（appId::viewId 键控）
└── AppHost.tsx     # React 绑定：useSyncExternalStore 订阅版本号 + 惰性 surface + ErrorBoundary + App 管理面板
```

分层依赖单向：`AppHost.tsx → loader/registry → agent/apps/manifest → types`；`types.ts` 是唯一公共类型出口。

## 2. 核心数据结构（registry.ts）

- **slot 表**：`Map<SlotKey, Map<appId, Spec[]>>`——同一 App 对同一 key 可注入多条（多面板场景）。
- **认领表**：`Map<SlotKey, Map<viewId, Set<appId>>>`——认领型 slot（`surface.main` / `surface.canvas`）的 viewId 即路由键，**同一舞台内全局唯一**；同一 App 对同 viewId 注入多次时，最后一个撤回才释放认领。
- **App 记录**：`{ manifest, status, error?, disposers[], module?, loaded }`。`loaded` 区分「装配过」（纯能力 App 无 UI 页但 setup 已跑）与 `status`。
- **版本号**：`version` 单调递增，`subscribe/getVersion` 暴露给 `useSyncExternalStore`；任何 mutation（注入/撤回/状态变更/bus 监听计数变化）都 `touch()`。

### 状态机

```
registerSuspended ──→ suspended ──(load)──→ loading ──成功──→ active
                         ↑                     │
                         └────(setEnabled)─────┴──失败──→ failed（error 落档，uninjectAll 已执行）
active ──(setStatus suspended)──→ suspended（模块保留，零成本恢复）
任意态 ──(unload)──→ 注销（排空 disposers → uninjectAll → 删除记录）
```

- **可见性规则**：`collect()` 只返回 `active` App 的贡献；未注册 appId = 宿主自身贡献（始终可见）。suspended/failed 自动不可见但模块保留。
- **fail-loud**：清单非法、越权 slot key、viewId 认领冲突、配置非法、setup 抛错 → 一律 `failed` 并保留错误信息，不影响其他 App。

## 3. 事件总线（EventBus）

| API | 语义 |
| --- | --- |
| `on(type, fn)` | 宿主级订阅（壳与宿主模块）：接收全部事件 |
| `onApp(appId, type, fn)` | App 级订阅：广播事件 + 定向到本 App（载荷已剥离 `appId`） |
| `emit(type, e)` | 宿主广播：全体可见 |
| `emitDirected(type, {appId,...})` | 宿主定向投递：App 级监听者收到剥离 `appId` 的副本 |
| `emitFromApp(type, e)` | App 发射：`HOST_ONLY_EVENTS`（如 `agent.hookResult`）只回宿主，其余广播 |

不变量：App 间不互相直传事件载荷（定向事件宿主中转）；`listenerCount` 暴露给 App 管理面板做泄漏观测；App 卸载时 `ctx.events.on` 登记的订阅随 `disposers` 统一排空。

## 4. AppContext 工厂（makeContext）

`setup(ctx)` 拿到的唯一 API 面。要点：

- **能力面收窄**：`hostApi ≥ 1` 时按 `inject`（页级覆盖插件级）构造 `granted` 集合，五个面（slots/events/agent/storage/settings）经 getter 惰性校验——**未声明即抛错**（fail-loud，不静默返回空实现）。`hostApi 0` 全量注入（旧契约兼容）。
- **storage 命名空间**：`localStorage` 键 `app:<id>:`，JSON 序列化，契约统一 async。
- **settings 只读快照**：值来自 `validateConfigValues`（非法配置在 load 前已阻止），App 不可写。
- **agent 单例**：所有 App 共享同一 `getAgent()` 实例（同一组 WS 连接与 REST 客户端）。

## 5. Agent 通道（agent.ts）

- **每会话一条 WS**：`connFor(sessionId)` 首个订阅者建连，最后一个退订者关闭；`onclose` 清理连接表。帧解析失败（非 JSON）静默忽略。
- **REST 收敛**：全部端点封装为 `AgentChannel` 方法；非 2xx 统一 `Error("HTTP <status>: <detail>")`（fail-loud，App 层直接展示）。`deleteSession` 对 404 幂等放行。
- **发送异步化**：`send` 固定 `wait:false`（202 受理），轮次进度经 WS 推送——App 不做长轮询。
- **基址校准**：`base` 由 loader 在 boot 时经 `setAgentBase(env.resolveBackendBase())` 校准；App 仓库（apps.ts）与通道共用。

## 6. 装配生命周期（loader.ts + registry.load）

```
discoverAndBoot()
  1. resolveBackendBase()          # 桌面 invoke → ?backend= → 默认 9800（进程内缓存一次）
  2. setAgentBase(base)            # 通道与仓库共用
  3. probeBackend(/v1/health)      # 只作展示（标题栏状态点），失败不阻断装配
  4. fetchInstalledManifests()     # GET /v1/apps → parseManifest 校验（非法者登记 failed+disabled）
  5. boot：enabled 并行 load()；disabled registerSuspended() 占位
  6. registry.poke()               # bootInfo 是注册表外快照，手动驱动 UI
```

`load(manifest)` 的七步：占位 loading → `readRawConfig` → `validateConfigValues`（非法阻止加载）→ `resolvePages`（`contributes.apps` 优先，缺省顶层 entry 单页，pageId="."）→ 逐页 `import(url?v=version)` → 校验 `setup` 导出 → `setup(makeContext(...))` 收集 Disposable → active。

热重载路径：`unload → clearLazyCache(appId) → load`（调用方负责清 lazy 缓存，保证 import 新模块）。

## 7. React 绑定层（AppHost.tsx + lazy-cache）

- **订阅**：组件用 `useSyncExternalStore(registry.subscribe, registry.getVersion)` 订阅版本号，注册表任何变化触发重渲染。
- **惰性渲染**：`MainSurface` / `CanvasSurface` / `SidebarPanels` / `CanvasPanels` 同构——按 viewId 取贡献 spec，经 lazy-cache 取/建 React.lazy 实例。
- **lazy-cache 不变量**：键 `appId::viewId`（画布视图额外前缀 `canvas:`、面板 `canvaspanel:`、侧栏面板 `:sidebar`）。**React.lazy 工厂每次渲染重建会反复 import 并重置组件状态，必须稳定持有**；`clearLazyCache(appId)` 在 unload/热重载后清除。
- **错误隔离**：每份贡献独立 `ErrorBoundary`，key 含 `lazyId(Comp)` 实例 id——热重载换新模块时边界 remount、error 自动重置；一个 App 崩溃不影响其他 App。
- **画布驻留**：画布 tab 关闭不卸载（壳层仅收宽度），lazy 实例驻留 lazy-cache；App 卸载后贡献消失，tab 渲染期过滤自动隐去。

## 8. 契约裁剪说明

| 差异 | 原因 |
| --- | --- |
| 无 PluginSource / backend plugin 通道 | onion-server 无 plugin 概念，第一版纯静态预置 App |
| AgentChannel 重写为 onion 端点 | 会话/messages/WS/pending 回填 + 能力枚举，事件帧原样透传 |
| 无 workspace 注册表 / 附件上传 / 文件搜索 / 审批档位 / workflow-state | onion 侧未提供对应能力，契约位预留 |
| contributes 仅装配 `apps`（UI 多页） | 技能/MCP/agents/hooks 由边车自持，宿主不重复装配 |
| storage / 启停态落 localStorage + 边车登记表 | 无 backend plugin 存储面 |

## 9. 已知边界

- `panel.canvas` / `panel.sidebar` 的 viewId 是**引用**，壳不做「指向的视图不存在」的强校验（面板自然不渲染）；如需严格化，可在 inject 时校验认领表。
- 总线事件名是开放字符串空间，`canvas.open` 等为约定事件；后续若加事件建议在 types.ts 集中声明常量。
- 多页面 plugin 的页级 `inject/surfaces/config` 覆盖已支持，但宿主设置页（config 表单渲染）尚未实现。
