# app_* 错误码排查

安装 / 加载 App 失败时按错误码对照下表处理，修复后重试安装。

| 错误码 | 原因 / 处理 |
| --- | --- |
| `app_manifest_missing` / `app_manifest_invalid` | 目录里没有 `manifest.json`，或不是合法 JSON 对象；确认文件名与 JSON 语法 |
| `app_id_missing` / `app_id_invalid` | `id` 缺失、含非法字符或超 64 字符；规则 `^[A-Za-z0-9._-]{1,64}$` |
| `app_name_missing` | `name` 为空 |
| `app_host_api_unsupported` | `hostApi` 超过宿主上限（当前 2），改回 `2` |
| `app_entry_missing` / `app_entry_invalid` / `app_entry_not_found` | 缺入口、入口是绝对路径或含 `..`、入口文件不存在；构建型 App 必须安装 `dist/` 而非源码目录 |
| `app_contributes_invalid` | `contributes` 路径越界（绝对路径或 `..`）或不存在、MCP / hooks 配置无法解析 |
| `app_exists` | 同 id App 已安装；覆盖安装请带 `overwrite: true` |
| `app_path_invalid` | 安装源不可用（必须是含 `manifest.json` 的目录） |

## 排查顺序

1. 确认安装源目录：`manifest.json` 是否存在、是否合法 JSON、`id` 与目录名是否一致。
2. 确认入口：`entry` / `contributes.apps[].entry` 是否为 App 根目录内的相对路径且文件真实存在（构建型装 `dist/`）。
3. 确认 `contributes` 所有路径落在 App 根目录内且存在。
4. 运行时 `failed`（非安装期报错）：看 App 管理面板的失败原因，多为 setup 抛错或越权注入（`surfaces` 未声明该 slot key）。
