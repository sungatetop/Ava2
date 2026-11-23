// RepoAtlas 样式：CSS 字符串 + 全局注入组件（幂等，id ra-styles 防重复挂载）。
import { useEffect } from "react";
import { ACCENT } from "./lib";

const CSS = `
.ra-root{height:100%;display:flex;flex-direction:column;box-sizing:border-box;color:var(--ava-label-primary);
  background:var(--ava-bg-base);font-size:13px;line-height:1.6;--ra-accent:${ACCENT}}
.ra-body{flex:1;overflow-y:auto;padding:var(--ava-space-4) var(--ava-space-5);max-width:780px}
.ra-bar{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-3) var(--ava-space-4);
  display:flex;gap:var(--ava-space-2);align-items:center;background:var(--ava-bg-layer-1);flex-wrap:wrap}
.ra-bar label{font-size:11px;color:var(--ava-label-tertiary);white-space:nowrap}
.ra-select{height:32px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);background:var(--ava-bg-base);
  color:var(--ava-label-primary);font:inherit;font-size:12px;padding:0 8px;min-width:200px;flex:1;max-width:360px}
.ra-query{flex:1;min-width:140px;height:32px;border-radius:var(--ava-radius-sm);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-base);padding:0 10px;font:inherit;color:var(--ava-label-primary)}
.ra-btn{height:32px;padding:0 16px;border-radius:var(--ava-radius-pill);border:0;background:var(--ra-accent);
  color:#fff;font:inherit;font-size:12px;font-weight:500;cursor:pointer;white-space:nowrap}
.ra-btn:disabled{opacity:.5;cursor:default}
.ra-btn-ghost{height:32px;padding:0 14px;border-radius:var(--ava-radius-pill);border:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-base);color:var(--ava-label-secondary);font:inherit;font-size:12px;cursor:pointer;white-space:nowrap}
.ra-btn-ghost:disabled{opacity:.5;cursor:default}
.ra-meta{font-size:11px;color:var(--ava-label-tertiary);margin-top:6px;font-family:var(--ava-font-mono)}
.ra-summary{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-3) var(--ava-space-4);
  margin-top:var(--ava-space-3);display:flex;gap:var(--ava-space-5);align-items:center;background:var(--ava-bg-layer-1);flex-wrap:wrap}
.ra-metric{display:flex;flex-direction:column;gap:2px}
.ra-metric b{font-size:20px;font-weight:700;font-variant-numeric:tabular-nums}
.ra-metric span{font-size:11px;color:var(--ava-label-tertiary)}
.ra-risk{display:flex;align-items:center;gap:8px;padding:var(--ava-space-2) var(--ava-space-3);border-radius:var(--ava-radius-sm);
  background:color-mix(in srgb,var(--ava-state-danger) 8%,transparent);font-size:12px;margin-top:var(--ava-space-3)}
.ra-tag{display:inline-flex;height:20px;align-items:center;padding:0 8px;border-radius:var(--ava-radius-pill);font-size:10.5px;
  border:0.5px solid var(--ava-border-l2);color:var(--ava-label-secondary)}
.ra-tag.bad{color:var(--ava-state-danger);border-color:color-mix(in srgb,var(--ava-state-danger) 40%,transparent)}
.ra-sec-title{font-size:11px;font-weight:600;color:var(--ava-label-tertiary);letter-spacing:.03em;margin:var(--ava-space-4) 0 var(--ava-space-2)}
.ra-row{display:flex;gap:var(--ava-space-3);padding:var(--ava-space-2) 0;border-bottom:0.5px solid var(--ava-border-l1)}
.ra-row:last-child{border-bottom:0}
.ra-row .lvl{width:56px;flex:0 0 auto;font-size:11px;font-weight:600;letter-spacing:.02em}
.lvl.direct{color:var(--ava-state-danger)}
.lvl.test{color:var(--ava-state-success)}
.lvl.indirect{color:var(--ava-state-warning)}
.ra-row .body{flex:1;min-width:0}
.ra-row .file{font-family:var(--ava-font-mono);font-size:12px;word-break:break-all}
.ra-row .hits{font-size:11px;color:var(--ra-accent);font-family:var(--ava-font-mono);white-space:nowrap}
.ra-row .note{font-size:11px;color:var(--ava-label-tertiary);margin-top:2px;font-family:var(--ava-font-mono)}
.ra-mono{font-family:var(--ava-font-mono)}
.ra-empty{color:var(--ava-label-tertiary);font-size:12px;text-align:center;padding:var(--ava-space-5) 0;white-space:pre-line}
.ra-err{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:var(--ava-space-4);
  color:var(--ava-label-secondary);font-size:12px;line-height:1.8}
.ra-spin{display:inline-block;width:12px;height:12px;border:2px solid var(--ava-border-l2);border-top-color:var(--ra-accent);
  border-radius:50%;animation:ra-rot 0.8s linear infinite;vertical-align:-2px;margin-right:6px}
@keyframes ra-rot{to{transform:rotate(360deg)}}
/* 画布：架构地图 */
.ra-mapwrap{position:relative;border-radius:var(--ava-radius-md);background:var(--ava-bg-layer-1);
  border:0.5px solid var(--ava-border-l1);padding:var(--ava-space-2)}
.ra-module{cursor:pointer}
.ra-module rect{fill:var(--ava-bg-layer-2);stroke:var(--ava-border-l2);stroke-width:1;rx:7}
.ra-module:hover rect{stroke:var(--ra-accent)}
.ra-module.sel rect{fill:color-mix(in srgb,var(--ra-accent) 14%,transparent);stroke:var(--ra-accent);stroke-width:1.4}
.ra-module text{fill:var(--ava-label-secondary);font-size:9.5px;font-family:var(--ava-font-mono);pointer-events:none}
.ra-module.sel text{fill:var(--ra-accent);font-weight:700}
.ra-edge{stroke:var(--ava-border-l3);stroke-width:1;fill:none}
.ra-edge.hot{stroke:var(--ra-accent);stroke-width:1.6}
.ra-canvas{padding:var(--ava-space-4);height:100%;overflow-y:auto}
.ra-legend{display:flex;gap:var(--ava-space-3);font-size:10px;color:var(--ava-label-tertiary);margin-top:var(--ava-space-2)}
.ra-heat{display:flex;align-items:center;gap:6px;font-size:11px;color:var(--ava-label-secondary);margin-top:6px}
.ra-heat .lbl{width:70px;flex:0 0 auto;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.ra-heat-bar{flex:1;height:6px;border-radius:999px;background:var(--ava-interactive-bg-hover);overflow:hidden}
.ra-heat-bar i{display:block;height:100%;border-radius:999px;background:var(--ra-accent)}
.ra-panel{padding:var(--ava-space-4)}
.ra-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:4px 0}
.ra-kv b{color:var(--ava-label-primary);font-weight:500}
`;

export function Styles() {
  useEffect(() => {
    if (document.getElementById("ra-styles")) return;
    const el = document.createElement("style");
    el.id = "ra-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}
