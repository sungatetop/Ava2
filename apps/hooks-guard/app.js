// 纯 hooks App 的占位入口：安装器要求每个 App 至少有一个可加载入口（app_entry_missing）。
// 本 App 无 UI 贡献，setup 不注入任何 slot；真正的产出是 contributes.hooks
// ——边车在会话装配期读取 hooks.json（Claude Code 兼容格式）挂载命令钩子。
export function setup() {
  return undefined;
}
