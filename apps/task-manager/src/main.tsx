// task-manager 页面入口：setup(ctx) 只登记 slot 工厂与设置面板，不碰 DOM（契约 §2.1）。
// 视图组件一律经 props 拿 agent（顶层组件拿不到 ctx——组件工厂闭包传入）。
import type { AppContext } from "@ava-ai/app-sdk";
import { TasksView } from "./tasks-view";

export function setup(ctx: AppContext): { dispose(): void } {
  // 登记设置面板：应用市场详情页出现「设置」入口（懒挂载，宿主不参与存储/校验）
  ctx.registerSettingsPanel(() => import("./settings-panel"));
  const disposers = [
    // lucide clock：定时语义
    ctx.slots.inject("nav.sidebar", {
      order: 8,
      viewId: "task-manager",
      title: "定时任务",
      icon: "M12 6v6l4.5 3 M21.5 12a9.5 9.5 0 1 1-19 0 9.5 9.5 0 0 1 19 0z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "task-manager",
      title: "定时任务",
      component: async () => ({ default: () => <TasksView agent={ctx.agent} /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
