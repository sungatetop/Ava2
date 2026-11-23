// 管家界面样式：借鉴 ava-app 的 butler-chat-redesign 原型。
// 设计要点：管家说话用文档式 prose（无气泡无头像），结构化信息用
// 派发卡 / 运行态活动条 / 升级卡 / 失败卡承载；右侧为 300px 工作台侧栏。
// 全部色值走 --ava-* 语义令牌，亮暗主题自适应。
export const CSS = `
.bt-root{height:100%;display:flex;min-height:0;box-sizing:border-box;color:var(--ava-label-primary);
  background:var(--ava-bg-base);font-size:13px;line-height:1.6}
.bt-mainpane{flex:1;min-width:0;display:flex;flex-direction:column}

/* ── 顶部标签条：总管家 + worker 会话 ── */
.bt-tabs{height:40px;flex:0 0 auto;display:flex;align-items:center;gap:2px;padding:0 8px;
  border-bottom:0.5px solid var(--ava-border-l2);overflow-x:auto;scrollbar-width:none}
.bt-tabs::-webkit-scrollbar{display:none}
.bt-tab{display:inline-flex;align-items:center;gap:6px;font-size:12px;color:var(--ava-label-secondary);
  padding:5px 9px;border-radius:var(--ava-radius-sm);cursor:pointer;white-space:nowrap;border:0;background:transparent;font-family:inherit}
.bt-tab:hover{background:var(--ava-interactive-bg-hover)}
.bt-tab.on{background:var(--ava-interactive-bg-hover);color:var(--ava-label-primary);font-weight:600}
.bt-tab .dot{width:5px;height:5px;border-radius:50%;background:var(--ava-state-business-primary);
  animation:bt-pulse 1.4s ease-in-out infinite;flex:0 0 auto}
.bt-tab .dot.done{background:var(--ava-label-tertiary);animation:none}
@keyframes bt-pulse{0%,100%{opacity:1}50%{opacity:.35}}
.bt-tab-spacer{flex:1}

/* ── 滚动区 ── */
.bt-scroll{flex:1;min-height:0;overflow-y:auto;padding:16px 20px 12px}
.bt-flow{display:flex;flex-direction:column;gap:14px;max-width:720px}
.bt-divider{display:flex;align-items:center;gap:10px;font-size:10px;color:var(--ava-label-tertiary);margin:2px 0}
.bt-divider::before,.bt-divider::after{content:"";flex:1;height:0.5px;background:var(--ava-border-l2)}

/* 用户发言：右对齐色块，无头像 */
.bt-self{display:flex;justify-content:flex-end}
.bt-self .bubble{max-width:78%;padding:8px 12px;border-radius:var(--ava-radius-lg);
  border-top-right-radius:4px;background:color-mix(in srgb,var(--ava-state-business-primary) 10%,transparent);
  font-size:13px;line-height:1.7;white-space:pre-wrap;word-break:break-word}

/* 管家发言：文档式排版，无气泡 */
.bt-prose{font-size:13.5px;line-height:1.85;color:var(--ava-label-primary);max-width:100%}
.bt-prose p{margin:0 0 8px}
.bt-prose p:last-child{margin-bottom:0}
.bt-prose ul,.bt-prose ol{margin:0 0 8px;padding-left:20px}
.bt-prose li{margin:2px 0}
.bt-prose code{font-family:var(--ava-font-mono);font-size:12px;padding:1px 5px;border-radius:4px;
  background:var(--ava-interactive-bg-hover);color:var(--ava-label-primary)}
.bt-prose b,.bt-prose strong{font-weight:650}

/* 派发卡：只描述在做什么，不暴露内部过程 */
.bt-dispatch{display:flex;align-items:center;gap:9px;padding:9px 11px;
  border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);background:var(--ava-bg-layer-1)}
.bt-dispatch .glyph{width:24px;height:24px;flex:0 0 auto;border-radius:6px;display:grid;place-items:center;
  font-size:12px;font-weight:650;color:var(--ava-state-business-primary);
  background:color-mix(in srgb,var(--ava-state-business-primary) 12%,transparent)}
.bt-dispatch .body{min-width:0;flex:1}
.bt-dispatch .name{font-size:12.5px;font-weight:550;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.bt-dispatch .meta{font-size:10.5px;color:var(--ava-label-tertiary);margin-top:2px;
  display:flex;gap:6px;align-items:center}
.bt-chip{font-size:10px;padding:1px 6px;border-radius:var(--ava-radius-pill);
  background:var(--ava-interactive-bg-hover);color:var(--ava-label-secondary)}
.bt-chip.ok{background:color-mix(in srgb,var(--ava-state-business-primary) 12%,transparent);color:var(--ava-state-business-primary)}
.bt-chip.warn{background:color-mix(in srgb,var(--ava-state-warning) 14%,transparent);color:var(--ava-state-warning)}
.bt-dispatch .arrow{color:var(--ava-label-tertiary);font-size:12px}
.bt-dispatch.clickable{cursor:pointer;transition:background .15s,border-color .15s}
.bt-dispatch.clickable:hover{background:var(--ava-interactive-bg-hover);border-color:var(--ava-border-l3)}
.bt-dispatch.clickable:hover .arrow{color:var(--ava-label-secondary)}
.bt-dispatch.clickable:focus-visible{outline:2px solid var(--ava-state-business-primary);outline-offset:1px}

/* 运行态活动条：阶段 + 计时 + 停止 */
.bt-activity{display:flex;align-items:center;gap:8px;padding:8px 11px;border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);border:0.5px solid var(--ava-border-l2);font-size:12px;
  color:var(--ava-label-secondary)}
.bt-activity .spin{width:11px;height:11px;flex:0 0 auto;border-radius:50%;
  border:1.5px solid color-mix(in srgb,var(--ava-state-business-primary) 30%,transparent);
  border-top-color:var(--ava-state-business-primary);animation:bt-spin .9s linear infinite}
@keyframes bt-spin{to{transform:rotate(360deg)}}
.bt-activity .phase{color:var(--ava-label-primary)}
.bt-activity .elapsed{margin-left:auto;font-variant-numeric:tabular-nums;
  color:var(--ava-label-tertiary);font-size:11px;font-family:var(--ava-font-mono)}
.bt-activity .stop{appearance:none;border:0.5px solid var(--ava-border-l2);background:transparent;
  color:var(--ava-label-secondary);font:inherit;font-size:11px;padding:2px 8px;
  border-radius:5px;cursor:pointer}
.bt-activity .stop:hover{color:var(--ava-label-primary)}

/* 升级卡：必达信息，保留强调 */
.bt-escalation{border:0.5px solid color-mix(in srgb,var(--ava-state-danger) 30%,var(--ava-border-l2));
  border-left:2.5px solid var(--ava-state-danger);border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);padding:9px 11px}
.bt-escalation .head{display:flex;align-items:center;gap:6px;font-size:11px;font-weight:650;
  color:var(--ava-state-danger)}
.bt-escalation .head .tag{border:0.5px solid var(--ava-state-danger);border-radius:4px;
  padding:0 4px;font-size:10px;font-weight:650}
.bt-escalation .body{margin-top:5px;font-size:12.5px;line-height:1.65}
.bt-escalation .body code{font-family:var(--ava-font-mono);font-size:11.5px;padding:1px 5px;
  border-radius:4px;background:var(--ava-interactive-bg-hover)}
.bt-escalation .acts{display:flex;gap:8px;margin-top:8px;flex-wrap:wrap}
.bt-escalation .acts button{appearance:none;border:0.5px solid var(--ava-border-l2);background:transparent;
  color:var(--ava-label-secondary);font:inherit;font-size:11px;padding:3px 9px;
  border-radius:5px;cursor:pointer}
.bt-escalation .acts button.go{border-color:var(--ava-state-danger);color:var(--ava-state-danger)}

/* 失败卡：本地行，带重试 */
.bt-failure{border:0.5px solid color-mix(in srgb,var(--ava-state-danger) 26%,var(--ava-border-l2));
  border-radius:var(--ava-radius-md);background:color-mix(in srgb,var(--ava-state-danger) 5%,var(--ava-bg-layer-1));
  padding:9px 11px;font-size:12px;line-height:1.6}
.bt-failure .head{color:var(--ava-state-danger);font-weight:600;font-size:11.5px}
.bt-failure .detail{color:var(--ava-label-secondary);margin-top:3px;word-break:break-word}
.bt-failure .acts{display:flex;gap:8px;margin-top:7px}
.bt-failure button{appearance:none;font:inherit;font-size:11px;cursor:pointer;
  padding:2px 9px;border-radius:5px;border:0.5px solid var(--ava-border-l2);
  background:transparent;color:var(--ava-label-secondary)}
.bt-failure button.primary{border-color:var(--ava-state-danger);color:var(--ava-state-danger)}

/* 执行结果：可折叠 */
.bt-result{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);overflow:hidden}
.bt-result .rh{display:flex;align-items:center;gap:8px;padding:8px 11px;font-size:12px;
  cursor:pointer;user-select:none}
.bt-result .rh .tick{width:16px;height:16px;border-radius:50%;display:grid;place-items:center;
  font-size:9px;color:#fff;background:var(--ava-state-success);flex:0 0 auto}
.bt-result .rh .name{flex:1;min-width:0;font-weight:550;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.bt-result .rh .fold{font-size:10.5px;color:var(--ava-label-tertiary)}
.bt-result .rbody{padding:0 11px 10px;font-size:12.5px;line-height:1.8;white-space:pre-wrap;
  color:var(--ava-label-secondary);word-break:break-word}
.bt-result.collapsed .rbody{display:none}

/* 晨报 */
.bt-morning{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);padding:10px 12px}
.bt-morning .hd{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:600;margin-bottom:6px}
.bt-morning .hd .sun{width:17px;height:17px;border-radius:50%;background:var(--ava-state-warning);
  color:#fff;display:grid;place-items:center;font-size:10px}
.bt-morning ul{margin:0;padding-left:18px;font-size:12px;color:var(--ava-label-secondary);line-height:1.9}

/* ── 输入区胶囊 ── */
.bt-composer{flex:0 0 auto;padding:8px 16px 10px;border-top:0.5px solid var(--ava-border-l2);
  background:var(--ava-bg-layer-1)}
.bt-capsule{display:flex;align-items:flex-end;gap:6px;padding:6px 6px 6px 12px;
  background:var(--ava-bg-base);border:0.5px solid transparent;border-radius:var(--ava-radius-lg);
  transition:border-color .15s,background .15s}
.bt-capsule:focus-within{background:var(--ava-bg-layer-1);
  border-color:color-mix(in srgb,var(--ava-state-business-primary) 45%,var(--ava-border-l2))}
.bt-capsule textarea{flex:1;min-width:0;border:0;outline:0;resize:none;background:transparent;
  font:inherit;font-size:13px;line-height:1.55;color:var(--ava-label-primary);
  max-height:92px;padding:4px 0;overflow-y:auto}
.bt-capsule textarea::placeholder{color:var(--ava-label-tertiary)}
.bt-send{width:26px;height:26px;border-radius:6px;border:0;flex:0 0 auto;cursor:pointer;
  background:var(--ava-state-business-primary);color:#fff;display:grid;place-items:center;padding:0;
  transition:opacity .15s,transform .1s}
.bt-send:hover{opacity:.9}
.bt-send:active{transform:scale(.95)}
.bt-send:disabled{opacity:.4;cursor:default;transform:none}
.bt-send svg{width:14px;height:14px}
.bt-hint{display:flex;align-items:center;justify-content:space-between;padding:5px 3px 0;
  font-size:10px;color:var(--ava-label-tertiary)}
.bt-hint .model{display:inline-flex;align-items:center;gap:4px}

/* ── 工作台侧栏 ── */
.bt-workbench{width:300px;flex:0 0 auto;border-left:0.5px solid var(--ava-border-l2);
  display:flex;flex-direction:column;min-height:0;background:var(--ava-bg-base)}
.bt-wb-head{height:40px;flex:0 0 auto;display:flex;align-items:center;gap:6px;padding:0 12px;
  border-bottom:0.5px solid var(--ava-border-l2)}
.bt-wb-title{font-size:12.5px;font-weight:600}
.bt-wb-body{flex:1;min-height:0;overflow-y:auto;padding:12px 10px 18px}
.bt-wb-section{margin-bottom:16px}
.bt-wb-section h4{margin:0 0 6px;padding-left:3px;font-size:10.5px;font-weight:600;
  letter-spacing:.05em;color:var(--ava-label-tertiary);display:flex;align-items:center;gap:6px}
.bt-wb-section h4 .count{font-weight:500;opacity:.9}
.bt-stats{display:grid;grid-template-columns:repeat(3,1fr);gap:6px;margin-bottom:8px}
.bt-stat{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);padding:7px 8px}
.bt-stat .n{font-size:15px;font-weight:650;font-variant-numeric:tabular-nums;letter-spacing:-.01em}
.bt-stat .k{font-size:10px;color:var(--ava-label-tertiary);margin-top:1px}
.bt-stat.alert .n{color:var(--ava-state-danger)}
.bt-wb-card{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  padding:9px 10px;margin-bottom:8px;cursor:pointer;transition:border-color .15s,background .15s}
.bt-wb-card:hover{border-color:var(--ava-label-tertiary)}
.bt-wb-card.on{border-color:color-mix(in srgb,var(--ava-state-business-primary) 55%,var(--ava-border-l2));
  background:color-mix(in srgb,var(--ava-state-business-primary) 4%,transparent)}
.bt-wb-card .top{display:flex;align-items:flex-start;gap:8px}
.bt-wb-av{width:22px;height:22px;border-radius:6px;flex:0 0 auto;margin-top:1px;
  display:grid;place-items:center;font-size:11px;font-weight:650;
  background:color-mix(in srgb,var(--ava-state-business-primary) 12%,transparent);
  color:var(--ava-state-business-primary)}
.bt-wb-card .name{font-size:12px;font-weight:550;line-height:1.4;
  display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}
.bt-wb-card .sub{margin-top:3px;font-size:10.5px;color:var(--ava-label-tertiary);
  display:flex;align-items:center;gap:6px}
.bt-wb-card .step{margin-top:7px;font-size:10.5px;color:var(--ava-label-secondary);
  display:flex;align-items:center;gap:6px}
.bt-wb-card .step .elapsed{margin-left:auto;font-variant-numeric:tabular-nums;
  color:var(--ava-label-tertiary);font-family:var(--ava-font-mono)}
.bt-wb-item{display:flex;align-items:center;gap:8px;padding:6px 8px;border-radius:6px;
  font-size:11.5px;color:var(--ava-label-primary);cursor:pointer}
.bt-wb-item:hover{background:var(--ava-interactive-bg-hover)}
.bt-wb-item .txt{flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.bt-wb-item .when{font-size:10px;color:var(--ava-label-tertiary);flex:0 0 auto}
.bt-empty{color:var(--ava-label-tertiary);font-size:11.5px;text-align:center;padding:16px 0}

/* ── 面板视图（台账 / 设置 / 纪律） ── */
.bt-panel{height:100%;overflow-y:auto;padding:var(--ava-space-4);font-size:13px}
.bt-sec-title{font-size:11px;font-weight:600;color:var(--ava-label-tertiary);
  letter-spacing:.04em;margin:0 0 var(--ava-space-2)}
.bt-set-row{display:flex;align-items:center;justify-content:space-between;padding:8px 0;
  border-bottom:0.5px solid var(--ava-border-l1);font-size:12px}
.bt-set-row .lbl{color:var(--ava-label-secondary)}
.bt-switch{width:32px;height:18px;border-radius:999px;background:var(--ava-border-l3);
  position:relative;cursor:pointer;flex:0 0 auto}
.bt-switch.on{background:var(--ava-state-success)}
.bt-switch::after{content:"";position:absolute;top:2px;left:2px;width:14px;height:14px;
  border-radius:50%;background:#fff;transition:left .15s}
.bt-switch.on::after{left:16px}
.bt-seg{display:flex;border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-pill);overflow:hidden}
.bt-seg button{flex:1;height:24px;border:0;background:transparent;font:inherit;font-size:10.5px;
  cursor:pointer;color:var(--ava-label-secondary);white-space:nowrap;padding:0 8px}
.bt-seg button.on{background:var(--ava-nav-active);color:var(--ava-label-primary);font-weight:600}
.bt-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:3px 0}
.bt-kv b{color:var(--ava-label-primary);font-weight:600}
.bt-kv b.no{color:var(--ava-state-danger)}
.bt-note{font-size:11px;color:var(--ava-label-tertiary);line-height:1.6}
.bt-banner{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  padding:8px 12px;background:var(--ava-interactive-bg-hover);color:var(--ava-label-secondary);
  font-size:12px;margin-bottom:12px}

/* 窄容器隐藏工作台 */
@media (max-width: 900px){ .bt-workbench{display:none} }
`;
