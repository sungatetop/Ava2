// app-devkit：纯能力 App（无 UI 贡献）。
// 安装器要求每个 App 至少有一个入口文件（app_entry_missing），这里提供空 setup——
// 不注入任何 slot，宿主加载后不会渲染任何视图；真正的产出是 contributes.skills
// 交给边车在会话装配期注册（见 crates/onion-server/src/app_contrib.rs）。
export function setup() {
  return undefined;
}
