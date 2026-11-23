// storyloom 下面板：panel.sidebar（节奏诊断）+ panel.canvas（角色统计）。
// 数据经 storyloom.open 事件与 storage("script") 水合，无 mock。
import type { AgentChannel, AppContext } from "@ava2/app-sdk";
import { useEffect, useState } from "react";
import type { Beat, Character, Script } from "./lib";
import { ACCENT, Styles } from "./styles";

type Events = AppContext["events"];
type StorageLike = AppContext["storage"];

/** 下面板：真实每拍字数张力曲线。 */
export function SidebarPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [script, setScript] = useState<Script | null>(null);

  useEffect(() => {
    let alive = true;
    const apply = (p: { beats: Beat[]; name: string; path: string }) => {
      if (!alive) return;
      const beats = p.beats || [];
      setScript({
        name: p.name, path: p.path, chars: [], beats,
        totalWords: beats.reduce((a, b) => a + b.words, 0),
      });
    };
    const d = events.on("storyloom.open", (e) => apply(e as { beats: Beat[]; name: string; path: string }));
    void storage.get<{ beats: Beat[]; name: string; path: string }>("script").then((saved) => {
      if (saved) apply(saved);
    });
    return () => {
      alive = false;
      d.dispose();
    };
  }, [events, storage]);

  if (!script || script.beats.length === 0) {
    return <div className="sl-root"><Styles /><div className="sl-empty">选择文稿后显示张力曲线。</div></div>;
  }
  const beats = script.beats;
  const maxWords = Math.max(1, ...beats.map((b) => b.words));
  const avg = script.totalWords / beats.length;
  const pts = beats.map((b, i) => {
    const x = 20 + (i * 560) / Math.max(1, beats.length - 1);
    const y = 70 - (b.words / maxWords) * 58;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
  const shortBeats = beats.filter((b) => b.words < avg * 0.6).length;
  return (
    <div className="sl-root">
      <Styles />
      <div className="sl-panel">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
          <p className="sl-sec-title" style={{ margin: 0 }}>节奏诊断 · 每拍字数</p>
          <span className="sl-chip">{beats.length} 拍</span>
        </div>
        <svg className="sl-curve" viewBox="0 0 600 80" preserveAspectRatio="none">
          <line x1="20" y1="70" x2="580" y2="70" stroke="var(--ava-border-l2)" strokeWidth="1" />
          <polyline points={pts} fill="none" stroke={ACCENT} strokeWidth="2" />
        </svg>
        <div>
          <span className="sl-tip">均长 {Math.round(avg)} 字/拍</span>
          <span className="sl-tip">最长 {maxWords} 字</span>
          {shortBeats > 0 && <span className="sl-tip">偏短拍 {shortBeats} 个（低于均长 60%）</span>}
        </div>
      </div>
    </div>
  );
}

/** 画布下面板：真实统计摘要。 */
export function CanvasPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [script, setScript] = useState<{ chars: Character[]; name: string } | null>(null);
  useEffect(() => {
    let alive = true;
    const apply = (p: { chars: Character[]; name: string }) => { if (alive) setScript(p); };
    const d = events.on("storyloom.open", (e) => apply(e as { chars: Character[]; name: string }));
    void storage.get<{ chars: Character[]; name: string }>("script").then((s) => { if (s) apply(s); });
    return () => { alive = false; d.dispose(); };
  }, [events, storage]);

  return (
    <div className="sl-root">
      <Styles />
      <div className="sl-panel">
        <p className="sl-sec-title">角色统计</p>
        {!script || script.chars.length === 0 ? (
          <div className="sl-empty">选择文稿后显示。</div>
        ) : (
          <>
            <div className="sl-kv"><span>文稿</span><b>{script.name}</b></div>
            <div className="sl-kv"><span>识别角色</span><b>{script.chars.length} 个</b></div>
            <div className="sl-kv"><span>首位</span><b>{script.chars[0].name}（{script.chars[0].count} 次）</b></div>
            <p style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8, marginBottom: 0 }}>
              按文中连续汉字串真实词频统计，已过滤常见虚词。
            </p>
          </>
        )}
      </div>
    </div>
  );
}
