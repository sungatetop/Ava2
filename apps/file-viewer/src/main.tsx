// file-viewer 页面入口：setup(ctx) 只登记 slot 工厂与总线监听，不碰 DOM（契约 §2.1）。
// 认领画布第二舞台 surface.canvas：任意 App 发 `canvas.open`（target=file-viewer）
// 即可打开本视图；`path` / `sessionId` 由本 App 自己从同一总线事件里取。
import type { AppContext } from "@ava-ai/app-sdk";
import { VIEW_ID, canvasStore } from "./canvas-store";
import { CanvasView } from "./viewer";

export function setup(ctx: AppContext): { dispose(): void } {
  canvasStore.setAgent(ctx.agent);
  const disposers = [
    ctx.events.on("canvas.open", (e) => canvasStore.accept(e)),
    ctx.events.on("canvas.openPath", (e) => canvasStore.accept(e)),
    ctx.slots.inject("surface.canvas", {
      viewId: VIEW_ID,
      title: "文件预览",
      // file-search（lucide 同源）：文件 + 放大镜，多路径组合
      icon: {
        d: [
          "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z",
          "M14 2v6h6",
          "M11 18a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
          "M16 16l2.5 2.5",
        ],
      },
      description: "预览工作区文件（图片 / 文本 / Markdown / HTML / Office）",
      component: async () => ({ default: () => <CanvasView /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
