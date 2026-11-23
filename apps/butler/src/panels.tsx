// 面板视图：注意力设置（panel.sidebar）、安全纪律（panel.canvas）、
// 派发台账（surface.canvas）——沿用原型侧栏的紧凑卡片语言。
import type { ButlerEngine, Discipline } from "./engine";
import { useButler } from "./engine";
import { Styles } from "./styles";
import { relTime } from "./chat";

const DISCIPLINE_LABEL: Record<Discipline, string> = {
  "escalate-only": "仅升级",
  disciplined: "纪律代批",
  relaxed: "宽松代批",
};

export function SettingsPanel({ engine }: { engine: ButlerEngine }) {
  const { snapshot, updateSettings } = useButler(engine);
  const s = snapshot.settings;
  return (
    <div className="bt-panel">
      <Styles />
      <h2 className="bt-sec-title">注意力 / 纪律设置</h2>
      <div className="bt-set-row">
        <span className="lbl">暂停自治（kill switch）</span>
        <div className={"bt-switch" + (s.killSwitch ? " on" : "")} onClick={() => updateSettings({ killSwitch: !s.killSwitch })} />
      </div>
      <div className="bt-set-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 6 }}>
        <span className="lbl">审批纪律档位</span>
        <div className="bt-seg">
          {(Object.keys(DISCIPLINE_LABEL) as Discipline[]).map((k) => (
            <button key={k} className={s.discipline === k ? "on" : ""} onClick={() => updateSettings({ discipline: k })}>
              {DISCIPLINE_LABEL[k]}
            </button>
          ))}
        </div>
      </div>
      <p className="bt-note" style={{ marginTop: 8 }}>代批留痕 who=butler · rule=条款编号；高危一律升级，不猜测。</p>
    </div>
  );
}

export function DisciplinePanel({ engine }: { engine: ButlerEngine }) {
  const { snapshot } = useButler(engine);
  return (
    <div className="bt-panel">
      <Styles />
      <h2 className="bt-sec-title">安全纪律</h2>
      <div className="bt-kv"><span>允许代批</span><b>构建 / 测试 / lint / 读文件 / 备份</b></div>
      <div className="bt-kv"><span>禁止代批</span><b className="no">发布 / 推送 / 删除 / 支付 / 凭据</b></div>
      <div className="bt-kv"><span>不确定</span><b>纪律档下升级，宽松档下放行</b></div>
      <div className="bt-kv"><span>当前档位</span><b>{DISCIPLINE_LABEL[snapshot.settings.discipline]}</b></div>
      <p className="bt-note" style={{ marginTop: 8 }}>App 层规则引擎：由 assessToolRisk 白名单裁决，高危硬护栏不交给模型裁量。</p>
    </div>
  );
}

/** 派发台账：全部代管会话（按状态分组）。 */
export function RosterPanel({ engine }: { engine: ButlerEngine }) {
  const { snapshot } = useButler(engine);
  const running = snapshot.workers.filter((w) => w.status === "running");
  const done = snapshot.workers.filter((w) => w.status === "done");
  const row = (label: string, list: typeof snapshot.workers) => (
    <>
      <h2 className="bt-sec-title" style={{ marginTop: 12 }}>{label} <span style={{ opacity: 0.8 }}>{list.length}</span></h2>
      {list.length === 0 ? (
        <div className="bt-empty">无</div>
      ) : (
        list.slice().reverse().map((w) => (
          <div className="bt-wb-item" key={w.sessionId}>
            <span className="txt">{w.task}</span>
            <span className="when">{w.status === "running" ? w.phase || "运行中" : relTime(w.doneAtMs || w.updatedAtMs)}</span>
          </div>
        ))
      )}
    </>
  );
  return (
    <div className="bt-panel">
      <Styles />
      <h2 className="bt-sec-title">派发台账 · 代管会话</h2>
      {row("运行中", running)}
      {row("已结算", done)}
    </div>
  );
}
