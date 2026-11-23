// storyloom 画布：surface.canvas 舞台组件（真实人名频次角色关系图）。
// 数据经 storyloom.open 事件与 storage("script") 水合，无 mock。
import type { AgentChannel, AppContext } from "@ava2/app-sdk";
import { useCallback, useEffect, useState } from "react";
import type { Script } from "./lib";
import { Styles } from "./styles";

type Events = AppContext["events"];
type StorageLike = AppContext["storage"];

/** 画布：真实人名频次角色关系图。 */
export function Canvas({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [script, setScript] = useState<Script | null>(null);

  const adopt = useCallback((p: Omit<Script, "totalWords"> & { totalWords?: number }) => {
    const beats = (p.beats || []).map((b) => ({
      ...b,
      ratio: b.words / Math.max(1, ...(p.beats || []).map((x) => x.words)),
    }));
    setScript({
      name: p.name, path: p.path, beats, chars: p.chars || [],
      totalWords: p.totalWords ?? beats.reduce((a, b) => a + b.words, 0),
    });
  }, []);

  useEffect(() => {
    let alive = true;
    const d = events.on("storyloom.open", (e) => {
      if (alive) adopt(e as Parameters<typeof adopt>[0]);
    });
    void storage.get<Parameters<typeof adopt>[0]>("script").then((saved) => {
      if (alive && saved && saved.beats) adopt(saved);
    });
    return () => {
      alive = false;
      d.dispose();
    };
  }, [events, storage, adopt]);

  const chars = script?.chars ?? [];
  const maxCount = Math.max(1, ...chars.map((c) => c.count));
  const hub = chars[0];
  const rest = chars.slice(1);
  return (
    <div className="sl-root">
      <Styles />
      <div className="sl-canvas">
        {chars.length === 0 ? (
          <div className="sl-empty">选择文稿后，这里显示按真实出现频次统计的人物关系。</div>
        ) : (
          <>
            <svg className="sl-map" viewBox="0 0 340 250">
              {rest.map((c, i) => {
                const step = 220 / Math.max(1, rest.length);
                const x = 60 + Math.max(0, rest.length - 1) * step / 2 + i * step;
                return <line key={c.name} className="sl-edge" x1="170" y1="70" x2={x} y2="180" />;
              })}
              {hub && (
                <g className="sl-node hub">
                  <circle cx="170" cy="70" r="24" />
                  <text x="170" y="74" textAnchor="middle">{hub.name.slice(0, 3)}</text>
                </g>
              )}
              {rest.map((c, i) => {
                const step = 220 / Math.max(1, rest.length);
                const x = 60 + Math.max(0, rest.length - 1) * step / 2 + i * step;
                const r = 14 + (c.count / maxCount) * 10;
                return (
                  <g key={c.name} className="sl-node">
                    <circle cx={x} cy="180" r={r} />
                    <text x={x} y={184} textAnchor="middle">{c.name.slice(0, 3)}</text>
                  </g>
                );
              })}
            </svg>
            <div style={{ height: "0.5px", background: "var(--ava-border-l2)", margin: "var(--ava-space-3) 0" }} />
            <p className="sl-sec-title">人名出现频次（真实统计）</p>
            {chars.map((c) => (
              <div className="sl-heat" key={c.name}>
                <span className="lbl">{c.name}</span>
                <div className="sl-bar"><i style={{ width: Math.round((c.count / maxCount) * 100) + "%" }} /></div>
                <span className="mono">{c.count}</span>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
