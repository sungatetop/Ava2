// 数据叙事台面板 slots：sidebar「数据体检」+ canvas「口径说明」，经 useSelection 读取当前数据集。
import type { AgentChannel } from "@ava2/app-sdk";
import { useSelection } from "./canvas";
import { computeInsights, fmtNum, type Events, type StorageLike } from "./lib";
import { Styles } from "./styles";

export function SidebarPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const { dataset, error, retry } = useSelection(agent, events, storage);
  const ins = dataset ? computeInsights(dataset.parsed) : null;
  const total = dataset ? dataset.parsed.rows.length : 0;
  const emptyCells = dataset
    ? dataset.parsed.rows.reduce((acc, r) => acc + r.filter((c) => c.trim() === "").length, 0)
    : 0;
  const cellTotal = total * (dataset ? dataset.parsed.headers.length : 0);
  const missingRate = cellTotal > 0 ? ((emptyCells / cellTotal) * 100).toFixed(1) : "0.0";
  return (
    <div className="in-root">
      <Styles />
      <div className="in-panel">
        <p className="in-sec-title">数据体检</p>
        {error ? (
          <div className="in-err" style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ flex: 1 }}>{error}</span>
            <button className="in-btn" onClick={retry}>重试</button>
          </div>
        ) : null}
        {ins ? (
          <>
            <div className="in-kv"><span>数据集</span><b style={{ fontFamily: "var(--ava-font-ui)" }}>{dataset!.name}</b></div>
            <div className="in-kv"><span>行数</span><b>{ins.rowCount}</b></div>
            <div className="in-kv"><span>列数</span><b>{ins.headers.length}</b></div>
            <div className="in-kv"><span>缺失率</span><b>{missingRate}%</b></div>
            <div className="in-kv"><span>异常值</span><b>{ins.spikes} 行</b></div>
            <div style={{ marginTop: 8 }}>
              <div className="in-bar" style={{ maxWidth: "100%" }}><i style={{ width: Math.max(4, 100 - Number(missingRate) * 4) + "%" }} /></div>
              <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 6 }}>
                {ins.spikes > 0 ? `检出 ${ins.spikes} 条异常尖峰，建议核对` : "未检出异常尖峰"}
              </div>
            </div>
          </>
        ) : (
          <div className="in-empty">未选择数据集</div>
        )}
      </div>
    </div>
  );
}

export function CanvasPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const { dataset, sel, error, retry } = useSelection(agent, events, storage);
  const ins = dataset ? computeInsights(dataset.parsed) : null;
  return (
    <div className="in-root">
      <Styles />
      <div className="in-panel">
        <p className="in-sec-title">口径说明</p>
        {error ? (
          <div className="in-err" style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ flex: 1 }}>{error}</span>
            <button className="in-btn" onClick={retry}>重试</button>
          </div>
        ) : null}
        {ins ? (
          <>
            <div className="in-kv"><span>统计列</span><b style={{ fontFamily: "var(--ava-font-ui)" }}>{ins.numName}</b></div>
            <div className="in-kv"><span>行数</span><b>{ins.rowCount}</b></div>
            <div className="in-kv"><span>区间</span><b>{fmtNum(ins.min)} ~ {fmtNum(ins.max)}</b></div>
            <div className="in-kv"><span>均值</span><b>{fmtNum(ins.avg)}</b></div>
            <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8, wordBreak: "break-all" }}>
              {sel ? sel.path : "—"}
            </div>
            <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 4 }}>
              结论基于均值与 2 倍尖峰启发式，全部来自所选 CSV 真实内容。
            </div>
          </>
        ) : (
          <div className="in-empty">未选择数据集</div>
        )}
      </div>
    </div>
  );
}
