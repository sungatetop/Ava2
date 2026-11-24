// 工作目录选择器：模态目录浏览器（数据源 GET /v1/fs/dirs，只列子目录）。
// 交互对齐系统「选择文件夹」：进入子目录 / 上一级 / 切盘（Windows）/ 使用此目录，
// 以选择代替手输路径。桌面端额外提供「浏览系统目录…」——直接唤起操作系统原生的
// 文件夹选择对话框（经宿主 ctx.desktop.pickFolder），与常规桌面应用的打开体验一致。
import { useEffect, useRef, useState } from "react";
import type { AgentChannel, DirListing } from "@ava-ai/app-sdk";

interface WorkspacePickerProps {
  agent: AgentChannel;
  /** 起始目录（通常为当前会话工作区）；缺省时后端回落到用户主目录 */
  startDir?: string;
  /** 最近使用的工作区（快捷入口，选中即以其新建会话） */
  recent?: string[];
  /** 原生系统文件夹选择器（桌面端才有；缺省时隐藏入口） */
  native?: () => Promise<string | null>;
  onCancel: () => void;
  onPick: (dir: string) => void;
}

export function WorkspacePicker({ agent, startDir, recent = [], native, onCancel, onPick }: WorkspacePickerProps) {
  const [listing, setListing] = useState<DirListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [nativeBusy, setNativeBusy] = useState(false);
  const aliveRef = useRef(true);

  /** 进入某目录（缺省 = 后端默认目录）；失败保留上次列表并提示 */
  const navigate = (path?: string) => {
    setLoading(true);
    agent
      .listDirs(path)
      .then((data) => {
        if (!aliveRef.current) return;
        setListing(data);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!aliveRef.current) return;
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (aliveRef.current) setLoading(false);
      });
  };

  useEffect(() => {
    aliveRef.current = true;
    navigate(startDir);
    return () => {
      aliveRef.current = false;
    };
  }, [startDir]);

  // Esc 关闭（与系统对话框一致）
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  /** 唤起操作系统原生文件夹选择对话框：选中即返回上层以该目录建会话 */
  const browseNative = async () => {
    if (!native || nativeBusy) return;
    setNativeBusy(true);
    try {
      const dir = await native();
      if (dir) onPick(dir);
    } finally {
      if (aliveRef.current) setNativeBusy(false);
    }
  };

  return (
    <>
      <div className="cc-picker-mask" onClick={onCancel} />
      <div className="cc-picker" role="dialog" aria-modal="true" aria-label="选择工作目录">
        <header className="cc-picker-head">
          <span className="cc-picker-title">选择工作目录</span>
          <button className="cc-picker-x" onClick={onCancel} title="关闭" aria-label="关闭">
            ✕
          </button>
        </header>

        <div className="cc-picker-path" title={listing?.path ?? ""}>
          {listing?.path ?? (loading ? "载入中…" : "")}
        </div>

        <div className="cc-picker-list">
          {listing?.parent && (
            <button className="cc-picker-row" onClick={() => navigate(listing.parent ?? undefined)}>
              <span className="cc-picker-ic">↑</span>
              <span className="cc-picker-name">上一级</span>
            </button>
          )}
          {listing?.entries.map((entry) => (
            <button
              key={entry.path}
              className="cc-picker-row"
              title={entry.path}
              onClick={() => navigate(entry.path)}
            >
              <span className="cc-picker-ic">📁</span>
              <span className="cc-picker-name">{entry.name}</span>
            </button>
          ))}
          {listing && listing.entries.length === 0 && <div className="cc-picker-empty">该目录下没有子目录</div>}
        </div>

        {recent.length > 0 && (
          <div className="cc-picker-recent">
            <span className="cc-picker-recent-label">最近使用</span>
            {recent.map((dir) => (
              <button key={dir} className="cc-picker-recent-item" title={dir} onClick={() => onPick(dir)}>
                {dir}
              </button>
            ))}
          </div>
        )}

        {listing && listing.roots.length > 0 && (
          <div className="cc-picker-roots">
            {listing.roots.map((root) => (
              <button key={root} className="cc-picker-root" onClick={() => navigate(root)}>
                {root}
              </button>
            ))}
          </div>
        )}

        {error && <div className="cc-picker-err">{error}</div>}

        <footer className="cc-picker-foot">
          {native && (
            <button
              className="cc-picker-native"
              disabled={nativeBusy}
              title="使用操作系统文件夹选择器"
              onClick={() => void browseNative()}
            >
              {nativeBusy ? "打开系统对话框…" : "浏览系统目录…"}
            </button>
          )}
          <span className="cc-picker-foot-gap" />
          <button className="cc-picker-cancel" onClick={onCancel}>
            取消
          </button>
          <button
            className="cc-picker-ok"
            disabled={!listing || loading}
            onClick={() => listing && onPick(listing.path)}
          >
            使用此目录
          </button>
        </footer>
      </div>
    </>
  );
}
