// composer 底部三枚 chip（工作区 / 审批模式 / 模型）。
// 交互：一律点击展开锚定上浮面板，不再使用原生 select；面板内行高 34、当前项打勾、
// ↑/↓ 移动焦点、Esc 与外部点击关闭。
import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { ProviderInfo } from "@ava-ai/app-sdk";
import { Popover } from "./popover";

export type ApprovalPreset = "workspace-write" | "auto" | "danger-full-access";

/** 审批预设目录（顺序即面板展示顺序） */
const APPROVALS: { value: ApprovalPreset; label: string; hint: string }[] = [
  { value: "workspace-write", label: "工作区写", hint: "只读与工作区内写直通；越界写、破坏性命令、网络访问需审批" },
  { value: "auto", label: "自动", hint: "常规工具调用自动放行" },
  { value: "danger-full-access", label: "全放行", hint: "跳过所有审批，谨慎使用" },
];

/** 路径短显示：首段 + … + 末段 */
export function shortDir(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `${parts[0]}/…/${parts[parts.length - 1]}` : parts.join("/");
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      className={"cc-chip-chevron" + (open ? " open" : "")}
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
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M20 6 9 17l-5-5" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" />
    </svg>
  );
}

function ApprovalIcon({ preset }: { preset: ApprovalPreset }) {
  const common = { width: 14, height: 14, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true };
  if (preset === "auto") {
    return (
      <svg {...common}>
        <path d="M13 2 4.5 13.5H11l-1 8.5 8.5-11.5H12l1-8.5Z" />
      </svg>
    );
  }
  if (preset === "danger-full-access") {
    return (
      <svg {...common}>
        <path d="M12 3 2.5 20h19L12 3Z" />
        <path d="M12 9v5M12 17.5v.01" />
      </svg>
    );
  }
  return (
    <svg {...common}>
      <path d="M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17v3Z" />
      <path d="M14 6.5 17.5 10" />
    </svg>
  );
}

function ModelIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 3 4 7.5v9L12 21l8-4.5v-9L12 3Z" />
      <path d="M4 7.5 12 12l8-4.5M12 12v9" />
    </svg>
  );
}

/** 面板内的行列表：挂载即聚焦当前项，↑/↓ 在行间移动焦点（Enter 由按钮原生触发） */
function MenuList({ children, autoFocusItem = true }: { children: ReactNode; autoFocusItem?: boolean }) {
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!autoFocusItem) return;
    const root = ref.current;
    const checked = root?.querySelector<HTMLElement>('[aria-checked="true"]:not(:disabled)');
    (checked ?? root?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)'))?.focus();
  }, [autoFocusItem]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Enter 显式激活当前行（部分环境下 button 的默认 click 不会由回车合成）
      if (e.key === "Enter") {
        const item = document.activeElement as HTMLElement | null;
        if (item?.getAttribute("role")?.startsWith("menuitem")) {
          e.preventDefault();
          item.click();
        }
        return;
      }
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      const items = Array.from(
        ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled), [role="menuitemradio"]:not(:disabled)') ?? [],
      );
      if (items.length === 0) return;
      e.preventDefault();
      const at = items.findIndex((item) => item === document.activeElement);
      const next =
        at === -1
          ? e.key === "ArrowDown"
            ? 0
            : items.length - 1
          : (at + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="cc-menu-list" ref={ref} role="menu">
      {children}
    </div>
  );
}

interface ChipProps {
  disabled?: boolean;
}

// ── 工作区 chip：当前工作目录 + 最近使用 + 浏览目录 ──

interface WorkspaceChipProps extends ChipProps {
  /** 当前会话工作区绝对路径 */
  current?: string;
  /** 最近使用的工作区（倒序，最多 5 个） */
  workspaces: string[];
  /** 选择某个工作区（以其为 cwd 新建会话） */
  onPick: (dir: string) => void;
  /** 打开目录浏览器选择其他目录 */
  onBrowse: () => void;
}

export function WorkspaceChip({ current, workspaces, disabled, onPick, onBrowse }: WorkspaceChipProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const label = current ? shortDir(current) : "选择目录";
  const picked = (dir: string) => {
    setOpen(false);
    onPick(dir);
  };

  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        className="cc-chip-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        title={current ? `当前工作区：${current}` : "选择工作目录"}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="cc-chip-icon">
          <FolderIcon />
        </span>
        <span className="cc-chip-text">{label}</span>
        <Chevron open={open} />
      </button>
      <Popover anchor={anchor} open={open} onClose={() => setOpen(false)} width={340}>
        <div className="cc-menu-path" title={current}>
          {current ?? "尚未选择工作区"}
        </div>
        {workspaces.length > 0 && (
          <MenuList>
            <div className="cc-menu-group-head">最近使用</div>
            {workspaces.map((dir) => (
              <button
                key={dir}
                type="button"
                role="menuitemradio"
                aria-checked={dir === current}
                className="cc-menu-item"
                title={dir}
                onClick={() => picked(dir)}
              >
                <span className="cc-menu-item-icon">
                  <FolderIcon />
                </span>
                <span className="cc-menu-item-text">{shortDir(dir)}</span>
                <span className="cc-menu-check">{dir === current && <CheckIcon />}</span>
              </button>
            ))}
          </MenuList>
        )}
        <div className="cc-menu-sep" />
        <button
          type="button"
          role="menuitem"
          className="cc-menu-item"
          onClick={() => {
            setOpen(false);
            onBrowse();
          }}
        >
          <span className="cc-menu-item-icon">
            <FolderIcon />
          </span>
          <span className="cc-menu-item-text">选择其他目录…</span>
        </button>
      </Popover>
    </>
  );
}

// ── 审批模式 chip ──

interface ApprovalChipProps extends ChipProps {
  value: ApprovalPreset;
  onChange: (preset: ApprovalPreset) => void;
}

export function ApprovalChip({ value, disabled, onChange }: ApprovalChipProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const current = APPROVALS.find((item) => item.value === value) ?? APPROVALS[0];

  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        className="cc-chip-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        title={`审批模式：${current.hint}`}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="cc-chip-icon">
          <ApprovalIcon preset={current.value} />
        </span>
        <span className="cc-chip-text">{current.label}</span>
        <Chevron open={open} />
      </button>
      <Popover anchor={anchor} open={open} onClose={() => setOpen(false)} width={220}>
        <MenuList>
          {APPROVALS.map((item) => (
            <button
              key={item.value}
              type="button"
              role="menuitemradio"
              aria-checked={item.value === value}
              className="cc-menu-item"
              title={item.hint}
              onClick={() => {
                setOpen(false);
                if (item.value !== value) onChange(item.value);
              }}
            >
              <span className="cc-menu-item-icon">
                <ApprovalIcon preset={item.value} />
              </span>
              <span className="cc-menu-item-text">{item.label}</span>
              <span className="cc-menu-check">{item.value === value && <CheckIcon />}</span>
            </button>
          ))}
        </MenuList>
      </Popover>
    </>
  );
}

// ── 模式 chip：标准 / 创造（作用于下一次新建会话） ──

export type ChatMode = "standard" | "creator";

/** 模式目录（顺序即面板展示顺序） */
const MODES: { value: ChatMode; label: string; hint: string }[] = [
  { value: "standard", label: "标准", hint: "常规对话与任务执行" },
  { value: "creator", label: "创造", hint: "按契约开发并安装 ava2 App" },
];

function ModeIcon({ mode }: { mode: ChatMode }) {
  if (mode === "creator") {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3Z" />
        <path d="M19 15.5l.9 2.4 2.4.9-2.4.9-.9 2.4-.9-2.4-2.4-.9 2.4-.9.9-2.4Z" />
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M21 12a9 9 0 1 1-9-9" />
      <path d="M12 7v5l3 3" />
    </svg>
  );
}

interface ModeChipProps extends ChipProps {
  value: ChatMode;
  onChange: (mode: ChatMode) => void;
}

export function ModeChip({ value, disabled, onChange }: ModeChipProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const current = MODES.find((item) => item.value === value) ?? MODES[0];

  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        className="cc-chip-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        title={`会话模式：${current.label}（${current.hint}）；选择后下一次新建会话生效`}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="cc-chip-icon">
          <ModeIcon mode={current.value} />
        </span>
        <span className="cc-chip-text">{current.label}</span>
        <Chevron open={open} />
      </button>
      <Popover anchor={anchor} open={open} onClose={() => setOpen(false)} width={260}>
        <MenuList>
          {MODES.map((item) => (
            <button
              key={item.value}
              type="button"
              role="menuitemradio"
              aria-checked={item.value === value}
              className="cc-menu-item"
              title={item.hint}
              onClick={() => {
                setOpen(false);
                if (item.value !== value) onChange(item.value);
              }}
            >
              <span className="cc-menu-item-icon">
                <ModeIcon mode={item.value} />
              </span>
              <span className="cc-menu-item-text">{item.label}</span>
              <span className="cc-menu-check">{item.value === value && <CheckIcon />}</span>
            </button>
          ))}
        </MenuList>
        <div className="cc-menu-hint">选择后下一次新建会话生效</div>
      </Popover>
    </>
  );
}

// ── 模型 chip：provider 分组列表 + 超过 4 项时可搜索 ──

interface ModelChipProps extends ChipProps {
  value: string;
  catalog: ProviderInfo[];
  onChange: (model: string) => void;
}

export function ModelChip({ value, catalog, disabled, onChange }: ModelChipProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const [query, setQuery] = useState("");
  const total = catalog.reduce((n, p) => n + p.models.length, 0);
  const searchable = total > 4;
  const keyword = searchable ? query.trim().toLowerCase() : "";
  const groups = catalog
    .map((p) => ({
      ...p,
      models: keyword
        ? p.models.filter((m) => (m.name || m.id).toLowerCase().includes(keyword) || m.id.toLowerCase().includes(keyword))
        : p.models,
    }))
    .filter((p) => p.models.length > 0);
  const currentName = catalog.flatMap((p) => p.models).find((m) => m.id === value)?.name || value || "模型…";

  const close = () => {
    setOpen(false);
    setQuery("");
  };

  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        className="cc-chip-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        title={`模型：当前 ${currentName}，切换后下一请求生效`}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="cc-chip-icon">
          <ModelIcon />
        </span>
        <span className="cc-chip-text">{currentName}</span>
        <Chevron open={open} />
      </button>
      <Popover anchor={anchor} open={open} onClose={close} width={300}>
        {searchable && (
          <input
            className="cc-menu-search"
            type="text"
            role="searchbox"
            aria-label="搜索模型"
            placeholder="搜索模型"
            value={query}
            autoFocus
            onChange={(e) => setQuery(e.target.value)}
          />
        )}
        <MenuList autoFocusItem={!searchable}>
          {groups.map((p) => (
            <div key={p.id} className="cc-menu-group">
              <div className="cc-menu-group-head" title={p.name || p.id}>
                {p.name || p.id}
              </div>
              {p.models.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  role="menuitemradio"
                  aria-checked={m.id === value}
                  className="cc-menu-item"
                  title={m.description || m.name || m.id}
                  onClick={() => {
                    close();
                    if (m.id !== value) onChange(m.id);
                  }}
                >
                  <span className="cc-menu-item-text">{m.name || m.id}</span>
                  <span className="cc-menu-check">{m.id === value && <CheckIcon />}</span>
                </button>
              ))}
            </div>
          ))}
          {groups.length === 0 && <div className="cc-menu-empty">无匹配模型</div>}
        </MenuList>
      </Popover>
    </>
  );
}
