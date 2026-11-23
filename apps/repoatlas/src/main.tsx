// 代码考古（RepoAtlas）：真实影响面分析 App。
// 用户旅程：选仓库（listDirs 顶层目录）→ 仓库体检（子目录/文件数）→ 输入目标符号
// → 扫描仓库真实文件内容统计引用 → 按命中聚合影响面（直接/测试/间接）。
// 全部真实数据，无 mock；扫描上限 80 个文件并诚实标注已扫数量。
// 扫描完成后经 storage("scan") + events("repoatlas.scan") 广播，画布免重复扫描。
import type { AppContext, DirEntry } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import { Canvas } from "./canvas";
import { CanvasPanel, SidebarPanel } from "./panels";
import { Styles } from "./styles";
import { listWorkdirs, scanRepo } from "./lib";
import type { ScanPayload, ScanResult, Workdir } from "./lib";

/** 上次选择记忆（storage key "last"）：工作区 / 仓库路径 / 目标符号。 */
interface LastPick {
  workdir: string;
  root: string;
  symbol: string;
}

function Main({ ctx }: { ctx: AppContext }) {
  const { agent, storage, events } = ctx;
  const [workdirs, setWorkdirs] = useState<Workdir[]>([]);
  const [workdir, setWorkdir] = useState("");
  const [sessionId, setSessionId] = useState<string | undefined>(undefined);
  const [subRepos, setSubRepos] = useState<DirEntry[]>([]);
  const [root, setRoot] = useState("");
  const [symbol, setSymbol] = useState("refund");
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [subdirs, setSubdirs] = useState<DirEntry[]>([]);
  const [rootFiles, setRootFiles] = useState(0);
  const [hydrated, setHydrated] = useState(false);

  // 旅程第 2 步：选工作区 → 列出其子目录作为仓库候选
  const selectWorkdir = useCallback(async (w: Workdir) => {
    setWorkdir(w.workingDir);
    setSessionId(w.sessionId);
    setResult(null);
    try {
      const listing = await agent.listDirs(w.workingDir, { files: true });
      setSubRepos(listing.entries || []);
      setSubdirs(listing.entries || []);
      setRootFiles((listing.files || []).length);
      setRoot(w.workingDir);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setSubRepos([]);
      setSubdirs([]);
      setRootFiles(0);
    }
  }, [agent]);

  // 旅程第 3 步：选仓库 → 体检其结构
  const selectRepo = useCallback(async (path: string) => {
    setRoot(path);
    setResult(null);
    try {
      const listing = await agent.listDirs(path, { files: true });
      setSubdirs(listing.entries || []);
      setRootFiles((listing.files || []).length);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setSubdirs([]);
      setRootFiles(0);
    }
  }, [agent]);

  // 旅程第 1 步：枚举可读工作区（会话权限根去重），并恢复上次选择（storage "last"）
  const initWorkdirs = useCallback(async () => {
    const list = await listWorkdirs(agent);
    setWorkdirs(list);
    if (list.length === 0) {
      setError("未找到会话工作区；请先在对话页打开一个会话。");
      return;
    }
    const last = await storage.get<LastPick>("last");
    const target = (last && list.find((w) => w.workingDir === last.workdir)) || list[0];
    await selectWorkdir(target);
    if (last?.root && last.root !== target.workingDir) await selectRepo(last.root);
    if (last?.symbol) setSymbol(last.symbol);
    setHydrated(true);
  }, [agent, storage, selectWorkdir, selectRepo]);

  useEffect(() => { void initWorkdirs(); }, [initWorkdirs]);

  // 记忆上次选择：恢复完成后，workdir/root/symbol 任何变化都持久化
  useEffect(() => {
    if (!hydrated) return;
    void storage.set("last", { workdir, root, symbol } satisfies LastPick);
  }, [hydrated, workdir, root, symbol, storage]);

  // 刷新：重新枚举工作区 + 重新体检当前仓库（优先保持现有选择）
  const refresh = useCallback(async () => {
    const list = await listWorkdirs(agent);
    setWorkdirs(list);
    if (list.length === 0) {
      setError("未找到会话工作区；请先在对话页打开一个会话。");
      return;
    }
    const cur = list.find((w) => w.workingDir === workdir) || list[0];
    await selectWorkdir(cur);
    if (root && root !== cur.workingDir) await selectRepo(root);
  }, [agent, workdir, root, selectWorkdir, selectRepo]);

  // 旅程第 4 步：输入符号 → 真实扫描，完成后写 storage 并广播（画布免重复扫描）
  const analyze = useCallback(async () => {
    if (!root || !symbol.trim() || scanning) return;
    setScanning(true);
    setResult(null);
    const r = await scanRepo(agent, root, symbol, sessionId);
    setResult(r);
    setScanning(false);
    const payload: ScanPayload = { root, symbol: symbol.trim(), result: r };
    await storage.set("scan", payload);
    events.emit("repoatlas.scan", payload);
  }, [agent, storage, events, root, symbol, scanning, sessionId]);

  return (
    <div className="ra-root">
      <Styles />
      <div className="ra-body">
        <div className="ra-bar">
          <label>工作区</label>
          <select className="ra-select" value={workdir} onChange={(e) => {
            const w = workdirs.find((x) => x.workingDir === e.target.value);
            if (w) void selectWorkdir(w);
          }}>
            {workdirs.length === 0 && <option value="">无可用工作区</option>}
            {workdirs.map((w) => <option key={w.workingDir} value={w.workingDir}>{w.workingDir}</option>)}
          </select>
          <button className="ra-btn-ghost" onClick={() => void refresh()}>刷新</button>
          <label>仓库</label>
          <select className="ra-select" value={root} onChange={(e) => void selectRepo(e.target.value)}>
            {root === workdir && <option value={workdir}>（工作区根）</option>}
            {subRepos.map((d) => <option key={d.path} value={d.path}>{d.name}</option>)}
          </select>
        </div>
        <div className="ra-bar" style={{ marginTop: 8 }}>
          <label>目标符号</label>
          <input className="ra-query" value={symbol} placeholder="如 apply_refund / TodoItem / setup" onChange={(e) => setSymbol(e.target.value)} onKeyDown={(e) => e.key === "Enter" && analyze()} />
          <button className="ra-btn" onClick={analyze} disabled={!root || !symbol.trim() || scanning}>
            {scanning ? "扫描中…" : "分析影响面"}
          </button>
        </div>
        {root && (
          <div className="ra-meta">扫描范围：{root}　·　子目录 {subdirs.length} 个 · 文件 {rootFiles} 个</div>
        )}
        {error ? (
          <div className="ra-err" style={{ marginTop: 12 }}>
            {error}
            <div style={{ marginTop: 8 }}>
              <button className="ra-btn-ghost" onClick={() => void initWorkdirs()}>重试</button>
            </div>
          </div>
        ) : null}

        {scanning ? (
          <div className="ra-empty" style={{ padding: "var(--ava-space-5) 0" }}><span className="ra-spin" />正在读取仓库文件内容并统计引用…</div>
        ) : result ? (
          <>
            <div className="ra-summary">
              <div className="ra-metric"><b>{result.direct.length}</b><span>直接引用文件</span></div>
              <div className="ra-metric"><b>{result.tests.length}</b><span>测试引用</span></div>
              <div className="ra-metric"><b>{result.indirect.length}</b><span>模块内波及</span></div>
              <div className="ra-metric"><b>{result.byDir.length}</b><span>命中目录</span></div>
              <div className="ra-metric"><b>{result.scanned}/{result.totalFiles}</b><span>已扫描文件</span></div>
            </div>
            {result.direct.length > 0 && (
              <div className="ra-risk">
                <span className="ra-tag bad">影响面</span>
                <span>符号「{result.symbol}」被 {result.direct.length} 个源文件直接引用，改动需回归这些文件。</span>
              </div>
            )}

            <p className="ra-sec-title">直接引用 · {result.direct.length}</p>
            {result.direct.length === 0 ? <div className="ra-empty">未找到引用该符号的源文件。</div> : result.direct.map((f) => (
              <div className="ra-row" key={f.path}>
                <div className="lvl direct">直接</div>
                <div className="body">
                  <div className="file">{f.path}</div>
                  {f.sample.length > 0 && <div className="note">例：{f.sample[0].slice(0, 80)}</div>}
                </div>
                <div className="hits">{f.hits} 处</div>
              </div>
            ))}

            {result.tests.length > 0 && (
              <>
                <p className="ra-sec-title">测试引用 · {result.tests.length}</p>
                {result.tests.map((f) => (
                  <div className="ra-row" key={f.path}>
                    <div className="lvl test">测试</div>
                    <div className="body"><div className="file">{f.path}</div></div>
                    <div className="hits">{f.hits} 处</div>
                  </div>
                ))}
              </>
            )}

            {result.indirect.length > 0 && (
              <>
                <p className="ra-sec-title">模块内波及（同目录未命中）· {result.indirect.length}</p>
                {result.indirect.map((f) => (
                  <div className="ra-row" key={f.path}>
                    <div className="lvl indirect">波及</div>
                    <div className="body"><div className="file">{f.path}</div></div>
                  </div>
                ))}
              </>
            )}

            {result.direct.length === 0 && result.tests.length === 0 && (
              <div className="ra-empty">该符号在 {result.scanned} 个文件中无引用。换一个符号重试，或确认仓库选择正确。</div>
            )}
          </>
        ) : (
          <div className="ra-empty">
            {root ? "输入要追溯的函数名或类型名，点击「分析影响面」即可查看它被哪些文件引用。\n当前仓库：子目录 " + subdirs.length + " 个 · 文件 " + rootFiles + " 个" : "请先选择一个仓库。"}
          </div>
        )}
      </div>
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 11, viewId: "repoatlas", title: "代码考古",
      icon: "M12 2a10 10 0 0 1 10 10 10 10 0 0 1-10 10A10 10 0 0 1 2 12 10 10 0 0 1 12 2zm0 4a6 6 0 0 0-6 6 6 6 0 0 0 6 6 6 6 0 0 0 6-6 6 6 0 0 0-6-6zM12 8a4 4 0 0 1 4 4 4 4 0 0 1-4 4 4 4 0 0 1-4-4 4 4 0 0 1 4-4z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "repoatlas", title: "代码考古",
      component: async () => ({ default: () => <Main ctx={ctx} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "repo-atlas", title: "架构地图", description: "真实目录命中热度",
      component: async () => ({ default: () => <Canvas ctx={ctx} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 11, viewId: "repoatlas", title: "分析旅程",
      component: async () => ({ default: SidebarPanel }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 11, viewId: "repo-atlas", title: "分级口径",
      component: async () => ({ default: CanvasPanel }),
    }),
  ];
  return { dispose() { for (const d of disposers) d.dispose(); } };
}
