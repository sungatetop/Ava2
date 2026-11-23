# app_* 错误码对照与排查

安装返回非 ok、或 App 在应用管理面板进入 `failed` 时，按下表排查。错误码同时出现在安装端点响应体与面板失败原因中。

| 错误码 | 原因 / 处理 |
| --- | --- |
| `app_manifest_missing` / `app_manifest_invalid` | 目录里没有 `manifest.json`，或不是合法 JSON 对象 |
| `app_id_missing` / `app_id_invalid` | `id` 缺失，或含非法字符 / 超 64 字符 |
| `app_name_missing` | `name` 为空 |
| `app_host_api_unsupported` | `hostApi` 超过宿主上限（当前 2） |
| `app_entry_missing` / `app_entry_invalid` / `app_entry_not_found` | 缺入口、入口是绝对路径或含 `..`、入口文件不存在（构建型请装 `dist/`） |
| `app_contributes_invalid` | `contributes` 里路径越界或不存在、MCP / hooks 配置无法解析 |
| `app_exists` | 已安装；覆盖请 `overwrite: true` |
| `app_path_invalid` | 安装源不可用（必须是含 `manifest.json` 的目录） |

排查顺序：

1. 先跑 `npm run check:contracts`——多数 manifest / 入口 / contributes 问题在校验期就能暴露。
2. 对照上表修正 manifest 或目录结构。
3. 运行期 `failed`（清单合法但 setup 抛错 / 越权注入）：失败原因展示在应用管理面板，修正后 `overwrite: true` 重装。
4. 装上了但页面无变化：确认浏览器访问的是 `http://localhost:1420/`，再确认 App 未被 `enabled: false` 停用（`GET /v1/apps` 看 `enabled`）。

## 运行期外联报错（非 app_* 错误码）

`App「<id>」外联越权：<host> 不在 net.allow 白名单（...）` ——宿主在 `ctx.net.fetch` 调用时抛出（fail-loud，不影响其他 App）。修正方向：

1. manifest 漏声明：`inject` 补 `"net"`；
2. 域未授权：把目标 host（或其父域）加入 `manifest.net.allow`；
3. 修正后 bump `version` 并 `overwrite: true` 重装。

另注意：`ctx.net.fetch` 返回的 404/405 等是**上游状态码原样透传**，属正常响应，按上游语义处理，勿当作宿主错误。
