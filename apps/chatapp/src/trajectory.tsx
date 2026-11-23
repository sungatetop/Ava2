// 轨迹视图（独立 tab，占中栏）：完整对齐 ava_square TrajectoryView 设计
//（docs/design/onion-agent-chat-ui-design.md §9）——
// 工具栏（搜索 / 记录数 / 折叠全部）+ 泳道概览（统计行 + 三泳道序列模式，块按序号
// 等宽排布，不表达时长）+ 回合分组记录表（粘性 Turn N 轮头 + 28px 记录行 +
// 可展开等宽详情）。数据由对话流同一份 ChatItem[] 前端派生（trajectory-model），
// 不新增端点；泳道块点击 → 选中记录并展开其所在回合，表格滚动定位。
import { useEffect, useMemo, useRef, useState } from "react";
import type { ChatItem } from "./chat-types";
import { buildTrajectory } from "./trajectory-model";
import type { TrajCell, TrajKind, TrajStats } from "./trajectory-model";
import { formatTokens } from "./format";

/** 泳道定义：三条 lane 各自收纳的记录类别（§9.3：输入 / 模型 / 工具） */
const LANES: { key: string; label: string; kinds: TrajKind[] }[] = [
  { key: "input", label: "输入", kinds: ["user", "system", "context"] },
  { key: "model", label: "模型", kinds: ["message", "thinking", "error"] },
  { key: "tool", label: "工具", kinds: ["tool"] },
];

/** 泳道块 / tag chip 色标（kind 语义对齐 ava_square KIND_COLOR） */
const KIND_CLASS: Record<TrajKind, string> = {
  user: "is-user",
  message: "is-message",
  thinking: "is-think",
  tool: "is-tool",
  system: "is-system",
  context: "is-context",
  error: "is-error",
};

/** lucide chevron-right / chevron-down（24×24 线性，与 items.tsx Icon 同规格） */
function Chevron({ open }: { open?: boolean }) {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={{ transform: open ? "rotate(90deg)" : undefined, transition: "transform 150ms ease" }}
    >
      <path d="M9 6l6 6-6 6" />
    </svg>
  );
}

/** 泳道概览：统计行 + 三泳道序列模式（块位置 = (index-1)/span 等宽排布） */
function Timeline({
  cells,
  stats,
  selectedId,
  onSelect,
}: {
  cells: TrajCell[];
  stats: TrajStats;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  if (cells.length === 0) return null;
  // 搜索过滤后 index 不连续，按最后一个 index 归一化，保证块不越界（§9.3）
  const span = cells[cells.length - 1].index || cells.length;
  const width = 100 / span;
  return (
    <div className="cc-trj-timeline">
      <p className="cc-trj-statline">
        {`${stats.turns} turns · ${stats.cells} records · ${stats.tools} tools · input ↑${formatTokens(stats.input)} / output ↓${formatTokens(stats.output)}`}
      </p>
      <div className="cc-trj-lanes">
        <div className="cc-trj-lane-labels">
          {LANES.map((lane) => (
            <span className="cc-trj-lane-label" key={lane.key}>
              {lane.label}
            </span>
          ))}
        </div>
        <div className="cc-trj-lane-tracks">
          {LANES.map((lane) => (
            <div className="cc-trj-lane-track" key={lane.key}>
              {cells.map((cell) =>
                lane.kinds.includes(cell.kind) ? (
                  <button
                    key={cell.id}
                    type="button"
                    className={`cc-trj-block ${KIND_CLASS[cell.kind]}${selectedId === cell.id ? " on" : ""}`}
                    style={{ left: `${(cell.index - 1) * width}%`, width: `max(3px, ${width}%)` }}
                    title={`Turn ${cell.turn} · ${cell.tag} · ${cell.title}`}
                    onClick={() => onSelect(cell.id)}
                  />
                ) : null,
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/** 记录行展开详情正文：工具显示 input/output，其余显示正文（§9.4） */
function detailBody(cell: TrajCell): string {
  return cell.detail?.trim() || "（无详情）";
}

/** 记录行：28px，grid [52px 标签 | 1fr 摘要 | auto 指标 | 14px 箭头]，点击展开详情 */
function Row({
  cell,
  open,
  selected,
  onToggle,
  registerRef,
}: {
  cell: TrajCell;
  open: boolean;
  selected: boolean;
  onToggle: () => void;
  registerRef: (el: HTMLButtonElement | null) => void;
}) {
  const failed = cell.ok === false;
  return (
    <div className="cc-trj-rowwrap">
      <button
        type="button"
        ref={registerRef}
        className={`cc-trj-row${selected ? " on" : ""}`}
        onClick={onToggle}
        aria-expanded={open}
      >
        <span className={`cc-trj-tag ${KIND_CLASS[cell.kind]}`}>{cell.tag}</span>
        <span className="cc-trj-row-main">
          <span className={"cc-trj-row-title" + (cell.summary ? " with-sum" : "")}>{cell.title}</span>
          {cell.summary && <span className="cc-trj-row-sum">{cell.summary}</span>}
        </span>
        <span className="cc-trj-row-meta">
          {cell.tokens != null && cell.tokens > 0 && <span>{`${formatTokens(cell.tokens)} tok`}</span>}
          {failed && <span className="is-fail">失败</span>}
          {!failed && cell.ok === true && <span className="is-ok">完成</span>}
        </span>
        <span className="cc-trj-row-caret">
          <Chevron open={open} />
        </span>
      </button>
      {open && (
        <div className="cc-trj-detail">
          <pre>{detailBody(cell)}</pre>
        </div>
      )}
    </div>
  );
}

export function TrajectoryView({ items }: { items: ChatItem[] }) {
  const model = useMemo(() => buildTrajectory(items), [items]);
  const [keyword, setKeyword] = useState("");
  const [collapsed, setCollapsed] = useState<Set<number>>(() => new Set());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLButtonElement | null>());

  // 搜索：按 searchText 过滤记录与回合，泳道与表格同步使用过滤后数据（§9.4）
  const kw = keyword.trim().toLowerCase();
  const view = useMemo(() => {
    if (!kw) return { cells: model.cells, turns: model.turns };
    const matched = model.cells.filter((cell) => cell.searchText.includes(kw));
    const ids = new Set(matched.map((cell) => cell.id));
    return {
      cells: matched,
      turns: model.turns
        .map((turn) => ({ ...turn, cells: turn.cells.filter((cell) => ids.has(cell.id)) }))
        .filter((turn) => turn.cells.length > 0),
    };
  }, [model, kw]);

  // 统计行随过滤视图收缩（与 ava_square TrajectoryView 同口径：基于 view.cells 重算）
  const stats = useMemo(() => {
    const totals: TrajStats = { turns: view.turns.length, cells: view.cells.length, tools: 0, input: 0, output: 0, failed: 0 };
    for (const cell of view.cells) {
      if (cell.kind === "tool") totals.tools += 1;
      if (cell.ok === false) totals.failed += 1;
    }
    for (const turn of view.turns) {
      totals.input += turn.input;
      totals.output += turn.output;
    }
    return totals;
  }, [view]);

  // 选中记录：展开其所在回合并滚动定位（泳道块 / 表格行共用）
  const select = (cellId: string) => {
    setSelectedId(cellId);
    const cell = view.cells.find((item) => item.id === cellId);
    if (!cell) return;
    setCollapsed((prev) => {
      if (!prev.has(cell.turn)) return prev;
      const next = new Set(prev);
      next.delete(cell.turn);
      return next;
    });
  };

  // 选中记录滚动到可视区中部（泳道点击 → 表格定位）
  useEffect(() => {
    if (!selectedId) return;
    rowRefs.current.get(selectedId)?.scrollIntoView({ block: "center" });
  }, [selectedId, view.turns]);

  const allCollapsed = view.turns.length > 0 && view.turns.every((turn) => collapsed.has(turn.turn));
  const toggleAll = () =>
    setCollapsed(allCollapsed ? new Set() : new Set(view.turns.map((turn) => turn.turn)));

  if (model.cells.length === 0) {
    return (
      <div className="cc-trj-root">
        <div className="cc-trj-empty">还没有可展示的轨迹记录</div>
      </div>
    );
  }

  return (
    <div className="cc-trj-root">
      <div className="cc-trj-toolbar">
        <span className="cc-trj-searchbox">
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-3.5-3.5" />
          </svg>
          <input
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="搜索记录…"
            aria-label="搜索轨迹记录"
          />
        </span>
        <span className="cc-trj-count">{`${view.cells.length} 条`}</span>
        <button type="button" className="cc-trj-toggleall" onClick={toggleAll}>
          {allCollapsed ? "展开全部" : "折叠全部"}
        </button>
      </div>

      <Timeline cells={view.cells} stats={stats} selectedId={selectedId} onSelect={select} />

      <div className="cc-trj-table">
        {view.turns.length === 0 ? (
          <p className="cc-trj-empty">没有匹配的记录</p>
        ) : (
          view.turns.map((turn) => {
            const isCollapsed = collapsed.has(turn.turn);
            const tokens = turn.input + turn.output;
            return (
              <section className="cc-trj-turn" key={turn.turn}>
                <button
                  type="button"
                  className={"cc-trj-turnhead" + (turn.streaming ? " run" : "")}
                  onClick={() =>
                    setCollapsed((prev) => {
                      const next = new Set(prev);
                      if (next.has(turn.turn)) next.delete(turn.turn);
                      else next.add(turn.turn);
                      return next;
                    })
                  }
                  aria-expanded={!isCollapsed}
                >
                  <span className="cc-trj-chev">
                    <Chevron open={!isCollapsed} />
                  </span>
                  <span className="cc-trj-turnname">{turn.turn === 0 ? "会话记录" : `Turn ${turn.turn}`}</span>
                  {turn.reason && <span className="cc-trj-reason">{turn.reason}</span>}
                  {turn.streaming && <span className="cc-trj-live">进行中</span>}
                  <span className="cc-trj-turnmeta">
                    {`${turn.steps} steps · ${turn.tools} tools${tokens > 0 ? ` · ${formatTokens(tokens)} tokens` : ""}`}
                  </span>
                </button>
                {!isCollapsed &&
                  turn.cells.map((cell) => (
                    <Row
                      key={cell.id}
                      cell={cell}
                      open={selectedId === cell.id}
                      selected={selectedId === cell.id}
                      onToggle={() => (selectedId === cell.id ? setSelectedId(null) : select(cell.id))}
                      registerRef={(el) => {
                        if (el) rowRefs.current.set(cell.id, el);
                        else rowRefs.current.delete(cell.id);
                      }}
                    />
                  ))}
              </section>
            );
          })
        )}
      </div>
    </div>
  );
}
