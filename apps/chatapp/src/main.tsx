// chatapp 页面入口：setup(ctx) 只登记 slot 工厂与订阅，不碰 DOM（契约 §2.1）。
import type { AppContext } from "@ava-ai/app-sdk";
import { setCanvasBus } from "./canvas-bridge";
import { ChatView } from "./chat-view";
import { SessionSidebar } from "./session-sidebar";

export function setup(ctx: AppContext): { dispose(): void } {
  // 交付物卡片 → 画布第二舞台的打开桥（组件深处拿不到 ctx，故在 setup 期登记总线）
  setCanvasBus(ctx.events);
  // 设置面板（宿主只提供入口）：应用市场详情页出现「设置」，点击惰性挂载本 App 的面板组件
  ctx.registerSettingsPanel(() => import("./settings"));
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 5,
      viewId: "chatapp",
      title: "对话",
      // message-circle（lucide 同源）：单气泡对话，描边 currentColor
      icon: "M7.9 20A9 9 0 1 0 4 16.1L2 22z",
    }),
    // 会话历史（按工作区分组）挂在宿主侧边栏，仅本视图处于激活态时渲染
    ctx.slots.inject("panel.sidebar", {
      order: 5,
      viewId: "chatapp",
      title: "会话历史",
      component: async () => ({ default: () => <SessionSidebar agent={ctx.agent} /> }),
    }),
    ctx.slots.inject("surface.main", {
      viewId: "chatapp",
      title: "对话",
      component: async () => ({ default: () => <ChatView agent={ctx.agent} storage={ctx.storage} /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
