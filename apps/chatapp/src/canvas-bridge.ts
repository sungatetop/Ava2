// 画布打开桥：对话流里的交付物卡片需要把文件送进画布第二舞台，
// 但 items.tsx 深处的组件拿不到 ctx（App 契约只把 ctx 交给 setup）。
// setup 期登记一次总线，组件侧只调 openInCanvas。
import type { AppContext } from "@ava-ai/app-sdk";

/** 画布文件预览视图 id（= file-viewer App 认领的 surface.canvas viewId） */
export const FILE_VIEWER_ID = "file-viewer";

let bus: AppContext["events"] | null = null;

export function setCanvasBus(events: AppContext["events"]) {
  bus = events;
}

/** 打开画布文件预览：`canvas.open` 让壳加入并展开 tab，
 *  `canvas.openPath` 携带 path（+ sessionId 解析相对路径）给预览视图。 */
export function openInCanvas(path: string, sessionId?: string) {
  if (!bus) return;
  const payload = { target: FILE_VIEWER_ID, path, sessionId };
  bus.emit("canvas.open", payload);
  bus.emit("canvas.openPath", payload);
}
