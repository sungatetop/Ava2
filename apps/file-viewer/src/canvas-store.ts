// 画布文件预览的共享状态：setup 期登记 agent + 总线监听（App 装配一次），
// 画布视图组件经 useSyncExternalStore 消费。
// 驱动约定（宿主 src/host/types.ts AppContext.events）：
// - 壳只处理 `canvas.open`（按 target 打开画布 tab）；
// - 本 App 自己监听 `canvas.open` / `canvas.openPath` 取 path + sessionId。
import { useSyncExternalStore } from "react";
import type { AgentChannel } from "@ava-ai/app-sdk";

/** 本 App 认领的画布视图 id（= canvas.open 的 target） */
export const VIEW_ID = "file-viewer";

/** 待预览的文件（相对路径按 sessionId 的权限根解析） */
export interface CanvasTarget {
  path: string;
  sessionId?: string;
}

let target: CanvasTarget | null = null;
let agent: AgentChannel | null = null;
const listeners = new Set<() => void>();

function publish(next: CanvasTarget) {
  target = next;
  for (const listener of listeners) listener();
}

/** 总线载荷 → 打开目标；target 非本视图或 path 缺失时返回 null */
function readTarget(e: unknown): CanvasTarget | null {
  if (typeof e !== "object" || e === null) return null;
  const payload = e as { target?: unknown; path?: unknown; sessionId?: unknown };
  if (typeof payload.target === "string" && payload.target !== VIEW_ID) return null;
  if (typeof payload.path !== "string" || payload.path.trim() === "") return null;
  return {
    path: payload.path,
    sessionId: typeof payload.sessionId === "string" ? payload.sessionId : undefined,
  };
}

export const canvasStore = {
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  snapshot(): CanvasTarget | null {
    return target;
  },
  /** setup 期登记 Agent 通道（组件渲染前必已完成） */
  setAgent(channel: AgentChannel) {
    agent = channel;
  },
  /** 总线事件入口（canvas.open / canvas.openPath 共用） */
  accept(e: unknown) {
    const next = readTarget(e);
    if (next) publish(next);
  },
};

/** 当前待预览目标 */
export function useCanvasTarget(): CanvasTarget | null {
  return useSyncExternalStore(canvasStore.subscribe, canvasStore.snapshot);
}

/** Agent 通道（setup 后非空；未装配时返回 null，由视图降级提示） */
export function getCanvasAgent(): AgentChannel | null {
  return agent;
}
