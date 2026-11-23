// socratic 样式：CSS 全部消费宿主 design token（--ava-*），选择器统一 sc- 前缀。
// Styles 组件负责把样式注入 document.head 一次（按 id 判重，幂等）。
import { useEffect } from "react";

const ACCENT = "#1f9e8f";

const CSS = `
.sc-root{height:100%;display:flex;flex-direction:column;box-sizing:border-box;color:var(--ava-label-primary);
  background:var(--ava-bg-base);font-size:13px;line-height:1.6;--sc-accent:${ACCENT}}
.sc-picker{flex:0 0 auto;display:flex;gap:8px;align-items:center;padding:10px 16px;
  border-bottom:0.5px solid var(--ava-border-l2);background:var(--ava-bg-layer-1);flex-wrap:wrap}
.sc-picker label{font-size:11px;color:var(--ava-label-tertiary);white-space:nowrap}
.sc-select{height:30px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-base);color:var(--ava-label-primary);font:inherit;font-size:12px;padding:0 8px;max-width:280px}
.sc-btn{height:30px;padding:0 14px;border-radius:var(--ava-radius-pill);border:0;background:var(--sc-accent);
  color:#fff;font:inherit;font-size:12px;font-weight:500;cursor:pointer;white-space:nowrap}
.sc-btn:disabled{opacity:.5;cursor:default}
.sc-meta{font-size:11px;color:var(--ava-label-tertiary);font-family:var(--ava-font-mono)}
.sc-body{flex:1;min-height:0;overflow-y:auto;padding:var(--ava-space-4) var(--ava-space-5)}
.sc-question{border:0.5px solid color-mix(in srgb,var(--sc-accent) 40%,transparent);border-radius:var(--ava-radius-md);
  background:color-mix(in srgb,var(--sc-accent) 6%,transparent);padding:var(--ava-space-3);margin-bottom:var(--ava-space-4)}
.sc-question .hd{display:flex;align-items:center;gap:8px;margin-bottom:6px}
.sc-chip{display:inline-flex;align-items:center;height:20px;padding:0 8px;border-radius:var(--ava-radius-pill);
  font-size:10.5px;font-weight:500;color:var(--sc-accent);background:color-mix(in srgb,var(--sc-accent) 12%,transparent)}
.sc-question p{margin:0;white-space:pre-wrap;font-size:13px;line-height:1.75}
.sc-dialogue{display:flex;flex-direction:column;gap:var(--ava-space-3);max-width:66ch}
.sc-msg{display:flex;gap:var(--ava-space-2)}
.sc-ava{width:24px;height:24px;border-radius:50%;flex:0 0 auto;display:grid;place-items:center;
  font-size:11px;font-weight:700;color:#fff;background:var(--sc-accent)}
.sc-ava.student{background:var(--ava-label-tertiary)}
.sc-bubble{border-radius:var(--ava-radius-md);padding:var(--ava-space-3);font-size:13px;line-height:1.75;
  background:var(--ava-bg-layer-1);border:0.5px solid var(--ava-border-l2);max-width:100%;white-space:pre-wrap}
.sc-msg.me .sc-bubble{background:var(--ava-interactive-bg-hover);border-color:transparent}
.sc-why{display:block;margin-top:6px;font-size:11px;color:var(--ava-label-tertiary)}
.sc-empty{color:var(--ava-label-tertiary);font-size:12px;text-align:center;padding:var(--ava-space-5) 0}
.sc-err{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-4);
  color:var(--ava-label-secondary);font-size:12px;line-height:1.8}
.sc-inputbar{flex:0 0 auto;display:flex;gap:8px;padding:10px 16px;border-top:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-layer-1)}
.sc-inputbar input{flex:1;min-width:0;height:34px;border-radius:var(--ava-radius-pill);
  border:0.5px solid var(--ava-border-l2);background:var(--ava-bg-base);padding:0 14px;font:inherit;font-size:13px;
  color:var(--ava-label-primary);outline:none}
.sc-inputbar input:focus{border-color:var(--sc-accent);box-shadow:0 0 0 2px color-mix(in srgb,var(--sc-accent) 18%,transparent)}
.sc-send{width:34px;height:34px;border-radius:50%;border:0;flex:0 0 auto;cursor:pointer;
  background:var(--sc-accent);color:#fff;display:grid;place-items:center;font-size:15px}
.sc-send:disabled{opacity:.45;cursor:default}
.sc-steps{padding:var(--ava-space-4);height:100%;overflow-y:auto;display:flex;flex-direction:column;gap:var(--ava-space-2)}
.sc-sec-title{font-size:11px;font-weight:600;color:var(--ava-label-tertiary);letter-spacing:.03em;margin:0 0 var(--ava-space-2)}
.sc-step{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:8px 10px 8px 34px;
  background:var(--ava-bg-layer-1);position:relative;font-size:12.5px;line-height:1.7;white-space:pre-wrap}
.sc-step .no{position:absolute;left:10px;top:9px;font-family:var(--ava-font-mono);font-size:11px;color:var(--ava-label-tertiary)}
.sc-step.latest{border-color:var(--sc-accent);box-shadow:inset 2px 0 0 var(--sc-accent)}
.sc-panel{padding:var(--ava-space-4)}
.sc-card{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-3);
  background:var(--ava-bg-layer-1)}
.sc-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:3px 0;gap:10px}
.sc-kv b{color:var(--ava-label-primary);font-weight:600;font-family:var(--ava-font-mono)}
.sc-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(22px,1fr));gap:4px;margin-top:6px}
.sc-cell{height:22px;border-radius:5px;display:grid;place-items:center;font-size:10px;font-family:var(--ava-font-mono);
  background:var(--ava-interactive-bg-hover);color:var(--ava-label-tertiary)}
.sc-cell.on{background:color-mix(in srgb,var(--sc-accent) 55%,transparent);color:#fff}
.sc-spin{display:inline-block;width:12px;height:12px;border:2px solid var(--ava-border-l2);border-top-color:var(--sc-accent);
  border-radius:50%;animation:sc-rot .8s linear infinite;vertical-align:-2px;margin-right:6px}
@keyframes sc-rot{to{transform:rotate(360deg)}}
`;

/** 样式注入组件：挂载时向 document.head 注入一次 sc- 样式。 */
export function Styles() {
  useEffect(() => {
    if (document.getElementById("sc-styles")) return;
    const el = document.createElement("style");
    el.id = "sc-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}
