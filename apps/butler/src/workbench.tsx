// 工作台侧栏：借鉴原型的三格统计 + 运行中会话卡 + 最近结算列表。
import { useEffect, useState } from "react";
import type { ButlerEngine, ButlerWorker } from "./engine";
import { useButler } from "./engine";
import { relTime } from "./chat";

/** 计时：mm:ss。 */
function elapsed(from: number): string {
  const s = Math.max(0, Math.floor((Date.now() - from) / 1000));
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

export function Workbench({ engine, onOpenWorker }: { engine: ButlerEngine; onOpenWorker: (id: string) => void }) {
  const { snapshot } = useButler(engine);
  const running = snapshot.workers.filter((w) => w.status === "running");
  const done = snapshot.workers.filter((w) => w.status === "done");
  const [, tick] = useState(0);

  useEffect(() => {
    const t = setInterval(() => tick((v) => v + 1), 1000);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="bt-workbench">
      <div className="bt-wb-head">
        <span className="bt-wb-title">工作台</span>
      </div>
      <div className="bt-wb-body">
        <div className="bt-wb-section">
          <div className="bt-stats">
            <div className="bt-stat"><div className="n">{running.length}</div><div className="k">运行中</div></div>
            <div className="bt-stat"><div className="n">{done.length}</div><div className="k">已结算</div></div>
            <div className={"bt-stat" + (snapshot.escalations.length > 0 ? " alert" : "")}>
              <div className="n">{snapshot.escalations.length}</div><div className="k">待处理</div>
            </div>
          </div>
        </div>

        <div className="bt-wb-section">
          <h4>运行中 <span className="count">{running.length}</span></h4>
          {running.length === 0 ? (
            <div className="bt-empty">没有正在执行的会话</div>
          ) : (
            running.map((w: ButlerWorker) => (
              <div className="bt-wb-card" key={w.sessionId} onClick={() => onOpenWorker(w.sessionId)}>
                <div className="top">
                  <span className="bt-wb-av">{w.task.slice(0, 1)}</span>
                  <div style={{ minWidth: 0 }}>
                    <div className="name">{w.task}</div>
                    <div className="sub">
                      <span className="bt-chip ok">运行中</span>
                      <span>{w.sessionId.slice(0, 8)}</span>
                    </div>
                  </div>
                </div>
                <div className="step">
                  <span>{w.phase || "跟进中"}</span>
                  <span className="elapsed">{elapsed(w.createdAtMs)}</span>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="bt-wb-section">
          <h4>最近结算 <span className="count">{done.length}</span></h4>
          {done.length === 0 ? (
            <div className="bt-empty">还没有完成的会话</div>
          ) : (
            done.slice().reverse().slice(0, 8).map((w) => (
              <div className="bt-wb-item" key={w.sessionId} onClick={() => onOpenWorker(w.sessionId)}>
                <span className="txt">{w.task}</span>
                <span className="when">{relTime(w.doneAtMs || w.updatedAtMs)}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

/** worker 会话视图：只读展示该会话的任务与产出（不做独立对话，避免与主会话纠缠）。 */
export function WorkerView({ engine, sessionId }: { engine: ButlerEngine; sessionId: string }) {
  const { snapshot } = useButler(engine);
  const w = snapshot.workers.find((x) => x.sessionId === sessionId);
  const notices = snapshot.conversation.filter((n) => n.workerId === sessionId);
  const [, tick] = useState(0);

  useEffect(() => {
    if (!w || w.status === "done") return;
    const t = setInterval(() => tick((v) => v + 1), 1000);
    return () => clearInterval(t);
  }, [w?.status, w]);

  return (
    <div className="bt-mainpane">
      <div className="bt-scroll">
        <div className="bt-flow">
          <div className="bt-wb-card" style={{ cursor: "default", marginBottom: 4 }}>
            <div className="top">
              <span className="bt-wb-av">{w ? w.task.slice(0, 1) : "·"}</span>
              <div style={{ minWidth: 0 }}>
                <div className="name">{w ? w.task : "会话已不在台账"}</div>
                <div className="sub">
                  {w ? <span className={"bt-chip" + (w.status === "running" ? " ok" : "")}>{w.status === "running" ? "运行中" : "已完成"}</span> : null}
                  <span>{sessionId.slice(0, 8)}</span>
                </div>
              </div>
            </div>
            {w && w.status === "running" ? (
              <div className="step">
                <span>{w.phase || "跟进中"}</span>
                <span className="elapsed">{elapsed(w.createdAtMs)}</span>
              </div>
            ) : null}
          </div>

          {notices.length === 0 ? (
            <div className="bt-empty">该会话还没有回传内容</div>
          ) : (
            notices.map((n) => (
              <div key={n.id}>
                {n.kind === "report" ? (
                  <div className="bt-result">
                    <div className="rh"><span className="tick">✓</span><span className="name">{n.text}</span></div>
                    <div className="rbody">{n.body ?? ""}</div>
                  </div>
                ) : (
                  <div className="bt-dispatch">
                    <span className="glyph">D</span>
                    <span className="body">
                      <span className="name">{n.text}</span>
                      <span className="meta"><span className="bt-chip ok">已派发</span><span>{relTime(n.ts)}</span></span>
                    </span>
                  </div>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
