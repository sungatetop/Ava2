// 数据叙事台画布 slot：图表墙（分布直方图 + 数值走势），经 useSelection 读取跨视图广播的数据集。
import type { AgentChannel } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import {
  SELECT_EVENT, computeInsights, fmtNum, parseCsv,
  type Dataset, type Events, type Selection, type StorageLike,
} from "./lib";
import { ACCENT, Styles } from "./styles";

/** 面板/画布读当前选中数据集：订阅总线事件 + storage 兜底（晚挂载也能恢复）。 */
export function useSelection(agent: AgentChannel, events: Events, storage: StorageLike) {
  const [dataset, setDataset] = useState<Dataset | null>(null);
  const [sel, setSel] = useState<Selection | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (s: Selection) => {
    try {
      const res = await agent.readWorkspaceFile(s.path, s.sessionId);
      const text = res.text ?? "";
      setSel(s);
      setDataset({ name: s.name, csv: text, parsed: parseCsv(text) });
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [agent]);

  const retry = useCallback(() => {
    if (sel) { void load(sel); return; }
    // 尚无选择时回退读 storage 上次选择
    void storage.get<Selection>("selection").then((saved) => {
      if (saved && saved.path) void load(saved);
    });
  }, [sel, load, storage]);

  useEffect(() => {
    let alive = true;
    const d = events.on(SELECT_EVENT, (e) => {
      if (alive) void load(e as Selection);
    });
    // 晚挂载兜底：读上次选择（storage 持久化）
    void storage.get<Selection>("selection").then((saved) => {
      if (alive && saved && saved.path) void load(saved);
    });
    return () => {
      alive = false;
      d.dispose();
    };
  }, [events, storage, load]);

  return { dataset, sel, error, retry };
}

export function Canvas({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const { dataset, error, retry } = useSelection(agent, events, storage);
  const ins = dataset ? computeInsights(dataset.parsed) : null;
  const maxBucket = ins ? Math.max(...ins.buckets, 1) : 1;
  const series = ins ? ins.series : [];
  const sMin = series.length ? Math.min(...series) : 0;
  const sMax = series.length ? Math.max(...series) : 1;
  const sSpan = sMax - sMin || 1;
  // 桶数自适应：等分绘图区（x 36→330 约 294px），每根柱在自己槽位内居中，保持总宽视觉均衡
  const bucketCount = ins ? ins.buckets.length : 0;
  const step = bucketCount > 0 ? 294 / bucketCount : 0;
  const barW = Math.max(4, step - 10);
  const pts = series.map((n, i) => {
    const x = 34 + (i * 290) / Math.max(1, series.length - 1);
    const y = 88 - ((n - sMin) / sSpan) * 66;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
  return (
    <div className="in-root">
      <Styles />
      <div className="in-charts">
        {error ? (
          <div className="in-err" style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ flex: 1 }}>{error}</span>
            <button className="in-btn" onClick={retry}>重试</button>
          </div>
        ) : null}
        <div className="in-chart">
          <div className="hd"><span>数值分布{ins ? `（${ins.numName}）` : ""}</span><span className="in-chip">结构</span></div>
          {ins ? (
            <svg viewBox="0 0 340 110">
              <line className="in-axis" x1="28" y1="92" x2="330" y2="92" />
              <line className="in-grid" x1="28" y1="46" x2="330" y2="46" />
              {ins.buckets.map((cnt, i) => {
                const h = Math.round((cnt / maxBucket) * 66);
                return <rect key={i} x={36 + i * step + (step - barW) / 2} y={92 - h} width={barW} height={Math.max(h, cnt > 0 ? 2 : 0)} rx={3}
                  fill={cnt === maxBucket ? ACCENT : "color-mix(in srgb,var(--in-accent) 34%,transparent)"} />;
              })}
              <text className="in-lbl" x="36" y="104">{fmtNum(ins.min)}</text>
              <text className="in-lbl" x="286" y="104">{fmtNum(ins.max)}</text>
            </svg>
          ) : (
            <div className="in-empty">未选择数据集</div>
          )}
        </div>
        <div className="in-chart">
          <div className="hd"><span>数值走势（前 {series.length} 行）</span><span className="in-chip">趋势</span></div>
          {ins && series.length > 0 ? (
            <svg viewBox="0 0 340 110">
              <line className="in-axis" x1="28" y1="92" x2="330" y2="92" />
              <line className="in-grid" x1="28" y1="60" x2="330" y2="60" />
              <line className="in-grid" x1="28" y1="28" x2="330" y2="28" />
              <polyline points={pts} fill="none" stroke={ACCENT} strokeWidth="2" />
              <text className="in-lbl" x="28" y="104">第 1 行</text>
              <text className="in-lbl" x="278" y="104">第 {series.length} 行</text>
            </svg>
          ) : (
            <div className="in-empty">无可用数值列</div>
          )}
        </div>
      </div>
    </div>
  );
}
