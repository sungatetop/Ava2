// 技能广场视图：腾讯 SkillHub 社区市场（初始热门列表 → 搜索 → 详情 → 安装到项目 .onion/skills）。
// 读面（搜索/详情）经统一联网服务 `ctx.net.fetch` 直连上游公开 API 并在本文件
// 归一化（上游契约：api.skillhub.cn/api/v1/search 与 /api/v1/skills/resolve）；
// 安装走会话面（POST /v1/skill-hub/install?session=<id>）——zip 落盘与注册表刷新
// 必须发生在服务端。只面向开源（社区）技能：企业源下载 404 由安装报错自然呈现。
// 展示对齐 ava_square 技能广场：头部市场卡 + 骨架屏 + 卡片网格（图标/徽标/统计/
// footer 外链与主按钮）+ 分页。
import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { AgentChannel, NetApi, SkillHubDetail, SkillHubSkill } from "@ava-ai/app-sdk";
import { BTN, BTN_PRIMARY, CHIP, GRID_STYLE, INPUT } from "./styles";

interface SkillHubViewProps {
  agent: AgentChannel;
  /** 统一联网服务（ctx.net）：直连上游搜索/详情 */
  net: NetApi;
  /** 最近会话 id（安装目标工作区来源） */
  sessionId: string;
  /** 安装成功后回调（父视图刷新本地目录） */
  onInstalled: (installedName: string) => void;
}

/** 上游搜索 / 详情端点（公开匿名可用） */
const SEARCH_URL = "https://api.skillhub.cn/api/v1/search";
const RESOLVE_URL = "https://api.skillhub.cn/api/v1/skills/resolve";

/** 每页条数（对齐 ava_square 广场） */
const PAGE_SIZE = 24;

/** 上游分类标识 → 中文展示名（未收录分类回退原文） */
const CATEGORY_LABEL: Record<string, string> = {
  "dev-programming": "开发编程",
  "office-efficiency": "办公效率",
  "data-analysis": "数据分析",
  "design-creation": "设计创作",
  "design-media": "设计媒体",
  "education-learning": "教育学习",
  education: "教育学习",
  "life-entertainment": "生活娱乐",
  "life-service": "生活服务",
  "professional-industry": "专业行业",
  professional: "专业行业",
  "ai-agent": "智能体",
  "it-ops-security": "运维安全",
  "content-creation": "内容创作",
  "knowledge-management": "知识管理",
  "business-ops": "商业运营",
};

/** 卡片视图类型：SDK 契约 + 本地展示字段（分类） */
type HubCard = SkillHubSkill & { category: string };

/** 强调色（链接/选中，契约唯一语义令牌） */
const ACCENT = "var(--ava-state-business-primary)";

/** lucide 风格描边图标（与 AppHost 图标范式一致：24 viewBox / currentColor） */
function SvgIcon({ children, size = 14 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={{ flexShrink: 0 }}
    >
      {children}
    </svg>
  );
}

/** Globe（头部市场标识） */
function GlobeIcon({ size }: { size?: number }) {
  return (
    <SvgIcon size={size}>
      <circle cx="12" cy="12" r="10" />
      <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
      <path d="M2 12h20" />
    </SvgIcon>
  );
}

/** Package（空态/兜底图标） */
function PackageIcon({ size }: { size?: number }) {
  return (
    <SvgIcon size={size}>
      <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
      <path d="m3.3 7 8.7 5 8.7-5" />
      <path d="M12 22V12" />
    </SvgIcon>
  );
}

/** User（发布者） */
function UserIcon() {
  return (
    <SvgIcon>
      <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </SvgIcon>
  );
}

/** Download（下载量） */
function DownloadIcon() {
  return (
    <SvgIcon>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" x2="12" y1="15" y2="3" />
    </SvgIcon>
  );
}

/** Star（星标） */
function StarIcon() {
  return (
    <SvgIcon>
      <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26" />
    </SvgIcon>
  );
}

/** ExternalLink（上游主页外链） */
function ExternalLinkIcon() {
  return (
    <SvgIcon size={13}>
      <path d="M15 3h6v6" />
      <path d="M10 14 21 3" />
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
    </SvgIcon>
  );
}

/** 外联取回 + JSON 解析（非 200 → 抛带状态码的 Error） */
async function fetchJson(net: NetApi, url: string): Promise<unknown> {
  const result = await net.fetch({ url });
  if (result.status !== 200) {
    throw new Error(`上游返回 HTTP ${result.status}`);
  }
  return JSON.parse(result.body) as unknown;
}

/** 上游搜索条目 → 规整视图（脏数据尽力兜底，slug 缺失剔除） */
function toCard(item: Record<string, unknown>): HubCard | null {
  const slug = typeof item.slug === "string" ? item.slug.trim() : "";
  if (!slug) return null;
  const str = (key: string): string => {
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
    tags: Array.isArray(item.tags)
      ? item.tags.filter((t): t is string => typeof t === "string" && t.trim() !== "")
      : [],
    category: str("category"),
    publisher:
      typeof item.publisher === "object" && item.publisher !== null
        ? (() => {
            const name = (item.publisher as Record<string, unknown>).name;
            return typeof name === "string" && name.trim() !== "" ? name.trim() : undefined;
          })()
        : undefined,
    verified:
      typeof item.publisher === "object" &&
      item.publisher !== null &&
      (item.publisher as Record<string, unknown>).verified === true,
    iconUrl: str("icon_url") || undefined,
    homepage: str("homepage") || undefined,
    namespace:
      typeof namespace === "object" && namespace !== null
        ? ((namespace as Record<string, unknown>).canonicalName as string | undefined)
        : undefined,
  };
}

/** 上游 resolve 响应 → 详情视图 */
function toDetail(raw: Record<string, unknown>): SkillHubDetail | null {
  const skill = raw.skill;
  if (typeof skill !== "object" || skill === null) return null;
  const s = skill as Record<string, unknown>;
  const str = (source: Record<string, unknown>, key: string): string => {
    const value = source[key];
    return typeof value === "string" ? value.trim() : "";
  };
  const slug = str(s, "slug") || str(raw, "slug");
  if (!slug) return null;
  const stats =
    typeof s.stats === "object" && s.stats !== null
      ? (s.stats as Record<string, unknown>)
      : {};
  const num = (source: Record<string, unknown>, key: string): number =>
    typeof source[key] === "number" ? source[key] : 0;
  const latestVersion = raw.latestVersion;
  const owner = raw.owner;
  const namespace = raw.namespace;
  return {
    slug,
    name: str(s, "displayName") || slug,
    summary: str(s, "summary") || str(s, "summary_zh"),
    version:
      typeof latestVersion === "object" && latestVersion !== null
        ? str(latestVersion as Record<string, unknown>, "version")
        : "",
    source: str(s, "source"),
    downloads: num(stats, "downloads"),
    installs: num(stats, "installs"),
    stars: num(stats, "stars"),
    owner:
      typeof owner === "object" && owner !== null
        ? str(owner as Record<string, unknown>, "displayName") || undefined
        : undefined,
    namespace:
      typeof namespace === "object" && namespace !== null
        ? str(namespace as Record<string, unknown>, "canonicalName") || undefined
        : undefined,
    sourceUrl: str(s, "sourceUrl") || undefined,
    iconUrl: str(s, "iconUrl") || undefined,
    verified: s.verified === true,
    updatedAt: typeof s.updatedAt === "number" ? s.updatedAt : undefined,
  };
}

/** 下载量展示（1.2k / 3.4w 简写） */
function formatCount(n: number): string {
  if (n >= 10000) return `${(n / 10000).toFixed(1)}w`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

/** 加载骨架屏（对齐 ava_square：网格占位卡片 + pulse） */
function SkeletonGrid() {
  return (
    <div style={GRID_STYLE}>
      {Array.from({ length: 8 }, (_, i) => (
        <div
          key={i}
          style={{
            height: 176,
            borderRadius: "var(--ava-radius-panel, 28px)",
            background: "var(--ava-nav-active)",
            animation: "ava-sk-pulse 1.2s ease-in-out infinite",
          }}
        />
      ))}
      <style>{`@keyframes ava-sk-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.45; } }`}</style>
    </div>
  );
}

/** 技能广场主视图（初始热门 → 搜索/翻页 → 卡片网格 → 详情/安装） */
export function SkillHubView({ agent, net, sessionId, onInstalled }: SkillHubViewProps) {
  const [results, setResults] = useState<HubCard[] | null>(null);
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [installingSlug, setInstallingSlug] = useState("");
  const [installedSlugs, setInstalledSlugs] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<SkillHubDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [err, setErr] = useState("");
  const [page, setPage] = useState(1);
  const searchSeq = useRef(0);

  /** 搜索/翻页加载（q 为空即拉取热门列表——初始进入直接展示技能） */
  const load = useCallback(
    async (q: string, p: number) => {
      const seq = ++searchSeq.current;
      setSearching(true);
      setErr("");
      setDetail(null);
      try {
        const url = `${SEARCH_URL}?q=${encodeURIComponent(q.trim())}&page=${p}&page_size=${PAGE_SIZE}`;
        const raw = (await fetchJson(net, url)) as { results?: unknown[] };
        const found = (raw.results ?? [])
          .filter((item): item is Record<string, unknown> => typeof item === "object" && item !== null)
          .map(toCard)
          .filter((skill): skill is HubCard => skill !== null);
        if (seq === searchSeq.current) setResults(found);
      } catch (e) {
        if (seq === searchSeq.current) {
          setResults([]);
          setErr(e instanceof Error ? e.message : String(e));
        }
      } finally {
        if (seq === searchSeq.current) setSearching(false);
      }
    },
    [net],
  );

  // 初始进入：空关键词拉热门列表（对齐 ava_square 广场，不再空白等待搜索）
  useEffect(() => {
    void load("", 1);
  }, [load]);

  const search = () => {
    setPage(1);
    void load(query, 1);
  };

  const openDetail = async (skill: SkillHubSkill) => {
    if (!skill.namespace) return;
    setDetailLoading(true);
    setErr("");
    try {
      // namespace 兼容 handle（`ns` / `@ns`）与 canonical 全坐标（`@ns/slug`）。
      const handle = skill.namespace.replace(/^@/, "");
      const coordinate = handle.includes("/") ? handle : `${handle}/${skill.slug}`;
      const url = `${RESOLVE_URL}?coordinate=${encodeURIComponent(`@${coordinate}`)}`;
      const raw = (await fetchJson(net, url)) as Record<string, unknown>;
      const parsed = toDetail(raw);
      if (!parsed) throw new Error("上游详情数据无法解析");
      setDetail(parsed);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setDetailLoading(false);
    }
  };

  const install = async (skill: SkillHubSkill) => {
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

  // 翻页可用性：满页才有下一页（上游无 total，用满页启发）
  const hasMore = (results?.length ?? 0) >= PAGE_SIZE;
  const gotoPage = (next: number) => {
    setPage(next);
    void load(query, next);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--ava-space-4, 16px)" }}>
      {/* 头部市场卡（对齐 ava_square：图标块 + 市场名与计数 + 搜索框） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--ava-space-3, 10px)",
          flexWrap: "wrap",
          padding: "var(--ava-space-3, 10px) var(--ava-space-4, 16px)",
          border: "0.5px solid var(--ava-border-l2)",
          borderRadius: "var(--ava-radius-xl, 20px)",
          background: "var(--ava-bg-layer-1)",
        }}
      >
        <div
          style={{
            width: 32,
            height: 32,
            borderRadius: "var(--ava-radius-md, 12px)",
            background: "var(--ava-nav-active)",
            color: ACCENT,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <GlobeIcon size={16} />
        </div>
        <div style={{ display: "flex", flexDirection: "column" }}>
          <b style={{ fontSize: 13 }}>技能广场 · 腾讯 SkillHub（开源）</b>
          <span style={{ fontSize: 11, color: "var(--ava-label-tertiary)" }}>
            {searching
              ? "加载中…"
              : results !== null
                ? `本页 ${results.length} 个技能 · 搜索后一键安装到项目 .onion/skills`
                : "连接开源技能市场…"}
          </span>
        </div>
        <span style={{ flex: 1 }} />
        <input
          style={{ ...INPUT, width: 260 }}
          placeholder="搜索社区技能，如 pdf、excel…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") search();
          }}
        />
        <button style={BTN} disabled={searching} onClick={search}>
          搜索
        </button>
      </div>
      {err ? (
        <div style={{ fontSize: 12, color: "var(--ava-state-danger)" }}>{err}</div>
      ) : null}
      {detailLoading ? <div style={{ fontSize: 12, color: "var(--ava-label-tertiary)" }}>加载详情中…</div> : null}
      {detail ? (
        <div
          style={{
            border: "0.5px solid var(--ava-border-l2)",
            borderRadius: "var(--ava-radius-xl, 20px)",
            padding: "var(--ava-space-3, 12px) var(--ava-space-4, 16px)",
            background: "var(--ava-bg-layer-1)",
            display: "flex",
            flexDirection: "column",
            gap: 6,
            fontSize: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }}>
            <b style={{ fontSize: 13 }}>{detail.name || detail.slug}</b>
            <span style={{ fontFamily: "var(--ava-font-mono, monospace)", color: "var(--ava-label-tertiary)" }}>
              {detail.namespace ?? `@${detail.slug}`}
            </span>
            <span style={CHIP()}>v{detail.version || "?"}</span>
            {detail.verified ? <span style={CHIP("var(--ava-state-success)")}>已认证</span> : null}
            <span style={{ flex: 1 }} />
            <button style={BTN} onClick={() => setDetail(null)}>
              收起
            </button>
          </div>
          <div style={{ color: "var(--ava-label-secondary)", whiteSpace: "pre-wrap" }}>{detail.summary}</div>
          <div style={{ color: "var(--ava-label-tertiary)", display: "flex", gap: 12, flexWrap: "wrap" }}>
            <span>下载 {formatCount(detail.downloads)}</span>
            <span>安装 {formatCount(detail.installs)}</span>
            <span>星标 {detail.stars}</span>
            {detail.owner ? <span>作者 {detail.owner}</span> : null}
            {detail.sourceUrl ? (
              <a href={detail.sourceUrl} target="_blank" rel="noreferrer" style={{ color: ACCENT }}>
                上游主页
              </a>
            ) : null}
          </div>
        </div>
      ) : null}
      {/* 网格：加载骨架 / 空态 / 卡片网格（对齐 ava_square） */}
      {searching ? (
        <SkeletonGrid />
      ) : results !== null && results.length === 0 ? (
        <div
          style={{
            textAlign: "center",
            padding: "48px 0",
            border: "1px dashed var(--ava-border-l2)",
            borderRadius: "var(--ava-radius-panel, 28px)",
            color: "var(--ava-label-tertiary)",
            fontSize: 12,
          }}
        >
          <PackageIcon size={40} />
          <div style={{ marginTop: 12, fontWeight: 600 }}>未找到匹配的开源技能</div>
        </div>
      ) : (
        <div style={GRID_STYLE}>
          {(results ?? []).map((skill) => (
            <SkillCard
              key={skill.slug}
              skill={skill}
              installed={installedSlugs.has(skill.slug)}
              installing={installingSlug === skill.slug}
              installDisabled={!sessionId || installingSlug !== ""}
              onDetail={() => void openDetail(skill)}
              onInstall={() => void install(skill)}
            />
          ))}
        </div>
      )}
      {/* 分页（上游无 total，满页启发式判下一页） */}
      {(page > 1 || hasMore) && !searching ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 12 }}>
          <button style={BTN} disabled={page <= 1 || searching} onClick={() => gotoPage(page - 1)}>
            上一页
          </button>
          <span style={{ fontSize: 12, color: "var(--ava-label-secondary)" }}>第 {page} 页</span>
          <button style={BTN} disabled={!hasMore || searching} onClick={() => gotoPage(page + 1)}>
            下一页
          </button>
        </div>
      ) : null}
    </div>
  );
}

/** 广场卡片（对齐 ava_square：大圆角软卡片 + hover 浮起/图标放大/标题变色） */
function SkillCard(props: {
  skill: HubCard;
  installed: boolean;
  installing: boolean;
  installDisabled: boolean;
  onDetail: () => void;
  onInstall: () => void;
}) {
  const { skill, installed, installing, installDisabled, onDetail, onInstall } = props;
  const [hover, setHover] = useState(false);
  const [iconBroken, setIconBroken] = useState(false);
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
    transform: hover ? "translateY(-2px)" : "none",
    boxShadow: hover ? "var(--ava-shadow-panel)" : "none",
    transition: "transform 0.15s ease, box-shadow 0.15s ease",
  };
  return (
    <div style={cardStyle} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
      {/* 顶行：图标（hover 放大）+ 分类/版本/认证徽标 */}
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 8 }}>
        {skill.iconUrl && !iconBroken ? (
          <img
            src={skill.iconUrl}
            alt=""
            width={40}
            height={40}
            loading="lazy"
            onError={() => setIconBroken(true)}
            style={{
              borderRadius: "var(--ava-radius-xl, 20px)",
              objectFit: "cover",
              flexShrink: 0,
              transform: hover ? "scale(1.08)" : "none",
              transition: "transform 0.15s ease",
            }}
          />
        ) : (
          <div
            style={{
              width: 40,
              height: 40,
              borderRadius: "var(--ava-radius-xl, 20px)",
              background: "var(--ava-nav-active)",
              color: ACCENT,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              transform: hover ? "scale(1.08)" : "none",
              transition: "transform 0.15s ease",
            }}
          >
            <PackageIcon size={20} />
          </div>
        )}
        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", justifyContent: "flex-end" }}>
          {skill.category ? (
            <span style={CHIP()}>{CATEGORY_LABEL[skill.category] ?? skill.category}</span>
          ) : null}
          {skill.version ? <span style={CHIP()}>v{skill.version}</span> : null}
          {skill.verified ? <span style={CHIP("var(--ava-state-success)")}>已认证</span> : null}
        </div>
      </div>
      {/* 名称（hover 变强调色，对齐 ava_square group-hover） */}
      <b
        style={{
          fontSize: 14,
          color: hover ? ACCENT : "var(--ava-label-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          transition: "color 0.15s ease",
        }}
        title={skill.name}
      >
        {skill.name}
      </b>
      {/* 摘要（两行截断） */}
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
        title={skill.summary || "（无描述）"}
      >
        {skill.summary || "（无描述）"}
      </div>
      {/* 统计行（图标 + 数值，星标琥珀色） */}
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
        {skill.publisher ? (
          <span
            style={{ display: "inline-flex", alignItems: "center", gap: 4, minWidth: 0, maxWidth: "45%" }}
            title={skill.publisher}
          >
            <UserIcon />
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {skill.publisher}
            </span>
          </span>
        ) : null}
        <span style={{ display: "inline-flex", alignItems: "center", gap: 4, whiteSpace: "nowrap" }}>
          <DownloadIcon /> {formatCount(skill.downloads)}
        </span>
        {skill.stars > 0 ? (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              whiteSpace: "nowrap",
              color: "var(--ava-state-warning)",
            }}
          >
            <StarIcon /> {formatCount(skill.stars)}
          </span>
        ) : null}
      </div>
      {/* footer：上游外链 + 详情 + 主安装按钮（对齐 ava_square 卡片底部） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          paddingTop: "var(--ava-space-3, 10px)",
          borderTop: "0.5px solid var(--ava-border-l2)",
        }}
      >
        {skill.homepage ? (
          <a
            href={skill.homepage}
            target="_blank"
            rel="noreferrer"
            onClick={(e) => e.stopPropagation()}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              fontSize: 11,
              fontWeight: 600,
              color: hover ? ACCENT : "var(--ava-label-tertiary)",
              textDecoration: "none",
              whiteSpace: "nowrap",
              transition: "color 0.15s ease",
            }}
          >
            <ExternalLinkIcon /> 在 SkillHub 查看
          </a>
        ) : (
          <span />
        )}
        <span style={{ flex: 1 }} />
        {skill.namespace ? (
          <button style={{ ...BTN, ...actionStyle }} onClick={onDetail}>
            详情
          </button>
        ) : null}
        {installed ? (
          <span style={CHIP("var(--ava-state-success)")}>已安装</span>
        ) : (
          <button
            style={{ ...BTN_PRIMARY, ...actionStyle }}
            disabled={installing || installDisabled}
            title="安装到项目 .onion/skills"
            onClick={onInstall}
          >
            {installing ? "安装中…" : "安装"}
          </button>
        )}
      </div>
    </div>
  );
}
