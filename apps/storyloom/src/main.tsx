// 故事织机（Storyloom）：真实文稿解析 App。
// 用户旅程：选工作区 → 选文稿文件（md/txt）→ 真实切分为节拍 → 节拍板显示真实字数
// → 稿纸显示选中节拍真实正文 → 画布按真实人名频次绘制角色关系 → 下面板真实张力曲线。
// 全部数据来自 readWorkspaceFile 的真实文件内容，无 mock。
import type { AgentChannel, AppContext, FileEntry } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import type { Script, Workdir } from "./lib";
import { listScripts, listWorkdirs, parseScript } from "./lib";
import { Styles } from "./styles";
import { Canvas } from "./canvas";
import { CanvasPanel, SidebarPanel } from "./panels";

type Events = AppContext["events"];
type StorageLike = AppContext["storage"];

/** 主区：工作区/文稿选择 + 节拍板 + 稿纸。 */
function Main({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [workdirs, setWorkdirs] = useState<Workdir[]>([]);
  const [workdir, setWorkdir] = useState<Workdir | null>(null);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [script, setScript] = useState<Script | null>(null);
  const [active, setActive] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadFiles = useCallback(async (w: Workdir) => {
    setLoading(true);
    try {
      const list = await listScripts(agent, w);
      setFiles(list);
      setError(list.length === 0 ? "该工作区未找到 .md / .txt 文稿。" : null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setFiles([]);
    } finally {
      setLoading(false);
    }
  }, [agent]);

  const selectWorkdir = useCallback(async (w: Workdir) => {
    setWorkdir(w);
    setScript(null);
    // 记忆本次工作区选择，下次进入优先恢复。
    await storage.set("workdir", { workingDir: w.workingDir, sessionId: w.sessionId }).catch(() => {});
    await loadFiles(w);
  }, [loadFiles, storage]);

  const open = useCallback(async (f: FileEntry) => {
    if (!workdir) return;
    setLoading(true);
    try {
      const res = await agent.readWorkspaceFile(f.path, workdir.sessionId);
      const parsed = parseScript(f.name, f.path, res.text ?? "");
      setScript(parsed);
      setActive(0);
      setError(parsed.beats.length === 0 ? "该文稿为空或无法切分。" : null);
      const payload = {
        workingDir: workdir.workingDir, sessionId: workdir.sessionId,
        path: f.path, name: f.name, beats: parsed.beats, chars: parsed.chars,
      };
      await storage.set("script", payload).catch(() => {});
      events.emit("storyloom.open", payload);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [agent, workdir, events, storage]);

  const init = useCallback(async () => {
    const list = await listWorkdirs(agent);
    setWorkdirs(list);
    if (list.length === 0) {
      setError("未找到会话工作区；请先在对话页打开一个会话。");
      return;
    }
    // 优先恢复上次选择的工作区（storage 记忆），已失效则回落第一个。
    const saved = await storage.get<Workdir>("workdir").catch(() => undefined);
    const restored = saved ? list.find((w) => w.workingDir === saved.workingDir) : undefined;
    await selectWorkdir(restored ?? list[0]);
  }, [agent, selectWorkdir, storage]);

  useEffect(() => { void init(); }, [init]);

  const beat = script && script.beats[active] ? script.beats[active] : null;

  return (
    <div className="sl-root">
      <Styles />
      <div className="sl-picker">
        <label>工作区</label>
        <select className="sl-select" value={workdir?.workingDir ?? ""} onChange={(e) => {
          const w = workdirs.find((x) => x.workingDir === e.target.value);
          if (w) void selectWorkdir(w);
        }}>
          {workdirs.length === 0 && <option value="">无可用工作区</option>}
          {workdirs.map((w) => <option key={w.workingDir} value={w.workingDir}>{w.workingDir}</option>)}
        </select>
        <label>文稿</label>
        <select className="sl-select" value="" onChange={(e) => {
          const f = files.find((x) => x.path === e.target.value);
          if (f) void open(f);
        }} disabled={files.length === 0}>
          <option value="">{files.length === 0 ? "无文稿文件" : `选择文稿（${files.length}）…`}</option>
          {files.map((f) => <option key={f.path} value={f.path}>{f.name}</option>)}
        </select>
        <button className="sl-btn" disabled={!workdir || loading}
          onClick={() => { if (workdir) void loadFiles(workdir); }}>刷新</button>
        {script && (
          <span className="sl-meta">{script.beats.length} 拍 · {script.totalWords} 字</span>
        )}
      </div>

      {error ? (
        <div style={{ padding: "var(--ava-space-4)" }}>
          <div className="sl-err">{error}</div>
          <div style={{ marginTop: 8 }}>
            <button className="sl-btn" onClick={() => void (workdir ? loadFiles(workdir) : init())}>重试</button>
          </div>
        </div>
      ) : null}

      {loading && !script ? (
        <div className="sl-empty" style={{ flex: 1 }}><span className="sl-spin" />正在读取并解析文稿…</div>
      ) : script && beat ? (
        <div className="sl-split">
          <div className="sl-beats">
            <p className="sl-sec-title">章节节拍 · 真实切分</p>
            {script.beats.map((b, i) => (
              <div key={b.no} className={"sl-beat" + (i === active ? " active" : "")} onClick={() => setActive(i)}>
                <div className="n">{b.no}</div>
                <h4>{b.title}</h4>
                <div className="sl-stress">
                  {[0, 1, 2, 3, 4].map((k) => <i key={k} className={b.ratio * 5 > k ? "on" : ""} />)}
                </div>
                <div className="sl-budget">{b.words} 字</div>
              </div>
            ))}
          </div>
          <div className="sl-paper">
            <span className="sl-speaker">{beat.no} · {beat.words} 字</span>
            <p className="sl-dialog">{beat.text}</p>
            <div className="sl-lock">
              <div style={{ marginBottom: 6 }}><span className="sl-chip">本拍边界</span></div>
              <div className="sl-kv"><span>文稿</span><b>{script.name}</b></div>
              <div className="sl-kv"><span>路径</span><b>{script.path}</b></div>
              <div className="sl-kv"><span>全篇</span><b>{script.beats.length} 拍 · {script.totalWords} 字</b></div>
              <div className="sl-kv"><span>本拍占比</span><b>{Math.round((beat.words / Math.max(1, script.totalWords)) * 100)}%</b></div>
            </div>
          </div>
        </div>
      ) : (
        <div className="sl-empty" style={{ flex: 1 }}>请选择工作区并挑选一个文稿文件开始。</div>
      )}
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 8, viewId: "storyloom", title: "故事织机",
      icon: "M12 2a3 3 0 0 1 3 3v1.2a3 3 0 0 1 3 3V19a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3V9.2a3 3 0 0 1 3-3V5a3 3 0 0 1 3-3zM8 6.2h8V5a1 1 0 0 0-1-1H9a1 1 0 0 0-1 1v1.2z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "storyloom", title: "故事织机",
      component: async () => ({ default: () => <Main agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "story-map", title: "角色关系图", description: "按真实人名频次绘制",
      component: async () => ({ default: () => <Canvas agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 8, viewId: "storyloom", title: "节奏诊断",
      component: async () => ({ default: () => <SidebarPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 8, viewId: "story-map", title: "角色统计",
      component: async () => ({ default: () => <CanvasPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
  ];
  return { dispose() { for (const d of disposers) d.dispose(); } };
}
