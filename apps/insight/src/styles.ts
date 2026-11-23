// 数据叙事台样式层：in- 前缀类名，全部消费 --ava-* 令牌；Styles 组件一次性注入 <style> 到 head。
import { useEffect } from "react";

export const ACCENT = "#3f5bd9";

export const CSS = `
.in-root{height:100%;display:flex;flex-direction:column;box-sizing:border-box;color:var(--ava-label-primary);
  background:var(--ava-bg-base);font-size:13px;line-height:1.6;--in-accent:${ACCENT}}
.in-body{flex:1;overflow-y:auto;padding:var(--ava-space-4) var(--ava-space-5);max-width:760px}
.in-dataset{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-3) var(--ava-space-4);
  display:flex;align-items:center;gap:var(--ava-space-3);background:var(--ava-bg-layer-1)}
.in-file{width:30px;height:30px;border-radius:8px;display:grid;place-items:center;font-family:var(--ava-font-mono);
  font-size:10px;font-weight:700;color:#fff;background:var(--in-accent);flex:0 0 auto}
.in-insight{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-4);
  background:var(--ava-bg-layer-1);margin-top:var(--ava-space-3)}
.in-insight h3{margin:8px 0 6px;font-size:14px;font-weight:600;line-height:1.5}
.in-num{font-size:22px;font-weight:700;letter-spacing:-.4px;font-variant-numeric:tabular-nums}
.in-num small{font-size:12px;font-weight:500;color:var(--ava-label-tertiary);margin-left:6px;letter-spacing:0}
.in-detail{margin-top:8px;font-size:12px;color:var(--ava-label-secondary);line-height:1.7}
.in-conf{display:flex;align-items:center;gap:6px;font-size:11px;color:var(--ava-label-tertiary);margin-top:8px}
.in-conf .lbl{width:52px;flex:0 0 auto}
.in-bar{flex:1;max-width:160px;height:6px;border-radius:999px;background:var(--ava-interactive-bg-hover);overflow:hidden}
.in-bar i{display:block;height:100%;border-radius:999px;background:var(--in-accent)}
.in-mono{font-family:var(--ava-font-mono);font-size:11px}
.in-chip{display:inline-flex;align-items:center;height:20px;padding:0 8px;border-radius:var(--ava-radius-pill);
  font-size:10.5px;font-weight:500;color:var(--in-accent);background:color-mix(in srgb,var(--in-accent) 12%,transparent)}
.in-empty{color:var(--ava-label-tertiary);font-size:12px;text-align:center;padding:var(--ava-space-5) 0}
.in-err{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-4);
  color:var(--ava-label-secondary);font-size:12px;line-height:1.8}
.in-btn{height:24px;padding:0 10px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-layer-1);color:var(--ava-label-primary);font-size:11px;cursor:pointer;flex:0 0 auto}
.in-btn:hover:not(:disabled){background:var(--ava-interactive-bg-hover)}
.in-btn:disabled{opacity:.5;cursor:default}
.in-sec-title{font-size:11px;font-weight:600;color:var(--ava-label-tertiary);letter-spacing:.03em;margin:0 0 var(--ava-space-2)}
/* 图表墙 */
.in-charts{padding:var(--ava-space-4);display:flex;flex-direction:column;gap:var(--ava-space-3);height:100%;overflow-y:auto}
.in-chart{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-3);background:var(--ava-bg-layer-1)}
.in-chart .hd{display:flex;justify-content:space-between;align-items:center;font-size:12px;font-weight:600;margin-bottom:var(--ava-space-2)}
.in-chart svg{display:block;width:100%;height:auto}
.in-axis{stroke:var(--ava-border-l2);stroke-width:1}
.in-grid{stroke:var(--ava-border-l1);stroke-width:1}
.in-lbl{fill:var(--ava-label-tertiary);font-size:9px;font-family:var(--ava-font-mono)}
.in-hl{fill:color-mix(in srgb,var(--in-accent) 12%,transparent)}
/* 面板 */
.in-panel{padding:var(--ava-space-4)}
.in-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:4px 0}
.in-kv b{color:var(--ava-label-primary);font-weight:600;font-family:var(--ava-font-mono)}
.in-nav{display:flex;flex-direction:column;gap:2px;padding:var(--ava-space-3)}
.in-nav-item{display:flex;align-items:center;gap:var(--ava-space-2);height:32px;padding:0 var(--ava-space-2);
  border-radius:var(--ava-radius-sm);color:var(--ava-label-secondary);cursor:pointer;font-size:13px}
.in-nav-item:hover{background:var(--ava-interactive-bg-hover)}
.in-nav-item.active{background:var(--ava-nav-active);color:var(--ava-label-primary);font-weight:500}
.in-nav-item .tail{margin-left:auto;font-size:11px;color:var(--ava-label-tertiary)}
`;

export function Styles() {
  useEffect(() => {
    if (document.getElementById("in-styles")) return;
    const el = document.createElement("style");
    el.id = "in-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}
