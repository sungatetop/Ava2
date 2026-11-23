// storyloom 样式：全部消费宿主 design token（--ava-*），选择器限域 sl- 前缀。
import { useEffect } from "react";

export const ACCENT = "#b8873d";

export const CSS = `
.sl-root{height:100%;display:flex;flex-direction:column;box-sizing:border-box;color:var(--ava-label-primary);
  background:var(--ava-bg-base);font-size:13px;line-height:1.6;--sl-accent:${ACCENT}}
.sl-split{flex:1;display:grid;grid-template-columns:280px 1fr;min-height:0}
.sl-picker{flex:0 0 auto;display:flex;gap:8px;align-items:center;padding:10px 16px;
  border-bottom:0.5px solid var(--ava-border-l2);background:var(--ava-bg-layer-1);flex-wrap:wrap}
.sl-picker label{font-size:11px;color:var(--ava-label-tertiary);white-space:nowrap}
.sl-select{height:30px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-base);color:var(--ava-label-primary);font:inherit;font-size:12px;padding:0 8px;max-width:280px}
.sl-btn{height:30px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-base);color:var(--ava-label-primary);font:inherit;font-size:12px;padding:0 10px;cursor:pointer}
.sl-btn:hover:not(:disabled){border-color:color-mix(in srgb,var(--sl-accent) 55%,transparent)}
.sl-btn:disabled{opacity:.45;cursor:default}
.sl-meta{font-size:11px;color:var(--ava-label-tertiary);font-family:var(--ava-font-mono)}
.sl-beats{border-right:0.5px solid var(--ava-border-l2);overflow-y:auto;padding:var(--ava-space-3)}
.sl-sec-title{font-size:11px;font-weight:600;color:var(--ava-label-tertiary);letter-spacing:.03em;margin:0 0 var(--ava-space-2)}
.sl-beat{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:8px 10px;
  margin-bottom:var(--ava-space-2);cursor:pointer;background:var(--ava-bg-layer-1);position:relative}
.sl-beat:hover{border-color:color-mix(in srgb,var(--sl-accent) 55%,transparent)}
.sl-beat.active{border-color:var(--sl-accent);box-shadow:inset 2px 0 0 var(--sl-accent)}
.sl-beat .n{font-family:var(--ava-font-mono);font-size:10.5px;color:var(--ava-label-tertiary)}
.sl-beat h4{margin:2px 0 4px;font-size:12.5px;font-weight:600;line-height:1.45;
  display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}
.sl-stress{display:flex;gap:2px;margin-top:6px}
.sl-stress i{width:10px;height:4px;border-radius:2px;background:var(--ava-interactive-bg-hover)}
.sl-stress i.on{background:var(--sl-accent)}
.sl-budget{font-size:10.5px;color:var(--ava-label-tertiary);margin-top:4px;font-family:var(--ava-font-mono)}
.sl-paper{overflow-y:auto;padding:calc(var(--ava-space-5) + 16px) var(--ava-space-6)}
.sl-dialog{max-width:66ch;font-size:14px;line-height:1.9;margin:0;text-indent:2em;white-space:pre-wrap}
.sl-speaker{font-size:11px;color:var(--ava-label-tertiary);letter-spacing:.06em;display:block;margin-bottom:6px;font-family:var(--ava-font-mono)}
.sl-lock{border:0.5px solid color-mix(in srgb,var(--sl-accent) 45%,transparent);border-radius:var(--ava-radius-md);
  background:color-mix(in srgb,var(--sl-accent) 7%,transparent);padding:var(--ava-space-3);margin-top:var(--ava-space-3);font-size:12px}
.sl-chip{display:inline-flex;align-items:center;height:20px;padding:0 8px;border-radius:var(--ava-radius-pill);
  font-size:10.5px;font-weight:500;color:var(--sl-accent);background:color-mix(in srgb,var(--sl-accent) 12%,transparent)}
.sl-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:3px 0;gap:12px}
.sl-kv b{color:var(--ava-label-primary);font-weight:600;text-align:right;word-break:break-all}
.sl-canvas{padding:var(--ava-space-4);height:100%;overflow-y:auto}
.sl-map{width:100%;height:auto}
.sl-node circle{fill:var(--ava-bg-layer-2);stroke:var(--sl-accent);stroke-width:1.2}
.sl-node.hub circle{fill:var(--sl-accent)}
.sl-node text{fill:var(--ava-label-secondary);font-size:10px;font-family:var(--ava-font-ui)}
.sl-node.hub text{fill:#fff;font-weight:600}
.sl-edge{stroke:var(--ava-border-l3);stroke-width:1}
.sl-heat{display:flex;align-items:center;gap:6px;font-size:11px;color:var(--ava-label-secondary);margin-top:6px}
.sl-heat .lbl{width:48px;flex:0 0 auto;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.sl-bar{flex:1;height:6px;border-radius:999px;background:var(--ava-interactive-bg-hover);overflow:hidden}
.sl-bar i{display:block;height:100%;border-radius:999px;background:var(--sl-accent)}
.sl-heat .mono{font-family:var(--ava-font-mono);font-size:10.5px;color:var(--ava-label-tertiary)}
.sl-panel{padding:var(--ava-space-3) var(--ava-space-4)}
.sl-curve{width:100%;height:80px}
.sl-empty{color:var(--ava-label-tertiary);font-size:12px;text-align:center;padding:var(--ava-space-5) 0}
.sl-err{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-4);
  color:var(--ava-label-secondary);font-size:12px;line-height:1.8}
.sl-spin{display:inline-block;width:12px;height:12px;border:2px solid var(--ava-border-l2);border-top-color:var(--sl-accent);
  border-radius:50%;animation:sl-rot .8s linear infinite;vertical-align:-2px;margin-right:6px}
@keyframes sl-rot{to{transform:rotate(360deg)}}
.sl-tip{display:inline-flex;align-items:center;height:22px;padding:0 10px;border-radius:var(--ava-radius-pill);
  background:var(--ava-interactive-bg-hover);color:var(--ava-label-secondary);font-size:11px;margin:4px 6px 0 0}
`;

/** 注入全局样式（幂等：已挂载则跳过）。 */
export function Styles() {
  useEffect(() => {
    if (document.getElementById("sl-styles")) return;
    const el = document.createElement("style");
    el.id = "sl-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}
