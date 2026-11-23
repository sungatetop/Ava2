---
title: AppHost 统一联网服务（net 能力面）设计
description: 平台级外联服务的设计：App 经 ctx.net.fetch 受控访问外部网络，双层安全（服务端安全底线 + host 层 App 级域白名单），与 slots/agent/storage 能力面同构。
keywords: AppHost, 联网服务, net, 外联, CORS, SSRF, 能力面, manifest, 契约
author: AI 助手
date: 2026-11-22
updated: 2026-11-22
status: done
category: design
---

# AppHost 统一联网服务（net 能力面）设计

## 1. 背景与动机

AppHost 生态中 App（web-ui 浏览器环境 / Tauri webview）需要访问外部网络（如
技能广场直连腾讯 SkillHub）。两条硬约束决定了平台必须提供统一外联服务：

1. **浏览器 CORS 白名单不可控**：实测第三方公开 API（如 `api.skillhub.cn`）只
   回显自家域名 origin，App 内 `fetch` 第三方站点在 web-ui 与 Tauri webview 中
   均被浏览器拦截；
2. **App 无文件系统 / 注册表**：涉及落盘、内存态刷新的后续动作只能在服务端，
   外联能力必须与平台服务链路衔接。

初期实现（`AgentChannel.fetchExternal(url)`）验证了链路，但暴露三个缺口：
外联语义错挂在「会话/Agent」通道上；任何声明 `agent` 的 App 自动获得外联
（无独立授权）；仅 GET / 纯文本 / 无 header。本设计将其正式化为独立能力面。

## 2. 架构总览

```
App 代码
  └─ ctx.net.fetch({ url, method?, headers?, body?, encoding? })     ← 唯一外联出口
       │
       ├─ host 层权限（registry.makeContext，per-App）
       │    ① 能力面收窄：manifest.inject 未声明 "net" → deny（响亮报错）
       │    ② 域白名单：manifest.net.allow 声明时，URL host 须匹配任一项
       │       （host === 域 或 host 以 "." + 域 结尾）；缺省 = 全部 https 公网
       │
       └─ POST /v1/net/fetch（onion-server rest/net.rs）             ← 安全底线
            ① 仅 https + 公网目标（字面量校验：拒绝 localhost / 私网 / 环回 /
               链路本地 IP / 无点单标签主机名）
            ② 重定向逐跳复检（任一跳不合规即停止跟随，防跳转绕过）
            ③ 方法仅 GET / POST；请求头白名单透传（accept / authorization /
               content-type / user-agent，其余丢弃，防头滥用）
            ④ 响应 ≤ 4 MiB（分块累计，超限即断）；上游超时 15s
            ⑤ 上游 HTTP 状态码透传（非 200 不是网关错误，调用方自行判定）
```

**双层分工**：服务端是安全底线（对所有 App 一致，防 SSRF / 滥用）；host 层是
App 级授权边界（per-App 域白名单，与 slots/agent/storage 的 `inject` 收窄机制
完全同构）。服务端不感知 App 身份（REST 无 App 上下文），App 级校验必须且
只能在 host 层做——`makeContext` 闭包持有 manifest，天然携带授权信息。

## 3. 契约面

### 3.1 manifest 声明

```jsonc
{
  "hostApi": 2,
  "inject": ["slots", "agent", "net"],          // 声明 net 能力面
  "net": { "allow": ["api.skillhub.cn"] },      // 可选；缺省 = 全部 https 公网
  ...
}
```

- `net.allow` 为裸域名清单（无 scheme / 路径）；匹配语义：URL host 等于域或为
  域的子域（`api.skillhub.cn` 匹配 `skillhub.cn`；域项忽略前导点）；
- `check:contracts` 校验 `net` 字段形态（`NetAuth` 类型分支：object + 可选
  allow 为非空字符串数组）。

### 3.2 ctx.net（types.ts `NetApi`）

```ts
interface NetFetchRequest {
  url: string;
  method?: "GET" | "POST";
  headers?: Record<string, string>;   // 白名单字段生效，其余丢弃
  body?: string;                       // 仅 POST
  encoding?: "text" | "dataUrl";       // dataUrl = data:<type>;base64,（图片等二进制）
}
interface NetApi {
  fetch(input: NetFetchRequest): Promise<{ status: number; contentType: string; body: string }>;
}
```

### 3.3 与 AgentChannel 的关系

`AgentChannel` **不含**外联方法（`fetchExternal` 已移除）——联网不是 Agent
能力。`installSkillHub` 等会话面方法仍在 AgentChannel（服务端落盘 + registry
刷新必须经会话身份）。

## 4. 越权语义（fail-loud）

与既有能力面收窄同语义，全部响应亮报错而非静默降级：

| 场景 | 报错 |
| --- | --- |
| manifest 未声明 `inject: ["net"]` 访问 `ctx.net` | `AppHost: App「<id>」缺少能力面 <net>（在 manifest.inject 声明）` |
| `net.allow` 声明后访问白名单外域 | `App「<id>」外联越权：<host> 不在 net.allow 白名单（<域>）` |
| URL 非公网 https / 方法非 GET·POST / 响应超限 | 服务端 400 / 422（`invalid_url` / `invalid_method` / `payload_too_large`） |

## 5. 实施落点

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 服务端 | `crates/onion-server/src/rest/net.rs` | `POST /v1/net/fetch` 通用形态 + `GET` 便捷形态；`validate_public_https_url` / `is_public_ip`（stable 手写私网段枚举，`IpAddr::is_global` 尚未稳定）/ 重定向 Policy / header 白名单 / dataUrl 编码 |
| 契约 | `contracts/app-contract.json` | inject 枚举增 `net`；ctx.surfaces 增 `net`；manifest fields 增 `net`（NetAuth）；ctx.net 服务段 |
| 契约校验 | `scripts/check-app-contract.mjs` | manifest 字段类型分派增 `NetAuth` 分支 |
| 宿主 | `src/host/types.ts` | `NetFetchRequest` / `NetApi` / `InjectKey` 增 `net` / `AppContext.net` / manifest `net.allow` |
| 宿主 | `src/host/agent.ts` | 导出 `netFetch(input)`（传输实现，POST /v1/net/fetch） |
| 宿主 | `src/host/registry.ts` | `CAPABILITY_SURFACES` 增 `net`；makeContext 增 `net` getter（granted + 域白名单校验） |
| 宿主 | `src/host/manifest.ts` | `INJECT_KEYS` 增 `net`；`net.allow` 形态校验 |
| App | `apps/skills-manager` | manifest 声明 `inject: [...,"net"]` + `net.allow`；视图经 `ctx.net.fetch` 直连上游 |

## 6. 测试

- 单元：`net_tests.rs` 覆盖 `validate_public_https_url` 放行/拒绝清单（私网 /
  环回 / 链路本地 / CGNAT / 保留段 / localhost 变体 / 无点主机名）；
- 集成（真实上游，`tests/rest_net.rs`）：search JSON 取回、上游 404 透传、
  POST method 透传（GET-only 端点回 405 证明原样转发）、非法方法 400、
  dataUrl 编码 + base64 解码校验、SSRF 边界；
- 交互验证：搜索 / 详情 / 安装全链路；临时改 `allow: ["example.com"]` 重装后
  搜索报「外联越权」，恢复后正常——host 层 fail-loud 实测生效。

## 7. 开放问题

- **响应头透传面**：目前仅 `content-type`；后续有分页 / ETag 需求时再扩字段。
- **服务端级域名策略**：可增加 `ONION_SERVER_NET_ALLOWLIST` 环境变量做全局
  收口（企业部署场景）；本地单用户形态暂无必要。
- **流式响应**：SSE / chunk 场景（如外部 LLM API）需读面协议升级（改用
  WS 多路复用或 SSE 透传），待真实需求出现再设计。
