// src/skills-view.tsx
import { useCallback as useCallback2, useEffect, useMemo, useState as useState2 } from "react";

// src/skill-hub-view.tsx
import { useCallback, useRef, useState } from "react";

// src/styles.ts
var BTN = {
  height: 26,
  padding: "0 12px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "transparent",
  fontSize: 12,
  color: "var(--ava-label-primary)",
  cursor: "pointer"
};
var INPUT = {
  height: 28,
  padding: "0 8px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "var(--ava-bg-layer-1)",
  color: "var(--ava-label-primary)",
  fontSize: 12,
  outline: "none",
  width: 200,
  fontFamily: "var(--ava-font-ui)"
};
var CHIP = (accent) => ({
  display: "inline-flex",
  alignItems: "center",
  padding: "1px 8px",
  borderRadius: "var(--ava-radius-pill)",
  fontSize: 11,
  whiteSpace: "nowrap",
  background: "var(--ava-nav-active)",
  color: accent || "var(--ava-label-secondary)"
});

// src/skill-hub-view.tsx
import { jsx, jsxs } from "react/jsx-runtime";
var SEARCH_URL = "https://api.skillhub.cn/api/v1/search";
var RESOLVE_URL = "https://api.skillhub.cn/api/v1/skills/resolve";
var SEARCH_LIMIT = 20;
async function fetchJson(net, url) {
  const result = await net.fetch({ url });
  if (result.status !== 200) {
    throw new Error(`\u4E0A\u6E38\u8FD4\u56DE HTTP ${result.status}`);
  }
  return JSON.parse(result.body);
}
function toSkill(item) {
  const slug = typeof item.slug === "string" ? item.slug.trim() : "";
  if (!slug) return null;
  const str = (key) => {
    const value = item[key];
    return typeof value === "string" ? value.trim() : "";
  };
  const namespace = item.namespace;
  return {
    slug,
    name: str("displayName") || str("name") || slug,
    summary: str("summary") || str("description"),
    version: str("version"),
    source: str("source"),
    downloads: typeof item.downloads === "number" ? item.downloads : 0,
    stars: typeof item.stars === "number" ? item.stars : 0,
    tags: Array.isArray(item.tags) ? item.tags.filter((t) => typeof t === "string" && t.trim() !== "") : [],
    publisher: typeof item.publisher === "object" && item.publisher !== null ? (() => {
      const name = item.publisher.name;
      return typeof name === "string" && name.trim() !== "" ? name.trim() : void 0;
    })() : void 0,
    verified: typeof item.publisher === "object" && item.publisher !== null && item.publisher.verified === true,
    iconUrl: str("icon_url") || void 0,
    homepage: str("homepage") || void 0,
    namespace: typeof namespace === "object" && namespace !== null ? namespace.canonicalName : void 0
  };
}
function toDetail(raw) {
  const skill = raw.skill;
  if (typeof skill !== "object" || skill === null) return null;
  const s = skill;
  const str = (source, key) => {
    const value = source[key];
    return typeof value === "string" ? value.trim() : "";
  };
  const slug = str(s, "slug") || str(raw, "slug");
  if (!slug) return null;
  const stats = typeof s.stats === "object" && s.stats !== null ? s.stats : {};
  const num = (source, key) => typeof source[key] === "number" ? source[key] : 0;
  const latestVersion = raw.latestVersion;
  const owner = raw.owner;
  const namespace = raw.namespace;
  return {
    slug,
    name: str(s, "displayName") || slug,
    summary: str(s, "summary") || str(s, "summary_zh"),
    version: typeof latestVersion === "object" && latestVersion !== null ? str(latestVersion, "version") : "",
    source: str(s, "source"),
    downloads: num(stats, "downloads"),
    installs: num(stats, "installs"),
    stars: num(stats, "stars"),
    owner: typeof owner === "object" && owner !== null ? str(owner, "displayName") || void 0 : void 0,
    namespace: typeof namespace === "object" && namespace !== null ? str(namespace, "canonicalName") || void 0 : void 0,
    sourceUrl: str(s, "sourceUrl") || void 0,
    iconUrl: str(s, "iconUrl") || void 0,
    verified: s.verified === true,
    updatedAt: typeof s.updatedAt === "number" ? s.updatedAt : void 0
  };
}
function formatCount(n) {
  if (n >= 1e4) return `${(n / 1e4).toFixed(1)}w`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}k`;
  return String(n);
}
function SkillHubView({ agent, net, sessionId, onInstalled }) {
  const [results, setResults] = useState(null);
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [installingSlug, setInstallingSlug] = useState("");
  const [installedSlugs, setInstalledSlugs] = useState(/* @__PURE__ */ new Set());
  const [detail, setDetail] = useState(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [err, setErr] = useState("");
  const searchSeq = useRef(0);
  const search = useCallback(async () => {
    const q = query.trim();
    if (!q) {
      setErr("\u8BF7\u8F93\u5165\u641C\u7D22\u5173\u952E\u8BCD");
      return;
    }
    const seq = ++searchSeq.current;
    setSearching(true);
    setErr("");
    setDetail(null);
    try {
      const url = `${SEARCH_URL}?q=${encodeURIComponent(q)}&limit=${SEARCH_LIMIT}`;
      const raw = await fetchJson(net, url);
      const found = (raw.results ?? []).filter((item) => typeof item === "object" && item !== null).map(toSkill).filter((skill) => skill !== null);
      if (seq === searchSeq.current) setResults(found);
    } catch (e) {
      if (seq === searchSeq.current) {
        setResults([]);
        setErr(e instanceof Error ? e.message : String(e));
      }
    } finally {
      if (seq === searchSeq.current) setSearching(false);
    }
  }, [agent, net, query]);
  const openDetail = async (skill) => {
    if (!skill.namespace) return;
    setDetailLoading(true);
    setErr("");
    try {
      const handle = skill.namespace.replace(/^@/, "");
      const coordinate = handle.includes("/") ? handle : `${handle}/${skill.slug}`;
      const url = `${RESOLVE_URL}?coordinate=${encodeURIComponent(`@${coordinate}`)}`;
      const raw = await fetchJson(net, url);
      const parsed = toDetail(raw);
      if (!parsed) throw new Error("\u4E0A\u6E38\u8BE6\u60C5\u6570\u636E\u65E0\u6CD5\u89E3\u6790");
      setDetail(parsed);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setDetailLoading(false);
    }
  };
  const install = async (skill) => {
    if (!sessionId || installingSlug) return;
    setInstallingSlug(skill.slug);
    setErr("");
    try {
      const result = await agent.installSkillHub(skill.slug, sessionId);
      setInstalledSlugs((prev) => new Set(prev).add(skill.slug));
      onInstalled(result.name);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setInstallingSlug("");
    }
  };
  return /* @__PURE__ */ jsxs("div", { style: { display: "flex", flexDirection: "column", gap: "var(--ava-space-4, 16px)" }, children: [
    /* @__PURE__ */ jsxs("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }, children: [
      /* @__PURE__ */ jsx("h3", { style: { margin: 0, fontSize: 14 }, children: "\u6280\u80FD\u5E7F\u573A \xB7 \u817E\u8BAF SkillHub\uFF08\u5F00\u6E90\uFF09" }),
      /* @__PURE__ */ jsx("span", { style: { flex: 1 } }),
      /* @__PURE__ */ jsx(
        "input",
        {
          style: { ...INPUT, width: 260 },
          placeholder: "\u641C\u7D22\u793E\u533A\u6280\u80FD\uFF0C\u5982 pdf\u3001excel\u2026",
          value: query,
          onChange: (e) => setQuery(e.target.value),
          onKeyDown: (e) => {
            if (e.key === "Enter") void search();
          }
        }
      ),
      /* @__PURE__ */ jsx("button", { style: BTN, disabled: searching, onClick: () => void search(), children: searching ? "\u641C\u7D22\u4E2D\u2026" : "\u641C\u7D22" })
    ] }),
    !results && !err ? /* @__PURE__ */ jsx("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u8F93\u5165\u5173\u952E\u8BCD\u641C\u7D22 SkillHub \u5F00\u6E90\u793E\u533A\u6280\u80FD\uFF0C\u67E5\u770B\u8BE6\u60C5\u540E\u4E00\u952E\u5B89\u88C5\u5230\u9879\u76EE `.onion/skills`\u3002" }) : null,
    err ? /* @__PURE__ */ jsx("div", { style: { fontSize: 12, color: "var(--ava-state-danger)" }, children: err }) : null,
    results !== null && results.length === 0 && !searching ? /* @__PURE__ */ jsx("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u65E0\u5339\u914D\u6280\u80FD\uFF0C\u6362\u4E2A\u5173\u952E\u8BCD\u8BD5\u8BD5\u3002" }) : null,
    detailLoading ? /* @__PURE__ */ jsx("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u52A0\u8F7D\u8BE6\u60C5\u4E2D\u2026" }) : null,
    detail ? /* @__PURE__ */ jsxs(
      "div",
      {
        style: {
          border: "0.5px solid var(--ava-border-l2)",
          borderRadius: "var(--ava-radius-lg, 16px)",
          padding: "var(--ava-space-3, 12px) var(--ava-space-4, 16px)",
          background: "var(--ava-bg-layer-1)",
          display: "flex",
          flexDirection: "column",
          gap: 6,
          fontSize: 12
        },
        children: [
          /* @__PURE__ */ jsxs("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }, children: [
            /* @__PURE__ */ jsx("b", { style: { fontSize: 13 }, children: detail.name || detail.slug }),
            /* @__PURE__ */ jsx("span", { style: { fontFamily: "var(--ava-font-mono, monospace)", color: "var(--ava-label-tertiary)" }, children: detail.namespace ?? `@${detail.slug}` }),
            /* @__PURE__ */ jsxs("span", { style: CHIP(), children: [
              "v",
              detail.version || "?"
            ] }),
            detail.verified ? /* @__PURE__ */ jsx("span", { style: CHIP("var(--ava-state-success)"), children: "\u5DF2\u8BA4\u8BC1" }) : null,
            /* @__PURE__ */ jsx("span", { style: { flex: 1 } }),
            /* @__PURE__ */ jsx("button", { style: BTN, onClick: () => setDetail(null), children: "\u6536\u8D77" })
          ] }),
          /* @__PURE__ */ jsx("div", { style: { color: "var(--ava-label-secondary)", whiteSpace: "pre-wrap" }, children: detail.summary }),
          /* @__PURE__ */ jsxs("div", { style: { color: "var(--ava-label-tertiary)", display: "flex", gap: 12, flexWrap: "wrap" }, children: [
            /* @__PURE__ */ jsxs("span", { children: [
              "\u4E0B\u8F7D ",
              formatCount(detail.downloads)
            ] }),
            /* @__PURE__ */ jsxs("span", { children: [
              "\u5B89\u88C5 ",
              formatCount(detail.installs)
            ] }),
            /* @__PURE__ */ jsxs("span", { children: [
              "\u661F\u6807 ",
              detail.stars
            ] }),
            detail.owner ? /* @__PURE__ */ jsxs("span", { children: [
              "\u4F5C\u8005 ",
              detail.owner
            ] }) : null,
            detail.sourceUrl ? /* @__PURE__ */ jsx("a", { href: detail.sourceUrl, target: "_blank", rel: "noreferrer", style: { color: "var(--ava-accent, #4c8dff)" }, children: "\u4E0A\u6E38\u4E3B\u9875" }) : null
          ] })
        ]
      }
    ) : null,
    (results ?? []).map((skill) => /* @__PURE__ */ jsxs(
      "div",
      {
        style: {
          border: "0.5px solid var(--ava-border-l2)",
          borderRadius: "var(--ava-radius-lg, 16px)",
          padding: "var(--ava-space-3, 12px) var(--ava-space-4, 16px)",
          background: "var(--ava-bg-layer-1)",
          display: "flex",
          alignItems: "center",
          gap: "var(--ava-space-2, 8px)"
        },
        children: [
          /* @__PURE__ */ jsx("b", { style: { fontSize: 13, whiteSpace: "nowrap" }, children: skill.name }),
          /* @__PURE__ */ jsx(
            "span",
            {
              style: {
                fontSize: 12,
                color: "var(--ava-label-secondary)",
                flex: 1,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap"
              },
              title: skill.summary,
              children: skill.summary
            }
          ),
          /* @__PURE__ */ jsxs("span", { style: { fontSize: 11, color: "var(--ava-label-tertiary)", whiteSpace: "nowrap" }, children: [
            "v",
            skill.version || "?",
            " \xB7 \u2193",
            formatCount(skill.downloads)
          ] }),
          skill.verified ? /* @__PURE__ */ jsx("span", { style: CHIP("var(--ava-state-success)"), children: "\u8BA4\u8BC1" }) : null,
          skill.namespace ? /* @__PURE__ */ jsx("button", { style: BTN, onClick: () => void openDetail(skill), children: "\u8BE6\u60C5" }) : null,
          installedSlugs.has(skill.slug) ? /* @__PURE__ */ jsx("span", { style: CHIP("var(--ava-state-success)"), children: "\u5DF2\u5B89\u88C5" }) : /* @__PURE__ */ jsx(
            "button",
            {
              style: BTN,
              disabled: installingSlug === skill.slug || !sessionId,
              title: "\u5B89\u88C5\u5230\u9879\u76EE .onion/skills",
              onClick: () => void install(skill),
              children: installingSlug === skill.slug ? "\u5B89\u88C5\u4E2D\u2026" : "\u5B89\u88C5"
            }
          )
        ]
      },
      skill.slug
    ))
  ] });
}

// src/skills-view.tsx
import { Fragment, jsx as jsx2, jsxs as jsxs2 } from "react/jsx-runtime";
var SOURCE_ORDER = [
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
  "bundled"
];
var SOURCE_LABEL = {
  "project-onion": "\u9879\u76EE .onion",
  "project-claude": "\u9879\u76EE .claude",
  "project-dsh": "\u9879\u76EE .dsh",
  "project-agents": "\u9879\u76EE .agents",
  runtime: "\u8FD0\u884C\u65F6\u6CE8\u518C",
  custom: "\u5BBF\u4E3B\u6302\u8F7D",
  "user-onion": "\u7528\u6237 .onion",
  "user-claude": "\u7528\u6237 .claude",
  "user-dsh": "\u7528\u6237 .dsh",
  "user-agents": "\u7528\u6237 .agents",
  bundled: "\u5185\u7F6E"
};
async function resolveSession(agent) {
  const sessions = await agent.listSessions();
  return sessions[0]?.id ?? "";
}
var TAB_LABEL = { local: "\u672C\u5730\u6280\u80FD", hub: "\u6280\u80FD\u5E7F\u573A" };
function SkillsView({ agent, net }) {
  const [skills, setSkills] = useState2(null);
  const [manager, setManager] = useState2(null);
  const [err, setErr] = useState2("");
  const [note, setNote] = useState2("");
  const [query, setQuery] = useState2("");
  const [busyName, setBusyName] = useState2("");
  const [tab, setTab] = useState2("local");
  const [sessionId, setSessionId] = useState2("");
  const load = useCallback2(async () => {
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
        agent.getSkillManager(id)
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
  const toggle = async (skill) => {
    if (!manager || busyName || !sessionId) return;
    const nextDisabled = skill.enabled ? [...manager.disabled, skill.name].sort() : manager.disabled.filter((n) => n !== skill.name);
    const prev = manager;
    setBusyName(skill.name);
    setErr("");
    setNote("");
    setManager({ ...prev, disabled: nextDisabled });
    setSkills(
      (list) => (list ?? []).map((s) => s.name === skill.name ? { ...s, enabled: !s.enabled } : s)
    );
    try {
      const saved = await agent.putSkillManager(nextDisabled, sessionId);
      setManager(saved);
      setNote(skill.enabled ? `\u5DF2\u7981\u7528\u300C${skill.name}\u300D\uFF0C\u4E0B\u4E00\u8F6E\u5BF9\u8BDD\u751F\u6548` : `\u5DF2\u542F\u7528\u300C${skill.name}\u300D`);
    } catch (e) {
      setManager(prev);
      setSkills(
        (list) => (list ?? []).map((s) => s.name === skill.name ? { ...s, enabled: skill.enabled } : s)
      );
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyName("");
    }
  };
  const groups = useMemo(() => {
    const filtered = (skills ?? []).filter((s) => {
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q);
    });
    const bySource = /* @__PURE__ */ new Map();
    for (const skill of filtered) {
      const bucket = bySource.get(skill.source) ?? [];
      bucket.push(skill);
      bySource.set(skill.source, bucket);
    }
    return [...bySource.entries()].sort(
      (a, b) => SOURCE_ORDER.indexOf(a[0]) - SOURCE_ORDER.indexOf(b[0]) || a[0].localeCompare(b[0])
    ).map(([source, items]) => ({
      source,
      items: items.sort((a, b) => a.name.localeCompare(b.name))
    }));
  }, [skills, query]);
  const disabledCount = manager?.disabled.length ?? 0;
  if (err && skills === null) {
    return /* @__PURE__ */ jsxs2("div", { style: { padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }, children: [
      "\u52A0\u8F7D\u5931\u8D25\uFF1A",
      err,
      /* @__PURE__ */ jsx2("button", { style: { ...BTN, marginLeft: 12 }, onClick: () => void load(), children: "\u91CD\u8BD5" })
    ] });
  }
  return /* @__PURE__ */ jsxs2(
    "div",
    {
      style: {
        padding: "var(--ava-space-5, 24px)",
        overflow: "auto",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-4, 16px)"
      },
      children: [
        /* @__PURE__ */ jsxs2("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }, children: [
          /* @__PURE__ */ jsx2("h2", { style: { margin: 0, fontSize: 16 }, children: "\u6280\u80FD\u7BA1\u7406" }),
          /* @__PURE__ */ jsxs2("span", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: [
            skills?.length ?? 0,
            " \u4E2A\u6280\u80FD \xB7 ",
            disabledCount,
            " \u4E2A\u5DF2\u7981\u7528"
          ] }),
          /* @__PURE__ */ jsx2("span", { style: { flex: 1 } }),
          ["local", "hub"].map((key) => /* @__PURE__ */ jsx2(
            "button",
            {
              style: {
                ...BTN,
                ...tab === key ? { background: "var(--ava-nav-active)", fontWeight: 600 } : {}
              },
              onClick: () => setTab(key),
              children: TAB_LABEL[key]
            },
            key
          ))
        ] }),
        note || err ? /* @__PURE__ */ jsx2("div", { style: { fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }, children: err || note }) : null,
        tab === "hub" ? /* @__PURE__ */ jsx2(
          SkillHubView,
          {
            agent,
            net,
            sessionId,
            onInstalled: (name) => {
              setNote(`\u5DF2\u5B89\u88C5\u300C${name}\u300D\u5230\u9879\u76EE .onion/skills\uFF0C\u4E0B\u4E00\u8F6E\u5BF9\u8BDD\u751F\u6548`);
              void load();
            }
          }
        ) : /* @__PURE__ */ jsxs2(Fragment, { children: [
          /* @__PURE__ */ jsxs2("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }, children: [
            /* @__PURE__ */ jsx2(
              "input",
              {
                style: INPUT,
                placeholder: "\u641C\u7D22\u540D\u79F0\u6216\u63CF\u8FF0\u2026",
                value: query,
                onChange: (e) => setQuery(e.target.value)
              }
            ),
            /* @__PURE__ */ jsx2("button", { style: BTN, onClick: () => void load(), children: "\u5237\u65B0" })
          ] }),
          manager?.file ? /* @__PURE__ */ jsxs2("div", { style: { fontSize: 11, color: "var(--ava-label-tertiary)" }, children: [
            "\u7981\u7528\u6E05\u5355\uFF1A",
            /* @__PURE__ */ jsx2("span", { style: { fontFamily: "var(--ava-font-mono, monospace)" }, children: manager.file })
          ] }) : null,
          skills !== null && skills.length === 0 ? /* @__PURE__ */ jsx2("div", { style: { padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }, children: "\u5C1A\u65E0\u53EF\u7528\u4F1A\u8BDD\u6216\u6280\u80FD\u2014\u2014\u5148\u521B\u5EFA\u4F1A\u8BDD\uFF08\u7981\u7528\u6E05\u5355\u6309\u4F1A\u8BDD\u5DE5\u4F5C\u533A\u89E3\u6790\uFF09\uFF0C\u6216\u5728\u9879\u76EE `.onion/skills` \u7B49\u76EE\u5F55\u653E\u7F6E\u6280\u80FD\u3002" }) : null,
          groups.map((group) => /* @__PURE__ */ jsxs2("div", { style: { display: "flex", flexDirection: "column", gap: 8 }, children: [
            /* @__PURE__ */ jsxs2("div", { style: { fontSize: 12, color: "var(--ava-label-secondary)", fontWeight: 600 }, children: [
              SOURCE_LABEL[group.source] ?? group.source,
              /* @__PURE__ */ jsx2("span", { style: { marginLeft: 8, fontWeight: 400, color: "var(--ava-label-tertiary)" }, children: group.items.length })
            ] }),
            group.items.map((skill) => /* @__PURE__ */ jsxs2(
              "div",
              {
                style: {
                  border: "0.5px solid var(--ava-border-l2)",
                  borderRadius: "var(--ava-radius-lg, 16px)",
                  padding: "var(--ava-space-3, 12px) var(--ava-space-4, 16px)",
                  background: "var(--ava-bg-layer-1)",
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--ava-space-2, 8px)",
                  opacity: skill.enabled ? 1 : 0.55
                },
                children: [
                  /* @__PURE__ */ jsx2("b", { style: { fontSize: 13, whiteSpace: "nowrap" }, children: skill.name }),
                  /* @__PURE__ */ jsx2(
                    "span",
                    {
                      style: {
                        fontSize: 12,
                        color: "var(--ava-label-secondary)",
                        flex: 1,
                        minWidth: 0,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap"
                      },
                      title: skill.description,
                      children: skill.description
                    }
                  ),
                  !skill.modelInvocable ? /* @__PURE__ */ jsx2("span", { style: CHIP("var(--ava-label-tertiary)"), children: "\u6A21\u578B\u4E0D\u53EF\u8C03" }) : null,
                  !skill.userInvocable ? /* @__PURE__ */ jsx2("span", { style: CHIP("var(--ava-label-tertiary)"), children: "\u624B\u52BF\u4E0D\u53EF\u8C03" }) : null,
                  /* @__PURE__ */ jsx2("span", { style: CHIP(skill.enabled ? "var(--ava-state-success)" : "var(--ava-state-danger)"), children: skill.enabled ? "\u542F\u7528\u4E2D" : "\u5DF2\u7981\u7528" }),
                  /* @__PURE__ */ jsx2("button", { style: BTN, disabled: busyName === skill.name, onClick: () => void toggle(skill), children: busyName === skill.name ? "\u4FDD\u5B58\u4E2D\u2026" : skill.enabled ? "\u7981\u7528" : "\u542F\u7528" })
                ]
              },
              `${skill.provider}:${skill.name}`
            ))
          ] }, group.source))
        ] })
      ]
    }
  );
}

// src/main.tsx
import { jsx as jsx3 } from "react/jsx-runtime";
function setup(ctx) {
  const disposers = [
    // lucide sliders-horizontal：启停语义
    ctx.slots.inject("nav.sidebar", {
      order: 12,
      viewId: "skills-manager",
      title: "\u6280\u80FD\u7BA1\u7406",
      icon: "M4 5h7 M4 10h9 M4 15h6 M17 4v6 M17 14v6 M14 7h6 M14 17h6"
    }),
    ctx.slots.inject("surface.main", {
      viewId: "skills-manager",
      title: "\u6280\u80FD\u7BA1\u7406",
      component: async () => ({ default: () => /* @__PURE__ */ jsx3(SkillsView, { agent: ctx.agent, net: ctx.net }) })
    })
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    }
  };
}
export {
  setup
};
