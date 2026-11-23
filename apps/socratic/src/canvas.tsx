// socratic 画布（surface.canvas / socratic-board）：真实推理板书。
// 会话状态来自 storage 回放 + socratic.turn 事件订阅，内容全部为真实 agent 回复，无 mock。
import type { AgentChannel } from "@ava2/app-sdk";
import { useEffect, useState } from "react";
import type { Events, Session, StorageLike } from "./lib";
import { Styles } from "./styles";

/** 画布：真实推理板书（按导师追问序列沉淀）。 */
export function Canvas({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [sess, setSess] = useState<Session | null>(null);
  useEffect(() => {
    let alive = true;
    const d = events.on("socratic.turn", (e) => { if (alive) setSess(e as Session); });
    void storage.get<Session>("session").then((s) => { if (alive && s) setSess(s); });
    return () => { alive = false; d.dispose(); };
  }, [events, storage]);

  const mentorTurns = sess ? sess.turns.filter((t) => t.role === "mentor") : [];
  return (
    <div className="sc-root">
      <Styles />
      <div className="sc-steps">
        <p className="sc-sec-title">推理板书 · 真实追问序列</p>
        {mentorTurns.length === 0 ? (
          <div className="sc-empty">开始追问后，导师的每一轮引导会沉淀在这里。</div>
        ) : (
          mentorTurns.map((t, i) => (
            <div className={"sc-step" + (i === mentorTurns.length - 1 ? " latest" : "")} key={i}>
              <span className="no">{i + 1}</span>
              {t.text}
            </div>
          ))
        )}
        <div style={{ height: "0.5px", background: "var(--ava-border-l2)", margin: "var(--ava-space-2) 0" }} />
        <p className="sc-sec-title">学生作答记录</p>
        {sess && sess.turns.filter((t) => t.role === "student").length > 0 ? (
          sess.turns.filter((t) => t.role === "student").map((t, i) => (
            <div className="sc-card" key={i} style={{ fontSize: 12, lineHeight: 1.7 }}>{t.text}</div>
          ))
        ) : (
          <div className="sc-empty">暂无作答</div>
        )}
      </div>
    </div>
  );
}
