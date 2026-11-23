// 画布打开桥：与 chatapp 的 canvas-bridge 同一约定——setup 期登记总线，
// 组件侧 emit `canvas.open`（壳负责加入/展开画布 tab）+ `canvas.openPath`
// （file-viewer App 消费 path 并预览）。
import type { AppContext } from "@ava-ai/app-sdk";

/** 画布文件预览视图 id（= file-viewer App 认领的 surface.canvas viewId） */
export const FILE_VIEWER_ID = "file-viewer";

let bus: AppContext["events"] | null = null;

export function setCanvasBus(events: AppContext["events"]) {
  bus = events;
}

/** 在画布打开文件（绝对路径；`sessionId` 让预览端按该会话权限根校验） */
export function openInCanvas(path: string, sessionId?: string) {
  if (!bus) return;
  bus.emit("canvas.open", { target: FILE_VIEWER_ID, path, sessionId });
  bus.emit("canvas.openPath", { target: FILE_VIEWER_ID, path, sessionId });
}
