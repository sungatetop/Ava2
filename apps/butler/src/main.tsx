// 总管家（Butler）：App 层代管 Agent（不动 onion 边车）。
// 界面借鉴 ava-app 的 butler-chat-redesign 原型：
//   顶部 tabstrip（总管家 + 已打开 worker 会话）→ 中部对话流 → 底部胶囊输入
//   → 右侧 300px 工作台侧栏（三格统计 + 运行中卡 + 最近结算）。
// 代管引擎在 engine.ts；对话流在 chat.tsx；工作台在 workbench.tsx；面板在 panels.tsx。
import type { AppContext } from "@ava-ai/app-sdk";
import { useState } from "react";
import { ButlerEngine, useButler } from "./engine";
import { ChatView } from "./chat";
import { Workbench, WorkerView } from "./workbench";
import { DisciplinePanel, RosterPanel, SettingsPanel } from "./panels";
import { Styles } from "./styles";

/** 主窗口：tabstrip + 视图切换 + 工作台侧栏。 */
function MainWindow({ engine }: { engine: ButlerEngine }) {
  const { snapshot } = useButler(engine);
  const [tab, setTab] = useState<string>("butler");
  const workers = snapshot.workers;

  // tab 对应的 worker 可能已被清理，回落总管家
  const active = tab !== "butler" && workers.some((w) => w.sessionId === tab) ? tab : "butler";

  return (
    <div className="bt-root">
      <Styles />
      <div className="bt-mainpane">
        <div className="bt-tabs">
          <button className={"bt-tab" + (active === "butler" ? " on" : "")} onClick={() => setTab("butler")}>
            总管家
            {snapshot.busy ? <span className="dot" /> : null}
          </button>
          {workers.slice().reverse().slice(0, 6).map((w) => (
            <button
              key={w.sessionId}
              className={"bt-tab" + (active === w.sessionId ? " on" : "")}
              onClick={() => setTab(w.sessionId)}
              title={w.task}
            >
              <span className={"dot" + (w.status === "done" ? " done" : "")} />
              {w.task.length > 12 ? w.task.slice(0, 12) + "…" : w.task}
            </button>
          ))}
          <span className="bt-tab-spacer" />
        </div>

        {active === "butler" ? (
          <ChatView engine={engine} onOpenWorker={(id) => setTab(id)} />
        ) : (
          <WorkerView engine={engine} sessionId={active} />
        )}
      </div>

      <Workbench engine={engine} onOpenWorker={(id) => setTab(id)} />
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  // 单例代管引擎：宿主装配期创建并启动，各视图组件 props 下传共享（不各自实例化）。
  const engine = new ButlerEngine(ctx.agent, ctx.storage);
  void engine.start();

  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 6,
      viewId: "butler",
      title: "总管家",
      icon: "M12 3a4 4 0 0 1 4 4v1.5a2 2 0 0 1 2 2V21a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V10.5a2 2 0 0 1 2-2V7a4 4 0 0 1 4-4zm2 5.5V7a2 2 0 1 0-4 0v1.5h4z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "butler",
      title: "总管家",
      component: async () => ({ default: () => <MainWindow engine={engine} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "butler-roster",
      title: "派发台账",
      component: async () => ({ default: () => <RosterPanel engine={engine} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 6,
      viewId: "butler",
      title: "注意力设置",
      component: async () => ({ default: () => <SettingsPanel engine={engine} /> }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 6,
      viewId: "butler-roster",
      title: "安全纪律",
      component: async () => ({ default: () => <DisciplinePanel engine={engine} /> }),
    }),
    ctx.slots.inject("user.menu", {
      id: "butler-open",
      title: "打开总管家",
      action: { kind: "openView", viewId: "butler" },
    }),
  ];
  return {
    dispose() {
      engine.stop();
      for (const d of disposers) d.dispose();
    },
  };
}
