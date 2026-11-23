// lazy 组件缓存（按 appId::viewId 键控）：React.lazy 工厂每次渲染重建会反复执行
// 动态 import 并重置组件内部状态，必须稳定持有；热重载/卸载时按 appId 清除。
import { lazy } from "react";

const lazyCache = new Map<string, ReturnType<typeof lazy>>();

/** 组件实例身份 id：热重载换新 lazy 实例时，MainSurface 以此作为 ErrorBoundary 的 key */
const lazyIds = new WeakMap<object, number>();
let lazyIdSeq = 0;

export function lazyId(comp: ReturnType<typeof lazy>): string {
  let id = lazyIds.get(comp as unknown as object);
  if (id === undefined) {
    id = ++lazyIdSeq;
    lazyIds.set(comp as unknown as object, id);
  }
  return String(id);
}

function key(appId: string, viewId: string): string {
  return `${appId}::${viewId}`;
}

export function getLazy(appId: string, viewId: string): ReturnType<typeof lazy> | undefined {
  return lazyCache.get(key(appId, viewId));
}

export function setLazy(appId: string, viewId: string, comp: ReturnType<typeof lazy>): void {
  lazyCache.set(key(appId, viewId), comp);
}

/** 按 appId 清除（App unload / 热重载后调用，保证下次渲染 import 新模块） */
export function clearLazyCache(appId: string): void {
  const prefix = `${appId}::`;
  for (const k of [...lazyCache.keys()]) {
    if (k.startsWith(prefix)) lazyCache.delete(k);
  }
}
