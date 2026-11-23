// 数据叙事台（Insight）主区 slot：读工作区 CSV → 一句话结论卡（运营分析场景）。
// 真实调用 ctx.agent.listDirs / readWorkspaceFile 读 CSV，本地解析统计（不依赖后端）。
// 无数据即空态（不使用 mock）；storage 记忆上次工作区与选择，重开自动恢复。
import type { AgentChannel, AppContext, FileEntry } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import {
  SELECT_EVENT, computeInsights, fmtNum, listWorkdirs, parseCsv,
  type Dataset, type Events, type Selection, type StorageLike, type Workdir,
} from "./lib";
import { Styles } from "./styles";
import { Canvas } from "./canvas";
import { CanvasPanel, SidebarPanel } from "./panels";

/** 主区数据流：工作区/CSV 枚举 + 读取选中数据集（真实工作区文件，零 mock）。 */
function useDataset(agent: AgentChannel, events: Events, storage: StorageLike) {
  const [workdirs, setWorkdirs] = useState<Workdir[]>([]);
  const [workdir, setWorkdir] = useState<Workdir | null>(null);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [dataset, setDataset] = useState<Dataset | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  /** 列出工作区内 CSV：根 + 一层 + 二层子目录（真实枚举，覆盖常见 docs/report 结构）。 */
  const loadFiles = useCallback(async (w: Workdir) => {
    setLoading(true);
    const found: FileEntry[] = [];
    const pushCsv = (list?: FileEntry[]) => {
      for (const f of list || []) {
        if (f.name.toLowerCase().endsWith(".csv") && !found.some((x) => x.path === f.path)) found.push(f);
      }
    };
    try {
      const root = await agent.listDirs(w.workingDir, { files: true });
      pushCsv(root.files);
      for (const d1 of (root.entries || []).slice(0, 10)) {
        if (found.length >= 60) break;
        try {
          const sub1 = await agent.listDirs(d1.path, { files: true });
          pushCsv(sub1.files);
          for (const d2 of (sub1.entries || []).slice(0, 4)) {
            if (found.length >= 60) break;
            try {
              const sub2 = await agent.listDirs(d2.path, { files: true });
              pushCsv(sub2.files);
            } catch {
              /* 跳过不可读目录 */
            }
          }
        } catch {
          /* 跳过不可读目录 */
        }
      }
      setFiles(found);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setFiles([]);
    } finally {
      setLoading(false);
    }
  }, [agent]);

  const pick = useCallback(async (f: FileEntry) => {
    if (!workdir) return;
    setLoading(true);
    try {
      const res = await agent.readWorkspaceFile(f.path, workdir.sessionId);
      const text = res.text ?? "";
      const ds = { name: f.name, csv: text, parsed: parseCsv(text) };
      setDataset(ds);
      setError(null);
      // 广播给画布/面板（跨 slot 共享真实数据）+ 持久化（晚挂载恢复）
      const s: Selection = {
        workingDir: workdir.workingDir,
        sessionId: workdir.sessionId,
        path: f.path,
        name: f.name,
      };
      await storage.set("selection", s).catch(() => {});
      events.emit(SELECT_EVENT, s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [agent, workdir, events]);

  const selectWorkdir = useCallback(async (w: Workdir) => {
    setWorkdir(w);
    setDataset(null);
    // 记住上次工作区（init 时优先恢复）
    await storage.set("workdir", w.workingDir).catch(() => {});
    await loadFiles(w);
  }, [loadFiles, storage]);

  /** 刷新当前工作区 CSV 列表（供「刷新」按钮）。 */
  const refresh = useCallback(async () => {
    if (workdir) await loadFiles(workdir);
  }, [workdir, loadFiles]);

  const init = useCallback(async () => {
    const list = await listWorkdirs(agent);
    setWorkdirs(list);
    if (list.length === 0) {
      setError("未找到会话工作区；请先在对话页打开一个会话。");
      return;
    }
    // 优先恢复上次选择的工作区（storage 记忆），否则回退第一个
    const saved = await storage.get<string>("workdir").catch(() => undefined);
    const restored = saved ? list.find((w) => w.workingDir === saved) : undefined;
    await selectWorkdir(restored ?? list[0]);
  }, [agent, selectWorkdir, storage]);

  useEffect(() => { void init(); }, [init]);

  return { workdirs, workdir, files, dataset, error, loading, selectWorkdir, pick, refresh, retry: init };
}

function Main({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const { workdirs, workdir, files, dataset, error, loading, selectWorkdir, pick, refresh, retry } = useDataset(agent, events, storage);
  const ins = dataset ? computeInsights(dataset.parsed) : null;
  return (
    <div className="in-root">
      <Styles />
      <div className="in-body">
        <div className="in-dataset" style={{ flexWrap: "wrap" }}>
          <div className="in-file">CSV</div>
          <div style={{ flex: 1, minWidth: 140 }}>
            <div style={{ fontWeight: 600 }}>{dataset ? dataset.name : "未选择数据集"}</div>
            <div className="in-mono" style={{ color: "var(--ava-label-tertiary)", fontSize: 11 }}>
              {ins ? `${ins.rowCount} 行 · ${ins.headers.length} 列` : loading ? "读取中…" : "无数据"}
            </div>
          </div>
          <select
            style={{ height: 28, maxWidth: 260, borderRadius: 8, border: "0.5px solid var(--ava-border-l2)", background: "var(--ava-bg-base)", color: "var(--ava-label-primary)", fontSize: 12, padding: "0 6px" }}
            value={workdir ? workdir.workingDir : ""}
            onChange={(e) => {
              const w = workdirs.find((x) => x.workingDir === e.target.value);
              if (w) void selectWorkdir(w);
            }}
          >
            {workdirs.length === 0 && <option value="">无可用工作区</option>}
            {workdirs.map((w) => <option key={w.workingDir} value={w.workingDir}>{w.workingDir}</option>)}
          </select>
          <button className="in-btn" title="重新枚举当前工作区的 CSV 文件" disabled={loading || !workdir} onClick={() => void refresh()}>刷新</button>
          <select
            style={{ height: 28, maxWidth: 240, borderRadius: 8, border: "0.5px solid var(--ava-border-l2)", background: "var(--ava-bg-base)", color: "var(--ava-label-primary)", fontSize: 12, padding: "0 6px" }}
            value=""
            onChange={(e) => { const f = files.find((x) => x.path === e.target.value); if (f) void pick(f); }}
            disabled={files.length === 0}
          >
            <option value="">{files.length === 0 ? "该工作区无 CSV 文件" : `选择 CSV（${files.length}）…`}</option>
            {files.map((f) => <option key={f.path} value={f.path}>{f.name}</option>)}
          </select>
        </div>

        {error ? (
          <div className="in-err" style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ flex: 1 }}>{error}</span>
            <button className="in-btn" onClick={() => void retry()}>重试</button>
          </div>
        ) : null}

        {ins ? (
          <>
            <div className="in-insight">
              <span className="in-chip">
                结论 1 · {ins.valid / Math.max(1, ins.rowCount) >= 0.95 ? "高" : ins.valid / Math.max(1, ins.rowCount) >= 0.7 ? "中" : "低"}置信
              </span>
              <h3>该数据共 {ins.rowCount} 行，{ins.numName} 均值 {fmtNum(ins.avg)}，区间 [{fmtNum(ins.min)}, {fmtNum(ins.max)}]</h3>
              <div className="in-num">{fmtNum(ins.avg)}<small>{ins.numName} 均值</small></div>
              <div className="in-detail">
                样本 {ins.rowCount} 行，{ins.numName} 列有 {ins.valid} 个有效数值（覆盖 {Math.round((ins.valid / Math.max(1, ins.rowCount)) * 100)}%），空值与不可解析值已剔除。
              </div>
              <div className="in-conf">
                <span className="lbl">数值覆盖</span>
                <div className="in-bar"><i style={{ width: Math.round((ins.valid / Math.max(1, ins.rowCount)) * 100) + "%" }} /></div>
                <span className="in-mono">{Math.round((ins.valid / Math.max(1, ins.rowCount)) * 100)}%</span>
              </div>
            </div>
            <div className="in-insight">
              <span className="in-chip" style={{ color: "var(--ava-state-warning)", background: "color-mix(in srgb,var(--ava-state-warning) 12%,transparent)" }}>结论 2 · 需人工确认</span>
              <h3>检出 {ins.spikes} 条疑似异常尖峰（{ins.numName} &gt; 均值 2 倍）</h3>
              <div className="in-num">{ins.spikes}<small>疑似异常行</small></div>
              <div className="in-detail">
                占全样本 {Math.round((ins.spikes / Math.max(1, ins.rowCount)) * 100)}%。异常行数值显著偏离均值，疑似录入占位或重复；建议核对原始记录后决定是否剔除。
              </div>
              <div className="in-conf">
                <span className="lbl">异常占比</span>
                <div className="in-bar"><i style={{ width: Math.min(100, Math.round((ins.spikes / Math.max(1, ins.rowCount)) * 100)) + "%" }} /></div>
                <span className="in-mono">{ins.spikes}/{ins.rowCount}</span>
              </div>
            </div>
          </>
        ) : dataset ? (
          <div className="in-empty">无法解析该 CSV，请确认首行为表头。</div>
        ) : (
          <div className="in-empty">
            {loading ? "正在读取工作区 CSV 列表…" : "请选择工作区并挑选一个 CSV 文件开始分析。"}
          </div>
        )}
      </div>
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 10, viewId: "insight", title: "数据叙事台",
      icon: "M3 3v18h18V3H3zm16 16H5V5h14v14zM7 13h4v4H7v-4zm6-6h4v10h-4V7z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "insight", title: "数据叙事台",
      component: async () => ({ default: () => <Main agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "insight-charts", title: "图表墙", description: "所选 CSV 的真实分布与走势",
      component: async () => ({ default: () => <Canvas agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 10, viewId: "insight", title: "数据体检",
      component: async () => ({ default: () => <SidebarPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 10, viewId: "insight-charts", title: "口径说明",
      component: async () => ({ default: () => <CanvasPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
  ];
  return { dispose() { for (const d of disposers) d.dispose(); } };
}
