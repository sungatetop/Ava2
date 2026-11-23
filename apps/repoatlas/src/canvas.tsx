// RepoAtlas 画布：架构地图。默认消费主区广播的扫描结果（storage 兜底恢复晚挂载场景），
// 本地扫描按钮仅作无广播时的兜底。
import type { AppContext, DirEntry } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import { Styles } from "./styles";
import { ACCENT, listWorkdirs, scanRepo } from "./lib";
import type { ScanPayload, ScanResult, Workdir } from "./lib";

export function Canvas({ ctx }: { ctx: AppContext }) {
  const { agent, storage, events } = ctx;
  const [state, setState] = useState<{ workdirs: Workdir[]; workdir: string; sessionId?: string; subRepos: DirEntry[]; root: string; result: ScanResult | null; error: string | null }>({ workdirs: [], workdir: "", sessionId: undefined, subRepos: [], root: "", result: null, error: null });
  const [symbol, setSymbol] = useState("refund");
  const [scanning, setScanning] = useState(false);

  // 订阅主区扫描广播：root/symbol/result 同步，直接渲染架构图与热度（免重复扫描）
  useEffect(() => {
    const d = events.on("repoatlas.scan", (e) => {
      const p = e as ScanPayload | null;
      if (!p || typeof p.root !== "string") return;
      setState((st) => ({ ...st, root: p.root, result: p.result }));
      setSymbol(p.symbol);
    });
    return () => d.dispose();
  }, [events]);

  const selectWorkdir = useCallback(async (w: Workdir) => {
    try {
      const listing = await agent.listDirs(w.workingDir);
      setState((st) => ({ ...st, workdir: w.workingDir, sessionId: w.sessionId, root: w.workingDir, subRepos: listing.entries || [], result: null, error: null }));
    } catch (e) {
      setState((st) => ({ ...st, error: e instanceof Error ? e.message : String(e) }));
    }
  }, [agent]);

  const initWorkdirs = useCallback(async () => {
    const list = await listWorkdirs(agent);
    setState((st) => ({ ...st, workdirs: list }));
    if (list.length === 0) {
      setState((st) => ({ ...st, error: "未找到会话工作区。" }));
      return;
    }
    // storage 兜底恢复（画布晚挂载：主区早已完成扫描）——有结果则不自动选工作区，避免清掉结果
    const saved = await storage.get<ScanPayload>("scan");
    if (saved?.result && typeof saved.root === "string") {
      setState((st) => (st.result ? st : { ...st, root: saved.root, result: saved.result }));
      setSymbol(saved.symbol);
      return;
    }
    await selectWorkdir(list[0]);
  }, [agent, storage, selectWorkdir]);
  useEffect(() => { void initWorkdirs(); }, [initWorkdirs]);

  const selectRepo = useCallback(async (path: string) => {
    setState((s) => ({ ...s, root: path, result: null }));
  }, []);

  // 兜底扫描：仅在无广播可消费时手动触发
  const analyze = useCallback(async () => {
    if (!state.root || !symbol.trim() || scanning) return;
    setScanning(true);
    const r = await scanRepo(agent, state.root, symbol, state.sessionId);
    setState((s) => ({ ...s, result: r }));
    setScanning(false);
  }, [agent, state.root, state.sessionId, symbol, scanning]);

  return (
    <div className="ra-root">
      <Styles />
      <div className="ra-canvas">
        <div className="ra-bar">
          <select className="ra-select" value={state.workdir} onChange={(e) => {
            const w = state.workdirs.find((x) => x.workingDir === e.target.value);
            if (w) void selectWorkdir(w);
          }}>
            {state.workdirs.length === 0 && <option value="">无工作区</option>}
            {state.workdirs.map((w) => <option key={w.workingDir} value={w.workingDir}>{w.workingDir}</option>)}
          </select>
          <select className="ra-select" value={state.root} onChange={(e) => void selectRepo(e.target.value)}>
            {state.root === state.workdir && <option value={state.workdir}>（工作区根）</option>}
            {state.root && state.root !== state.workdir && !state.subRepos.some((d) => d.path === state.root) && (
              <option value={state.root}>{state.root.split(/[\\/]/).pop()}</option>
            )}
            {state.subRepos.map((d) => <option key={d.path} value={d.path}>{d.name}</option>)}
          </select>
          <input className="ra-query" value={symbol} placeholder="符号" onChange={(e) => setSymbol(e.target.value)} />
          <button className="ra-btn" onClick={analyze} disabled={!state.root || !symbol.trim() || scanning}>{scanning ? "扫描…" : "分析"}</button>
        </div>
        {state.error ? <div className="ra-err" style={{ marginTop: 8 }}>{state.error}</div> : null}
        {state.result && (
          <>
            <div className="ra-mapwrap" style={{ marginTop: 10 }}>
              <svg viewBox="0 0 380 300" style={{ width: "100%", height: "auto" }}>
                {(() => {
                  const dirs = state.result!.byDir;
                  const shown = dirs.slice(0, 8);
                  const overflow = dirs.length - shown.length;
                  // 圆环槽位数：目录 ≤8 个时即目录数；超出时 "+N 更多目录" 占第 9 槽
                  const n = shown.length + (overflow > 0 ? 1 : 0);
                  const ring = (i: number) => {
                    const th = (2 * Math.PI * i) / n - Math.PI / 2;
                    return { cx: 190 + 100 * Math.cos(th), cy: 150 + 100 * Math.sin(th) };
                  };
                  return (
                    <>
                      {shown.map((d, i) => {
                        const { cx, cy } = ring(i);
                        const hot = i === 0;
                        return (
                          <g key={d.name}>
                            <path className={"ra-edge" + (hot ? " hot" : "")} d={`M190,150 L${cx},${cy}`} />
                            <g className={"ra-module" + (hot ? " sel" : "")}>
                              <rect x={cx - 60} y={cy - 17} width="120" height="34" rx="7" />
                              <text x={cx - 48} y={cy + 4}>{d.name.length > 13 ? d.name.slice(0, 13) + "…" : d.name}</text>
                              <text x={cx - 48} y={cy + 28} style={{ fontSize: 8.5, fill: "var(--ava-label-tertiary)", fontFamily: "var(--ava-font-mono)" }}>{d.hits} 处命中</text>
                            </g>
                          </g>
                        );
                      })}
                      {overflow > 0 && (() => {
                        const { cx, cy } = ring(shown.length);
                        return (
                          <text x={cx} y={cy + 3} textAnchor="middle" style={{ fontSize: 9.5, fill: "var(--ava-label-tertiary)", fontFamily: "var(--ava-font-mono)" }}>+{overflow} 更多目录</text>
                        );
                      })()}
                    </>
                  );
                })()}
                <circle cx="190" cy="150" r="16" fill={ACCENT} />
                <text x="190" y="155" textAnchor="middle" style={{ fontSize: 10, fill: "#fff", fontWeight: 700, fontFamily: "var(--ava-font-mono)" }}>{state.result.symbol.slice(0, 6)}</text>
              </svg>
            </div>
            <div className="ra-legend">
              <span>— 目录内命中数（热边 = 命中最多）</span>
            </div>
            <div style={{ height: "0.5px", background: "var(--ava-border-l2)", margin: "var(--ava-space-3) 0" }} />
            <p className="ra-sec-title">目录命中热度（真实）</p>
            {(() => {
              const byDir = state.result!.byDir;
              const max = Math.max(...byDir.map((x) => x.hits)) || 1;
              return byDir.length === 0 ? (
                <div className="ra-empty">无命中目录</div>
              ) : (
                byDir.slice(0, 6).map((d) => (
                  <div className="ra-heat" key={d.name}>
                    <span className="lbl">{d.name}</span>
                    <div className="ra-heat-bar"><i style={{ width: Math.round((d.hits / max) * 100) + "%" }} /></div>
                    <span className="ra-mono" style={{ fontSize: 10.5, color: "var(--ava-label-tertiary)" }}>{d.hits}</span>
                  </div>
                ))
              );
            })()}
          </>
        )}
      </div>
    </div>
  );
}
