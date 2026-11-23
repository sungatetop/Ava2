// chatapp 会话历史的共享状态：侧边栏面板（panel.sidebar）与对话视图（surface.main）
// 是两个独立挂载点，经模块级 store 同步「会话列表 / 当前会话 / 近用工作区 / 命令」，
// 避免各自重复拉取 REST 与状态漂移。结构：会话按工作区分组。
import { useSyncExternalStore } from "react";
import type { SessionListItem } from "@ava-ai/app-sdk";

/** 侧边栏 → 对话视图的一次性命令（seq 去重，避免 React 严格模式下重复执行） */
export type ChatCommand =
  | { seq: number; kind: "select"; id: string }
  | { seq: number; kind: "new"; cwd?: string };

export interface ChatStoreState {
  /** REST 会话列表（含 workingDir / updatedAtMs，侧边栏按工作区分组） */
  sessions: SessionListItem[];
  /** 当前挂载的会话 id（由对话视图 attach 回写） */
  currentId: string | null;
  /** 近用工作区（storage 持久化，composer chip 与侧边栏共用） */
  recents: string[];
  /** 待执行命令（侧边栏消费后由对话视图执行） */
  command: ChatCommand | null;
}

let state: ChatStoreState = { sessions: [], currentId: null, recents: [], command: null };
const listeners = new Set<() => void>();
let seq = 0;

function publish(patch: Partial<ChatStoreState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) listener();
}

export const chatStore = {
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  snapshot(): ChatStoreState {
    return state;
  },
  /** 写入会话列表并返回排序后的副本（调用方据此决定「启动恢复哪个会话」） */
  setSessions(sessions: SessionListItem[]): SessionListItem[] {
    // 惰性创建：没有任何事件的会话等于「点了新建但还没发首条消息」的草稿，
    // 不落盘也不进列表（历史遗留的空会话同样在此隐藏）；当前挂载会话始终保留，
    // 避免首条消息落盘前的短暂窗口里选中项凭空消失。
    const visible = sessions.filter((session) => session.eventCount > 0 || session.id === state.currentId);
    // REST 返回顺序不稳定（磁盘枚举序）：统一按最近活跃倒序，
    // 侧边栏分组顺序与「启动恢复最近会话」都依赖该顺序
    const sorted = [...visible].sort((a, b) => (b.updatedAtMs ?? 0) - (a.updatedAtMs ?? 0));
    publish({ sessions: sorted });
    return sorted;
  },
  setCurrent(currentId: string | null) {
    publish({ currentId });
  },
  /** 就地更新单个会话标题（自动命名 durable `session_title` 事件到达时） */
  setTitle(id: string, title: string) {
    const target = state.sessions.find((session) => session.id === id);
    if (!target || target.title === title) return;
    publish({
      sessions: state.sessions.map((session) =>
        session.id === id ? { ...session, title } : session,
      ),
    });
  },
  setRecents(recents: string[]) {
    publish({ recents });
  },
  /** 请求切到某会话 */
  select(id: string) {
    publish({ command: { seq: ++seq, kind: "select", id } });
  },
  /** 请求新建会话（可选工作区绝对路径） */
  create(cwd?: string) {
    publish({ command: { seq: ++seq, kind: "new", cwd } });
  },
};

export function useChatStore(): ChatStoreState {
  return useSyncExternalStore(chatStore.subscribe, chatStore.snapshot);
}

export interface SessionGroup {
  /** 分组键：工作区绝对路径；无法判定时为 ""（未归类） */
  key: string;
  /** 分组显示名（工作区目录名） */
  label: string;
  sessions: SessionListItem[];
}

/** 路径末段（工作区分组显示名） */
function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

/** 会话按工作区分组：入参已按最近活跃倒序，组内与组间顺序自然继承 */
export function groupSessions(sessions: SessionListItem[]): SessionGroup[] {
  const buckets = new Map<string, SessionListItem[]>();
  for (const session of sessions) {
    const key = session.workingDir ?? "";
    const bucket = buckets.get(key);
    if (bucket) bucket.push(session);
    else buckets.set(key, [session]);
  }
  return [...buckets].map(([key, list]) => ({
    key,
    label: key ? baseName(key) : "未归类",
    sessions: list,
  }));
}

/** 会话行标题：尚无消息的会话 REST 标题为空，回落到短 id */
export function sessionLabel(session: SessionListItem): string {
  const title = session.title.trim();
  return title || `新会话 ${session.id.slice(-6)}`;
}

/** 会话行时间：刚/分钟/小时/天（相对时间显示） */
export function sessionTime(updatedAtMs: number | undefined): string {
  if (!updatedAtMs) return "";
  const delta = Date.now() - updatedAtMs;
  if (delta < 60_000) return "刚刚";
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)} 分钟前`;
  if (delta < 86_400_000) return `${Math.floor(delta / 3_600_000)} 小时前`;
  return `${Math.floor(delta / 86_400_000)} 天前`;
}
