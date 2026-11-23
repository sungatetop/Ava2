// skills-manager 页面入口：setup(ctx) 只登记 slot 工厂（契约 §2.1）。
// 视图组件一律经 props 拿 agent（顶层组件拿不到 ctx——组件工厂闭包传入）。
import type { AppContext } from "@ava-ai/app-sdk";
import { SkillsView } from "./skills-view";

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    // lucide sliders-horizontal：启停语义
    ctx.slots.inject("nav.sidebar", {
      order: 12,
      viewId: "skills-manager",
      title: "技能管理",
      icon: "M4 5h7 M4 10h9 M4 15h6 M17 4v6 M17 14v6 M14 7h6 M14 17h6",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "skills-manager",
      title: "技能管理",
      component: async () => ({ default: () => <SkillsView agent={ctx.agent} net={ctx.net} /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
