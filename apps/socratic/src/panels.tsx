// socratic 面板：侧栏交互统计（panel.sidebar / socratic）与画布信息面板（panel.canvas / socratic-board）。
// 数据源与画布一致：storage 回放 + socratic.turn 事件，全部来自真实 agent 会话。
import type { AgentChannel } from "@ava2/app-sdk";
import { useEffect, useState } from "react";
import type { Events, Session, StorageLike } from "./lib";
import { Styles } from "./styles";

/** 侧栏：真实交互轮次统计（替代原掌握度热力）。 */
export function SidebarPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [sess, setSess] = useState<Session | null>(null);
  useEffect(() => {
    let alive = true;
    const d = events.on("socratic.turn", (e) => { if (alive) setSess(e as Session); });
    void storage.get<Session>("session").then((s) => { if (alive && s) setSess(s); });
    return () => { alive = false; d.dispose(); };
  }, [events, storage]);

  const mentor = sess ? sess.turns.filter((t) => t.role === "mentor").length : 0;
  const student = sess ? sess.turns.filter((t) => t.role === "student").length : 0;
  /** 格数随真实追问轮数动态增长：max(10, mentor 轮数)，10 格以上照常填充。 */
  const cells = Math.max(10, mentor);
  return (
    <div className="sc-root">
      <Styles />
      <div className="sc-panel">
        <p className="sc-sec-title">交互统计（真实）</p>
        {!sess ? (
          <div className="sc-empty">尚未开始追问</div>
        ) : (
          <>
            <div className="sc-kv"><span>题目</span><b style={{ fontFamily: "var(--ava-font-ui)" }}>{sess.questionName}</b></div>
            <div className="sc-kv"><span>导师追问</span><b>{mentor} 轮</b></div>
            <div className="sc-kv"><span>学生作答</span><b>{student} 次</b></div>
            <div className="sc-kv"><span>会话</span><b>{sess.sessionId.slice(0, 8)}</b></div>
            <div className="sc-grid">
              {Array.from({ length: cells }).map((_, i) => (
                <div key={i} className={"sc-cell" + (i < mentor ? " on" : "")}>{i < mentor ? "●" : ""}</div>
              ))}
            </div>
            <p style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8, marginBottom: 0 }}>
              每格代表一轮真实追问；数据来自 agent 会话实际回复。
            </p>
          </>
        )}
      </div>
    </div>
  );
}

/** 画布下面板：真实会话信息。 */
export function CanvasPanel({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [sess, setSess] = useState<Session | null>(null);
  useEffect(() => {
    let alive = true;
    const d = events.on("socratic.turn", (e) => { if (alive) setSess(e as Session); });
    void storage.get<Session>("session").then((s) => { if (alive && s) setSess(s); });
    return () => { alive = false; d.dispose(); };
  }, [events, storage]);

  return (
    <div className="sc-root">
      <Styles />
      <div className="sc-panel">
        <p className="sc-sec-title">会话信息</p>
        {!sess ? (
          <div className="sc-empty">尚未开始</div>
        ) : (
          <>
            <div className="sc-kv"><span>会话 ID</span><b>{sess.sessionId.slice(0, 8)}</b></div>
            <div className="sc-kv"><span>题目文件</span><b style={{ fontFamily: "var(--ava-font-ui)", wordBreak: "break-all" }}>{sess.questionName}</b></div>
            <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8, wordBreak: "break-all" }}>{sess.questionPath}</div>
            <p style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 6 }}>
              追问内容由真实 agent 会话生成，导师约束为「只提问不给答案」。
            </p>
          </>
        )}
      </div>
    </div>
  );
}
