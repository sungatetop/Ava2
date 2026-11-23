// 工作区文件浏览器（panel.canvas）：以最近会话的工作区为根，目录懒加载展开，
// 双击文件 → canvas.open/openPath 交给 file-viewer 画布预览（每个文件一个 tab，可多开）。
// 形态：面板自带 38px 头部（根名 + 刷新），树行高 28px、每级缩进 18px；
// 折叠/展开由宿主画布 tab 栏右侧的文件树按钮统一控制。
// 数据全部来自 ctx.agent 公开能力（listDirs / listSessions），无私有端点。
import { useCallback, useEffect, useState } from "react";
import type { AgentChannel, DirEntry, FileEntry } from "@ava-ai/app-sdk";
import { fileExplorerCss } from "./styles";
import { openInCanvas } from "./canvas-bridge";

/** 树节点状态：path → 懒加载的子目录与文件 */
interface NodeState {
  loading: boolean;
  error: string | null;
  dirs: DirEntry[];
  files: FileEntry[];
}

const ICON = {
  chevron: "M9 6l6 6-6 6",
  folder: "M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  refresh: "M20 12a8 8 0 1 1-2.3-5.6 M20 4v5h-5",
} as const;

function Icon({ d, size = 14 }: { d: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

/** 路径末段（根路径展示用；`C:\` 这类根保留原样） */
function baseName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

export function FileExplorer({ agent }: { agent: AgentChannel }) {
  /** 浏览根：最近会话的工作区（无会话时回落主目录） */
  const [root, setRoot] = useState<string | null>(null);
  /** 根所属会话 id（预览端按该会话权限根校验路径） */
  const [rootSessionId, setRootSessionId] = useState<string | undefined>(undefined);
  const [nodes, setNodes] = useState<Record<string, NodeState>>({});
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [rootError, setRootError] = useState<string | null>(null);
  /** 会话列表尚未返回（用于区分「定位中」与「确实没有工作区」） */
  const [rootPending, setRootPending] = useState(true);

  /** 拉取一个目录的子项（含文件），写入 nodes */
  const load = useCallback(
    (path: string) => {
      setNodes((prev) => ({ ...prev, [path]: { ...(prev[path] ?? { dirs: [], files: [] }), loading: true, error: null } }));
      agent
        .listDirs(path, { files: true })
        .then((listing) => {
          setNodes((prev) => ({
            ...prev,
            [listing.path]: { loading: false, error: null, dirs: listing.entries, files: listing.files ?? [] },
          }));
        })
        .catch((e) => {
          setNodes((prev) => ({
            ...prev,
            [path]: { ...(prev[path] ?? { dirs: [], files: [] }), loading: false, error: e instanceof Error ? e.message : String(e) },
          }));
        });
    },
    [agent],
  );

  // 初始：用最近活跃会话的工作区做根
  useEffect(() => {
    let alive = true;
    setRootError(null);
    setRootPending(true);
    agent
      .listSessions()
      .then((sessions) => {
        if (!alive) return;
        const recent = [...sessions]
          .sort((a, b) => (b.updatedAtMs ?? 0) - (a.updatedAtMs ?? 0))
          .find((s) => s.workingDir);
        setRoot(recent?.workingDir ?? null);
        setRootSessionId(recent?.id);
        setRootPending(false);
        if (recent?.workingDir) load(recent.workingDir);
      })
      .catch((e) => {
        if (!alive) return;
        setRootError(e instanceof Error ? e.message : String(e));
        setRootPending(false);
      });
    return () => {
      alive = false;
    };
  }, [agent, load]);

  const toggle = (path: string) => {
    const isOpen = expanded.has(path);
    setExpanded((prev) => {
      const next = new Set(prev);
      if (isOpen) next.delete(path);
      else next.add(path);
      return next;
    });
    if (!isOpen && !nodes[path] && !rootError) load(path);
  };

  /** 递归渲染一个目录的子树 */
  const renderDir = (entry: DirEntry, depth: number) => {
    const open = expanded.has(entry.path);
    const state = nodes[entry.path];
    return (
      <div key={entry.path}>
        <button className="fe-row" style={{ paddingLeft: 8 + depth * 18 }} onClick={() => toggle(entry.path)} title={entry.path}>
          <span className={"fe-chev" + (open ? " open" : "")}>
            <Icon d={ICON.chevron} size={10} />
          </span>
          <span className="fe-icon">
            <Icon d={ICON.folder} size={13} />
          </span>
          <span className="fe-name">{entry.name}</span>
        </button>
        {open && (
          <div>
            {state?.loading && <div className="fe-state" style={{ paddingLeft: 8 + (depth + 1) * 18 }}>读取中…</div>}
            {state?.error && <div className="fe-state err" style={{ paddingLeft: 8 + (depth + 1) * 18 }}>{state.error}</div>}
            {state && !state.loading && !state.error && state.dirs.length === 0 && state.files.length === 0 && (
              <div className="fe-state" style={{ paddingLeft: 8 + (depth + 1) * 18 }}>空目录</div>
            )}
            {state?.dirs.map((child) => renderDir(child, depth + 1))}
            {state?.files.map((file) => (
              <button
                key={file.path}
                className="fe-row"
                style={{ paddingLeft: 8 + (depth + 1) * 18 }}
                title={`${file.path}\n双击在画布预览`}
                onDoubleClick={() => openInCanvas(file.path, rootSessionId)}
              >
                <span className="fe-chev ghost" />
                <span className="fe-icon">
                  <Icon d={ICON.file} size={13} />
                </span>
                <span className="fe-name file">{file.name}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    );
  };

  const rootState = root ? nodes[root] : undefined;

  return (
    <div className="fe-root">
      <style>{fileExplorerCss}</style>
      <div className="fe-head">
        <span className="fe-title">文件</span>
        {root && (
          <span className="fe-root-name" title={root}>
            {baseName(root)}
          </span>
        )}
        <button
          className="fe-btn"
          title="刷新目录"
          disabled={!root}
          onClick={() => root && load(root)}
        >
          <Icon d={ICON.refresh} size={13} />
        </button>
      </div>
      <div className="fe-body">
        {rootError && <div className="fe-state err">会话列表读取失败：{rootError}</div>}
        {!rootError && rootPending && <div className="fe-state">正在定位工作区…</div>}
        {!rootError && !rootPending && !root && <div className="fe-state">暂无会话，无法确定工作区</div>}
        {root && (
          <div className="fe-tree">
            {rootState?.loading && <div className="fe-state">读取中…</div>}
            {rootState?.error && <div className="fe-state err">{rootState.error}</div>}
            {rootState?.dirs.map((child) => renderDir(child, 0))}
            {rootState?.files.map((file) => (
              <button
                key={file.path}
                className="fe-row"
                style={{ paddingLeft: 8 }}
                title={`${file.path}\n双击在画布预览`}
                onDoubleClick={() => openInCanvas(file.path, rootSessionId)}
              >
                <span className="fe-chev ghost" />
                <span className="fe-icon">
                  <Icon d={ICON.file} size={13} />
                </span>
                <span className="fe-name file">{file.name}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
