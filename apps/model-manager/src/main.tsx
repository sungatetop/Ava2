// model-manager 页面入口：setup(ctx) 只登记 slot 工厂与设置面板，不碰 DOM（契约 §2.1）。
// 视图组件一律经 props 拿 agent（顶层组件拿不到 ctx——组件工厂闭包传入）。
import type { AppContext } from "@ava-ai/app-sdk";
import { ModelsView } from "./models-view";

export function setup(ctx: AppContext): { dispose(): void } {
  // 登记设置面板：应用市场详情页出现「设置」入口（懒挂载，宿主不参与存储/校验）
  ctx.registerSettingsPanel(() => import("./settings-panel"));
  const disposers = [
    // lucide sliders-horizontal：模型/供应商配置语义
    ctx.slots.inject("nav.sidebar", {
      order: 7,
      viewId: "model-manager",
      title: "模型管理",
      icon: "M21 4h-7 M10 4H3 M21 12h-9 M8 12H3 M21 20h-5 M12 20H3 M14 2v4 M8 10v4 M16 18v4",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "model-manager",
      title: "模型管理",
      component: async () => ({ default: () => <ModelsView agent={ctx.agent} /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
