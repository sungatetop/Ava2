// file-explorer 页面入口：setup(ctx) 只登记 slot 工厂与总线桥，不碰 DOM（契约 §2.1）。
// 文件树挂在画布（第二舞台）里，作为全局可折叠面板（viewId "*"），
// 跟随任意画布视图出现（含 canvas:home），无需先打开文件预览；折叠由宿主 tab 栏按钮控制。
import type { AppContext } from "@ava-ai/app-sdk";
import { FILE_VIEWER_ID, setCanvasBus } from "./canvas-bridge";
import { FileExplorer } from "./explorer";

export function setup(ctx: AppContext): { dispose(): void } {
  setCanvasBus(ctx.events);
  const disposers = [
    ctx.slots.inject("panel.canvas", {
      // 全局画布面板：viewId "*" 使其跟随所有画布视图（含 canvas:home），独立于文件预览
      order: 6,
      viewId: "*",
      title: "文件",
      // folder-tree（lucide 同源）：树形目录，多路径组合（应用市场图标）
      icon: {
        d: [
          "M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z",
          "M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z",
          "M3 5a2 2 0 0 0 2 2h3",
          "M3 3v13a2 2 0 0 0 2 2h3",
        ],
      },
      component: async () => ({
        default: () => <FileExplorer agent={ctx.agent} />,
      }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
