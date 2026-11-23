// 技能管理主视图：本地技能目录（按来源分组 + 搜索 + 启停开关，卡片网格呈现）与
// 技能广场（SkillHub）两个页签。禁用名单落盘
// `<projectRoot>/.onion/skills-manager.json`（服务端 PUT 原子写），
// 目录/手势/skill 工具三个注入点下一轮 pre-step 自动收敛，无需重启会话。
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import type { AgentChannel, NetApi, SkillInfo, SkillManagerInfo } from "@ava-ai/app-sdk";
import { SkillHubView } from "./skill-hub-view";
import { BTN, BTN_PRIMARY, CHIP, GRID_STYLE, INPUT } from "./styles";

interface SkillsViewProps {
  agent: AgentChannel;
  /** 统一联网服务（技能广场直连上游用） */
  net: NetApi;
}

/** 来源桶展示名与分组排序（project 在前，bundled 兜底） */
const SOURCE_ORDER: string[] = [
  "project-onion",
  "project-claude",
  "project-dsh",
  "project-agents",
  "runtime",
  "custom",
  "user-onion",
  "user-claude",
  "user-dsh",
  "user-agents",
  "bundled",
];
const SOURCE_LABEL: Record<string, string> = {
  "project-onion": "项目 .onion",
  "project-claude": "项目 .claude",
  "project-dsh": "项目 .dsh",
  "project-agents": "项目 .agents",
  runtime: "运行时注册",
  custom: "宿主挂载",
  "user-onion": "用户 .onion",
  "user-claude": "用户 .claude",
  "user-dsh": "用户 .dsh",
  "user-agents": "用户 .agents",
  bundled: "内置",
};

/** 最近会话解析工作区（禁用清单按会话 cwd 的 project root 落盘）。 */
async function resolveSession(agent: AgentChannel): Promise<string> {
  const sessions = await agent.listSessions();
  return sessions[0]?.id ?? "";
}

/** 页签：本地技能目录 / 技能广场（SkillHub） */
type Tab = "local" | "hub";

const TAB_LABEL: Record<Tab, string> = { local: "本地技能", hub: "技能广场" };

export function SkillsView({ agent, net }: SkillsViewProps) {
  const [skills, setSkills] = useState<SkillInfo[] | null>(null);
  const [manager, setManager] = useState<SkillManagerInfo | null>(null);
  const [err, setErr] = useState("");
  const [note, setNote] = useState("");
  const [query, setQuery] = useState("");
  const [busyName, setBusyName] = useState("");
  const [tab, setTab] = useState<Tab>("local");
  const [sessionId, setSessionId] = useState("");

  const load = useCallback(async () => {
    try {
      const id = await resolveSession(agent);
      setSessionId(id);
      if (!id) {
        setErr("");
        setSkills([]);
        setManager(null);
        return;
      }
      const [list, info] = await Promise.all([
        agent.listSkills(id),
        agent.getSkillManager(id),
      ]);
      setErr("");
      setSkills(list);
      setManager(info);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }, [agent]);

  useEffect(() => {
    void load();
  }, [load]);

  /** 启停切换：乐观更新 disabled 列表，PUT 失败回滚。 */
  const toggle = async (skill: SkillInfo) => {
    if (!manager || busyName || !sessionId) return;
    const nextDisabled = skill.enabled
      ? [...manager.disabled, skill.name].sort()
      : manager.disabled.filter((n) => n !== skill.name);
    const prev = manager;
    setBusyName(skill.name);
    setErr("");
    setNote("");
    // 乐观更新：列表标记与名单同步翻转。
    setManager({ ...prev, disabled: nextDisabled });
    setSkills((list) =>
      (list ?? []).map((s) => (s.name === skill.name ? { ...s, enabled: !s.enabled } : s)),
    );
    try {
      const saved = await agent.putSkillManager(nextDisabled, sessionId);
      setManager(saved);
      setNote(skill.enabled ? `已禁用「${skill.name}」，下一轮对话生效` : `已启用「${skill.name}」`);
    } catch (e) {
      // 回滚乐观状态。
      setManager(prev);
      setSkills((list) =>
        (list ?? []).map((s) => (s.name === skill.name ? { ...s, enabled: skill.enabled } : s)),
      );
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyName("");
    }
  };

  /** 搜索过滤（名称 + 描述）后按来源分组。 */
  const groups = useMemo(() => {
    const filtered = (skills ?? []).filter((s) => {
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q);
    });
    const bySource = new Map<string, SkillInfo[]>();
    for (const skill of filtered) {
      const bucket = bySource.get(skill.source) ?? [];
      bucket.push(skill);
      bySource.set(skill.source, bucket);
    }
    return [...bySource.entries()]
      .sort(
        (a, b) =>
          SOURCE_ORDER.indexOf(a[0]) - SOURCE_ORDER.indexOf(b[0]) ||
          a[0].localeCompare(b[0]),
      )
      .map(([source, items]) => ({
        source,
        items: items.sort((a, b) => a.name.localeCompare(b.name)),
      }));
  }, [skills, query]);

  const disabledCount = manager?.disabled.length ?? 0;

  if (err && skills === null) {
    return (
      <div style={{ padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }}>
        加载失败：{err}
        <button style={{ ...BTN, marginLeft: 12 }} onClick={() => void load()}>
          重试
        </button>
      </div>
    );
  }

  return (
    <div
      style={{
        padding: "var(--ava-space-5, 24px)",
        overflow: "auto",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-4, 16px)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }}>
        <h2 style={{ margin: 0, fontSize: 16 }}>技能管理</h2>
        <span style={{ fontSize: 12, color: "var(--ava-label-tertiary)" }}>
          {skills?.length ?? 0} 个技能 · {disabledCount} 个已禁用
        </span>
        <span style={{ flex: 1 }} />
        {(["local", "hub"] as Tab[]).map((key) => (
          <button
            key={key}
            style={{
              ...BTN,
              ...(tab === key
                ? { background: "var(--ava-nav-active)", fontWeight: 600 }
                : {}),
            }}
            onClick={() => setTab(key)}
          >
            {TAB_LABEL[key]}
          </button>
        ))}
      </div>
      {note || err ? (
        <div style={{ fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }}>
          {err || note}
        </div>
      ) : null}
      {tab === "hub" ? (
        <SkillHubView
          agent={agent}
          net={net}
          sessionId={sessionId}
          onInstalled={(name) => {
            setNote(`已安装「${name}」到项目 .onion/skills，下一轮对话生效`);
            void load();
          }}
        />
      ) : (
        <>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }}>
            <input
              style={INPUT}
              placeholder="搜索名称或描述…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <button style={BTN} onClick={() => void load()}>
              刷新
            </button>
          </div>
          {manager?.file ? (
            <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)" }}>
              禁用清单：
              <span style={{ fontFamily: "var(--ava-font-mono, monospace)" }}>{manager.file}</span>
            </div>
          ) : null}
          {skills !== null && skills.length === 0 ? (
            <div style={{ padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }}>
              尚无可用会话或技能——先创建会话（禁用清单按会话工作区解析），或在项目 `.onion/skills` 等目录放置技能。
            </div>
          ) : null}
          {groups.map((group) => (
            <div key={group.source} style={{ display: "flex", flexDirection: "column", gap: "var(--ava-space-3, 10px)" }}>
              <div style={{ fontSize: 12, color: "var(--ava-label-secondary)", fontWeight: 600 }}>
                {SOURCE_LABEL[group.source] ?? group.source}
                <span style={{ marginLeft: 8, fontWeight: 400, color: "var(--ava-label-tertiary)" }}>
                  {group.items.length}
                </span>
              </div>
              <div style={GRID_STYLE}>
                {group.items.map((skill) => (
                  <LocalSkillCard
                    key={`${skill.provider}:${skill.name}`}
                    skill={skill}
                    busy={busyName === skill.name}
                    onToggle={() => void toggle(skill)}
                  />
                ))}
              </div>
            </div>
          ))}
        </>
      )}
    </div>
  );
}

/** 本地技能卡片（与广场卡片同语言：圆角软卡片 + hover 浮起/标题变色 + footer 操作区） */
function LocalSkillCard(props: { skill: SkillInfo; busy: boolean; onToggle: () => void }) {
  const { skill, busy, onToggle } = props;
  const [hover, setHover] = useState(false);
  // 操作按钮 hover 才显形（visibility 保持布局占位，避免 footer 高度跳动）
  const actionStyle: CSSProperties = {
    opacity: hover ? 1 : 0,
    visibility: hover ? "visible" : "hidden",
    transition: "opacity 0.15s ease",
  };
  const cardStyle: CSSProperties = {
    border: "0.5px solid var(--ava-border-l2)",
    borderRadius: "var(--ava-radius-panel, 28px)",
    padding: "var(--ava-space-5, 24px)",
    background: "var(--ava-bg-layer-1)",
    display: "flex",
    flexDirection: "column",
    gap: 10,
    minWidth: 0,
    opacity: skill.enabled ? 1 : 0.55,
    transform: hover ? "translateY(-2px)" : "none",
    boxShadow: hover ? "var(--ava-shadow-panel)" : "none",
    transition: "transform 0.15s ease, box-shadow 0.15s ease, opacity 0.15s ease",
  };
  const description = skill.description || "（无描述）";
  return (
    <div style={cardStyle} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
      {/* 顶行：首字母头像（hover 放大）+ 来源/调用能力徽标 */}
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 8 }}>
        <div
          style={{
            width: 40,
            height: 40,
            borderRadius: "var(--ava-radius-xl, 20px)",
            background: "var(--ava-nav-active)",
            color: "var(--ava-state-business-primary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 16,
            fontWeight: 700,
            flexShrink: 0,
            transform: hover ? "scale(1.08)" : "none",
            transition: "transform 0.15s ease",
          }}
        >
          {skill.name.slice(0, 1).toUpperCase()}
        </div>
        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", justifyContent: "flex-end" }}>
          <span style={CHIP()}>{SOURCE_LABEL[skill.source] ?? skill.source}</span>
          {!skill.modelInvocable ? <span style={CHIP()}>模型不可调</span> : null}
          {!skill.userInvocable ? <span style={CHIP()}>手势不可调</span> : null}
        </div>
      </div>
      {/* 名称（hover 变强调色，path 作悬浮提示） */}
      <b
        style={{
          fontSize: 14,
          color: hover ? "var(--ava-state-business-primary)" : "var(--ava-label-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          transition: "color 0.15s ease",
        }}
        title={skill.path ?? skill.name}
      >
        {skill.name}
      </b>
      {/* 描述（两行截断，whenToUse 并入悬浮提示） */}
      <div
        style={{
          fontSize: 12,
          color: "var(--ava-label-secondary)",
          lineHeight: 1.5,
          minHeight: 36,
          display: "-webkit-box",
          WebkitLineClamp: 2,
          WebkitBoxOrient: "vertical",
          overflow: "hidden",
        }}
        title={skill.whenToUse ? `${description}\n\n触发时机：${skill.whenToUse}` : description}
      >
        {description}
      </div>
      {/* 元信息行（provider，对齐广场统计行位置） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          fontSize: 11,
          fontWeight: 600,
          color: "var(--ava-label-tertiary)",
          marginTop: "auto",
        }}
      >
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={skill.provider}>
          {skill.provider}
        </span>
      </div>
      {/* footer：启停状态 + 切换主按钮 */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          paddingTop: "var(--ava-space-3, 10px)",
          borderTop: "0.5px solid var(--ava-border-l2)",
        }}
      >
        <span style={CHIP(skill.enabled ? "var(--ava-state-success)" : "var(--ava-state-danger)")}>
          {skill.enabled ? "启用中" : "已禁用"}
        </span>
        <span style={{ flex: 1 }} />
        <button
          style={{ ...BTN_PRIMARY, ...actionStyle }}
          disabled={busy}
          title={skill.enabled ? "禁用后下一轮对话生效" : "启用后下一轮对话生效"}
          onClick={onToggle}
        >
          {busy ? "保存中…" : skill.enabled ? "禁用" : "启用"}
        </button>
      </div>
    </div>
  );
}
