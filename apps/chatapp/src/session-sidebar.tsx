// chatapp 侧边栏会话历史（panel.sidebar）：会话按工作区分组，组内按最近活跃倒序，
// 每组默认只显示 5 条；顶部为「新建会话」，行 hover 显示「导出 / 删除」。
// 删除确认用 Popconfirm 气泡（浮层指南：中低风险操作轻量确认，锚定触发点、点外取消，
// 不散落 window.confirm）。
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { AgentChannel, SessionListItem } from "@ava-ai/app-sdk";
import { chatappCss } from "./styles";
import { downloadText, sessionToMarkdown } from "./export";
import { chatStore, groupSessions, sessionLabel, sessionTime, useChatStore } from "./session-store";
import type { SessionGroup } from "./session-store";

/** 每组折叠上限 */
const SESSION_LIMIT = 5;

/** 气泡确认（Popconfirm）：锚定触发点、点外取消，无遮罩；危险操作用 danger 强调 */
function Popconfirm({
  title,
  onConfirm,
  onClose,
}: {
  title: string;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  /** fixed 坐标（mount 后按锚点 rect 计算；首帧隐藏防闪跳） */
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [onClose]);
  // fixed 定位按锚点 rect 计算：不受滚动容器 overflow 裁剪；clamp 视口，放不下向上弹
  useLayoutEffect(() => {
    const el = ref.current;
    const anchor = el?.parentElement;
    if (!el || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8));
    let top = r.bottom + 6;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 6);
    setPos({ top, left });
  }, []);
  return (
    <div
      className="cc-pc"
      ref={ref}
      role="dialog"
      aria-label="确认操作"
      style={pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="cc-pc-msg">{title}</div>
      <div className="cc-pc-acts">
        <button className="cc-pc-cancel" onClick={onClose}>
          取消
        </button>
        <button
          className="cc-pc-confirm"
          onClick={() => {
            onClose();
            onConfirm();
          }}
        >
          确认
        </button>
      </div>
    </div>
  );
}

function FolderIcon({ open, current }: { open: boolean; current: boolean }) {
  return open || current ? (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" />
    </svg>
  ) : (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" />
    </svg>
  );
}

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      className={"cc-sb-chev" + (open ? " open" : "")}
      width="10"
      height="10"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M9 6l6 6-6 6" />
    </svg>
  );
}

function SessionRow({
  session,
  active,
  busy,
  onExport,
  onRemove,
}: {
  session: SessionListItem;
  active: boolean;
  busy: boolean;
  onExport: (session: SessionListItem) => void;
  onRemove: (session: SessionListItem) => void;
}) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  return (
    <div className="cc-sb-item">
      <button
        className={"cc-sb-row" + (active ? " on" : "")}
        data-session-id={session.id}
        onClick={() => chatStore.select(session.id)}
        title={sessionLabel(session)}
        aria-current={active ? "true" : undefined}
      >
        <span className="cc-sb-row-slot">{session.live && <span className="cc-sb-live" />}</span>
        <span className="cc-sb-row-title">{sessionLabel(session)}</span>
        <span className="cc-sb-row-time">{sessionTime(session.updatedAtMs)}</span>
      </button>
      <span className="cc-sb-acts">
        <button
          className="cc-sb-act"
          title="导出为 Markdown"
          disabled={busy}
          onClick={() => onExport(session)}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M12 4v11 M7 11l5 5 5-5 M4 20h16" />
          </svg>
        </button>
        <button
          className="cc-sb-act is-danger"
          title="删除会话"
          disabled={busy}
          onClick={() => setConfirmOpen((v) => !v)}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M4 7h16 M9 7V5h6v2 M6 7l1 13h10l1-13" />
          </svg>
        </button>
      </span>
      {confirmOpen && (
        <span className="cc-pc-anchor">
          <Popconfirm
            title={`删除会话「${sessionLabel(session)}」？历史记录将从磁盘移除，不可恢复。`}
            onClose={() => setConfirmOpen(false)}
            onConfirm={() => void onRemove(session)}
          />
        </span>
      )}
    </div>
  );
}

export function SessionSidebar({ agent }: { agent: AgentChannel }) {
  const { sessions, currentId } = useChatStore();
  const groups = groupSessions(sessions);
  /** 当前会话所属工作区（草稿态为 undefined） */
  const currentKey = sessions.find((s) => s.id === currentId)?.workingDir;
  /** 草稿态（点了新建、还没发首条消息）沿用最后所在工作区作为展开基准：
   *  既不整列收起，也不把全部工作区铺开。 */
  const lastKeyRef = useRef<string | undefined>(currentKey);
  if (currentKey !== undefined) lastKeyRef.current = currentKey;
  const anchorKey = currentKey ?? lastKeyRef.current;
  /** 手动展开的组（非当前组默认折叠，点组头展开后记录） */
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  /** 手动折叠的组（当前组默认展开，点组头折叠后记录） */
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  /** 组内「显示全部」的分组 key（默认每组只显示 SESSION_LIMIT 条） */
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  /** 导出中会话 id（按钮禁用，避免重复下载） */
  const [busyId, setBusyId] = useState<string | null>(null);
  /** 删除中的工作区 key（组头按钮禁用） */
  const [busyWs, setBusyWs] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const toggle = (key: string, isCurrent: boolean) => {
    if (isCurrent) {
      // 当前组：默认展开，点组头折叠
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        return next;
      });
    } else {
      // 非当前组：默认折叠，点组头展开
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        return next;
      });
    }
  };

  const refresh = async () => {
    try {
      chatStore.setSessions(await agent.listSessions());
    } catch (e) {
      setError(`会话列表刷新失败：${e instanceof Error ? e.message : String(e)}`);
    }
  };

  /** 待确认删除的会话 id / 工作区 key（Popconfirm 锚定态） */
  const [pendingWs, setPendingWs] = useState<string | null>(null);

  const remove = async (session: SessionListItem) => {
    setBusyId(session.id);
    setError(null);
    try {
      await agent.deleteSession(session.id);
      const rest = sessions.filter((s) => s.id !== session.id);
      chatStore.setSessions(rest);
      // 当前会话被删：切到剩余最近会话；一个不剩则在同一工作区新建空会话
      if (currentId === session.id) {
        if (rest.length > 0) chatStore.select(rest[0].id);
        else chatStore.create(session.workingDir);
      }
      await refresh();
    } catch (e) {
      setError(`删除会话失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  /** 删除整个工作区（该分组下全部会话），确认后逐个删除并刷新 */
  const removeWorkspace = async (group: SessionGroup) => {
    setBusyWs(group.key);
    setError(null);
    try {
      for (const s of group.sessions) {
        await agent.deleteSession(s.id);
      }
      const rest = sessions.filter((s) => (s.workingDir ?? "") !== group.key);
      chatStore.setSessions(rest);
      // 当前会话在该工作区被删：切到剩余最近会话；一个不剩则新建空会话
      if (currentId !== null && group.sessions.some((s) => s.id === currentId)) {
        if (rest.length > 0) chatStore.select(rest[0].id);
        else chatStore.create(group.key || undefined);
      }
      await refresh();
    } catch (e) {
      setError(`删除工作区失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyWs(null);
    }
  };

  const exportOne = async (session: SessionListItem) => {
    setBusyId(session.id);
    setError(null);
    try {
      downloadText(sessionLabel(session), await sessionToMarkdown(agent, session));
    } catch (e) {
      setError(`导出会话失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="cc-root cc-sb">
      <style>{chatappCss}</style>
      <button className="cc-sb-new" onClick={() => chatStore.create()} title="新建会话">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
          <path d="M12 5v14M5 12h14" />
        </svg>
        新建会话
      </button>
      {error && <div className="cc-sb-error">{error}</div>}
      <div className="cc-sb-list">
        {groups.length === 0 && <div className="cc-sb-empty">暂无历史会话</div>}
        {groups.map((group) => {
          const isCurrent = group.sessions.some((s) => s.id === currentId);
          // 展开基准：当前会话所在组；草稿态退到最后所在工作区（anchorKey）
          const anchored = group.key === anchorKey;
          const open = anchored ? !collapsed.has(group.key) : expanded.has(group.key);
          const showAll = revealed.has(group.key);
          const visible = showAll ? group.sessions : group.sessions.slice(0, SESSION_LIMIT);
          const hidden = group.sessions.length - visible.length;
          return (
            <div className={"cc-sb-group" + (isCurrent ? " is-current" : "")} key={group.key}>
              <div
                className="cc-sb-group-head"
                role="button"
                tabIndex={0}
                onClick={() => toggle(group.key, anchored)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") toggle(group.key, anchored);
                }}
                title={group.key || "未归类会话"}
              >
                <ChevronIcon open={open} />
                <span className="cc-sb-folder">
                  <FolderIcon open={open} current={isCurrent} />
                </span>
                <span className="cc-sb-group-label">{group.label}</span>
                <button
                  className="cc-sb-del"
                  title={`删除工作区「${group.label}」及其全部 ${group.sessions.length} 个会话`}
                  disabled={busyWs !== null}
                  onClick={(e) => {
                    e.stopPropagation();
                    setPendingWs(pendingWs === group.key ? null : group.key);
                  }}
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M4 7h16 M9 7V5h6v2 M6 7l1 13h10l1-13" />
                  </svg>
                </button>
                {pendingWs === group.key && (
                  <span className="cc-pc-anchor head">
                    <Popconfirm
                      title={`删除工作区「${group.label}」及其全部 ${group.sessions.length} 个会话？历史记录将从磁盘移除，不可恢复。`}
                      onClose={() => setPendingWs(null)}
                      onConfirm={() => void removeWorkspace(group)}
                    />
                  </span>
                )}
                <button
                  className="cc-sb-add"
                  title={`在 ${group.label} 新建会话`}
                  onClick={(e) => {
                    e.stopPropagation();
                    chatStore.create(group.key || undefined);
                  }}
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                    <path d="M12 5v14M5 12h14" />
                  </svg>
                </button>
              </div>
              {open &&
                visible.map((session) => (
                  <SessionRow
                    key={session.id}
                    session={session}
                    active={session.id === currentId}
                    busy={busyId === session.id}
                    onExport={(s) => void exportOne(s)}
                    onRemove={(s) => void remove(s)}
                  />
                ))}
              {open && hidden > 0 && (
                <button
                  className="cc-sb-more"
                  onClick={() =>
                    setRevealed((prev) => {
                      const next = new Set(prev);
                      next.add(group.key);
                      return next;
                    })
                  }
                >
                  展开其余 {hidden} 个会话
                </button>
              )}
              {open && showAll && group.sessions.length > SESSION_LIMIT && (
                <button
                  className="cc-sb-more"
                  onClick={() =>
                    setRevealed((prev) => {
                      const next = new Set(prev);
                      next.delete(group.key);
                      return next;
                    })
                  }
                >
                  收起
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
