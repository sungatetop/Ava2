// 热榜聚合 App（形态 B 构建型：esbuild → dist/main.js，manifest 经 contributes.apps 声明）
// 数据面：百度热搜 + 头条热榜，均经唯一外联出口 ctx.net.fetch 直连上游公开 API；
// net.allow 在 manifest 中收窄为这两域，越权调用会 fail-loud。
// 持久化面：ctx.storage（app:hotboard: 命名空间）——收藏与列表缓存跨标签切换/挂起存活
// （非激活视图会被卸载，组件内 state 不能当存储用）。
// 组件拿不到 ctx：经闭包 props 注入（task-manager 同构模式）。
import { useEffect, useMemo, useRef, useState } from "react";
import type { AppContext } from "@ava-ai/app-sdk";
import { css } from "./styles";

/** 归一化榜单条目 */
interface Item {
  rank: number;
  title: string;
  url: string;
  hot: number;
  tag: string;
}

/** 收藏条目（本地持久化，上限 100 条落盘） */
interface Fav {
  title: string;
  url: string;
  hot: number;
  ts: number;
  source: string;
}

/** 列表缓存（storage 形状：cache:<sourceKey>） */
interface CacheEntry {
  v?: number;
  items?: Item[];
  ts?: number;
}

/** 数据源注册表：key → 上游端点与归一化函数（脏数据尽力兜底，缺字段剔除） */
const SOURCES: Source[] = [
  {
    key: "baidu",
    label: "百度热搜",
    url: "https://top.baidu.com/api/board?platform=wise&tab=realtime",
    parse(body: string): Item[] {
      const j = JSON.parse(body);
      const cards = Array.isArray(j?.data?.cards) ? j.data.cards : [];
      const groups = [];
      for (const card of cards) {
        if (!card || !Array.isArray(card.content)) continue;
        for (const g of card.content) {
          if (Array.isArray(g?.content)) groups.push(...g.content);
        }
        if (groups.length) break;
      }
      const items: Item[] = [];
      let rank = 0;
      for (const it of groups) {
        const title = typeof it.word === "string" ? it.word.trim() : "";
        if (!title) continue;
        if (it.isTop) continue; // 置顶运营位不参与排名
        rank += 1;
        const labelTag = it.labelTag && typeof it.labelTag.text === "string" ? it.labelTag.text : "";
        const rawTag = (typeof it.newHotName === "string" && it.newHotName) || labelTag || "";
        items.push({
          rank,
          title,
          url: typeof it.url === "string" ? it.url : "",
          // 该接口已无 hotScore 字段（实测 50 条 0 命中），热度改由排名归一化推导
          hot: Number(it.hotScore) || 0,
          // 「热」在本榜高频出现（约 1/4 条目），无区分度，降噪丢弃
          tag: rawTag === "热" ? "" : rawTag,
        });
      }
      return items;
    },
  },
  {
    key: "toutiao",
    label: "头条热榜",
    url: "https://www.toutiao.com/hot-event/hot-board/?origin=toutiao_pc",
    parse(body: string): Item[] {
      const j = JSON.parse(body);
      const arr = Array.isArray(j?.data) ? j.data : [];
      const items: Item[] = [];
      let rank = 0;
      for (const it of arr) {
        const title = typeof it.Title === "string" ? it.Title.trim() : "";
        if (!title) continue;
        rank += 1;
        const label = typeof it.Label === "string" ? it.Label : "";
        items.push({
          rank,
          title,
          url: typeof it.Url === "string" ? it.Url : "",
          hot: Number(it.HotValue) || 0,
          tag: label === "new" ? "新" : label === "boil" ? "沸" : "",
        });
      }
      return items;
    },
  },
];

interface Source {
  key: string;
  label: string;
  url: string;
  parse(body: string): Item[];
}

/** 热度数值 → 中文量级（万 / 亿） */
function fmtHot(v: number): string {
  if (!Number.isFinite(v) || v <= 0) return "";
  if (v >= 1e8) return `${(v / 1e8).toFixed(1)} 亿`;
  if (v >= 1e4) return `${Math.round(v / 1e4)} 万`;
  return String(Math.round(v));
}

/** HH:MM:SS */
function fmtClock(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/** 取回一个源的榜单并归一化（非 200 / 解析失败均抛出可读 Error） */
async function fetchBoard(ctx: AppContext, src: Source): Promise<Item[]> {
  const res = await ctx.net.fetch({ url: src.url });
  if (res.status !== 200) throw new Error(`上游返回 HTTP ${res.status}`);
  return src.parse(res.body);
}

/** 收藏键（url 全局唯一即可去重） */
const favKey = (it: Pick<Item, "url" | "title">) => it.url || it.title;

/** lucide refresh-cw */
const ICON_REFRESH = "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8 M21 3v5h-5 M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16 M8 16H3v5";
/** lucide bookmark */
const ICON_BOOKMARK = "m19 21-5.54-5.54a1 1 0 0 0-1.42-.01l-2.56 2.56 M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z";
/** lucide external-link */
const ICON_EXT = "M15 3h6v6 M10 14 21 3 M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6";
/** lucide flame（导航 + 脚注共用） */
const FLAME = "M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z";
/** lucide circle-alert（错误态图标槽） */
const ICON_ALERT = "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z M12 8v4 M12 16h.01";
/** lucide inbox（空态图标槽） */
const ICON_INBOX = "M22 12h-6l-2 3h-4l-2-3H2 M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z";

function Svg({ d, filled, size = 16 }: { d: string; filled?: boolean; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={d} />
    </svg>
  );
}

/** TOP 3 卡片：排名数字块 + 标题 + 热度条，视觉上与其余列表分层 */
function TopCard({
  item,
  ratio,
  faved,
  onToggleFav,
}: {
  item: Item;
  ratio: number;
  faved: boolean;
  onToggleFav: () => void;
}) {
  const tagCls = item.tag === "新" ? " ok" : item.tag === "沸" ? " boil" : "";
  return (
    <li className="hb-card">
      <span className="hb-card-rank">{item.rank}</span>
      <div className="hb-card-body">
        {item.url ? (
          <a className="hb-card-title" href={item.url} target="_blank" rel="noreferrer" title={item.title}>
            {item.title}
          </a>
        ) : (
          <span className="hb-card-title">{item.title}</span>
        )}
        <div className="hb-card-meta">
          {item.tag && <span className={`hb-tag${tagCls}`}>{item.tag}</span>}
          {item.hot > 0 && <span className="hb-hot">{fmtHot(item.hot)}</span>}
          <span className="hb-bar" aria-hidden="true">
            <i style={{ width: `${ratio}%` }} />
          </span>
        </div>
      </div>
      <span className="hb-act">
        {item.url && (
          <a className="hb-iconbtn" href={item.url} target="_blank" rel="noreferrer" title="打开原链接">
            <Svg d={ICON_EXT} size={15} />
          </a>
        )}
        <button
          type="button"
          className={`hb-iconbtn${faved ? " faved" : ""}`}
          title={faved ? "取消收藏" : "收藏热点"}
          onClick={onToggleFav}
        >
          <Svg d={ICON_BOOKMARK} size={15} filled={faved} />
        </button>
      </span>
    </li>
  );
}

function HotboardView({ ctx }: { ctx: AppContext }) {
  const [active, setActive] = useState(SOURCES[0].key);
  const [items, setItems] = useState<Record<string, Item[]>>({}); // key → 归一化条目
  const [ts, setTs] = useState<Record<string, number>>({}); // key → 成功时间戳
  const [err, setErr] = useState<Record<string, string>>({}); // key → 错误信息
  const [loading, setLoading] = useState(true);
  const [favs, setFavs] = useState<Fav[]>([]);
  const [tick, setTick] = useState(Date.now());
  const aliveRef = useRef(true);
  const activeRef = useRef(SOURCES[0].key);

  useEffect(() => {
    activeRef.current = active;
  }, [active]);

  const activeSrc = useMemo(() => SOURCES.find((s) => s.key === active) || SOURCES[0], [active]);
  const favedKeys = useMemo(() => new Set(favs.map(favKey)), [favs]);

  const persist = (key: string, value: unknown) => {
    ctx.storage.set(key, value).catch(() => {});
  };

  const load = async (src: Source, quiet?: boolean) => {
    if (!quiet) setLoading(true);
    setErr((m) => ({ ...m, [src.key]: "" }));
    try {
      const list = await fetchBoard(ctx, src);
      if (!aliveRef.current) return;
      setItems((m) => ({ ...m, [src.key]: list }));
      const now = Date.now();
      setTs((m) => ({ ...m, [src.key]: now }));
      setTick(now);
      persist(`cache:${src.key}`, { v: 1, items: list.slice(0, 30), ts: now });
    } catch (e) {
      if (!aliveRef.current) return;
      setErr((m) => ({ ...m, [src.key]: e instanceof Error ? e.message : String(e) }));
    } finally {
      if (aliveRef.current) setLoading(false);
    }
  };

  useEffect(() => {
    aliveRef.current = true;
    // 恢复缓存与收藏（标签切回即有内容，不等网络）
    (async () => {
      const favList = await ctx.storage.get<Fav[]>("favorites").catch(() => undefined);
      const caches = await Promise.all(
        SOURCES.map((s) => ctx.storage.get<CacheEntry>(`cache:${s.key}`).catch(() => undefined)),
      );
      if (!aliveRef.current) return;
      SOURCES.forEach((s, i) => {
        const c = caches[i];
        if (c && Array.isArray(c.items) && c.items.length) {
          setItems((m) => ({ ...m, [s.key]: c.items as Item[] }));
          setTs((m) => ({ ...m, [s.key]: c.ts || 0 }));
        }
      });
      if (Array.isArray(favList)) setFavs(favList);
    })();
    SOURCES.forEach((s) => load(s));
    const timer = setInterval(() => {
      const cur = SOURCES.find((s) => s.key === activeRef.current);
      if (cur) load(cur, true); // 静默自动刷新当前源
    }, 5 * 60 * 1000);
    return () => {
      aliveRef.current = false;
      clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleFav = (item: Pick<Item, "title" | "url" | "hot">) => {
    if (!item.title) return;
    setFavs((list) => {
      const key = favKey(item);
      const next = list.some((f) => favKey(f) === key)
        ? list.filter((f) => favKey(f) !== key)
        : [{ title: item.title, url: item.url || "", hot: item.hot || 0, ts: Date.now(), source: activeSrc.label }, ...list];
      persist("favorites", next.slice(0, 100));
      return next;
    });
  };

  const list = items[active] || [];
  const maxHot = list.reduce((m, it) => Math.max(m, it.hot), 0);
  // 上游可能不提供热度数值（百度已移除 hotScore）。此时热度条退化为按排名
  // 推导的相对热度：真实热度呈指数衰减，线性归一化会让头部若干条几乎等长，
  // 故用指数曲线拉开梯度，仅作视觉密度指示，不展示数值。
  const heatRatio = (it: Item): number => {
    if (maxHot > 0) return Math.max(6, Math.round((it.hot / maxHot) * 100));
    return Math.max(6, Math.round(Math.exp(-(it.rank - 1) / 7) * 100));
  };

  return (
    <div className="hb-root">
      <style>{css}</style>
      {/* 工具栏 */}
      <div className="hb-toolbar">
        <div className="hb-tabs" role="tablist">
          {SOURCES.map((s) => (
            <button
              key={s.key}
              type="button"
              role="tab"
              aria-selected={active === s.key}
              className={`hb-tab${active === s.key ? " on" : ""}`}
              onClick={() => setActive(s.key)}
            >
              {s.label}
            </button>
          ))}
        </div>
        <div className="hb-spacer" />
        {ts[active] ? (
          <span className="hb-time">更新 {fmtClock(ts[active])} · 每 5 分钟自动刷新</span>
        ) : (
          <span className="hb-time">等待首次刷新…</span>
        )}
        <button
          type="button"
          className="hb-btn"
          disabled={loading}
          onClick={() => load(activeSrc)}
          title="立即刷新"
        >
          <Svg d={ICON_REFRESH} size={14} />
          {loading ? "刷新中…" : "刷新"}
        </button>
      </div>
      {/* 收藏条 */}
      {favs.length > 0 && (
        <div className="hb-favbar">
          <span className="hb-favlabel">已收藏 {favs.length}</span>
          <div className="hb-favchips">
            {favs.slice(0, 30).map((f, i) => (
              <a
                key={favKey(f) + i}
                className="hb-chip"
                href={f.url || undefined}
                target="_blank"
                rel="noreferrer"
                title={`${f.source} · 点击打开`}
              >
                <span>{f.title}</span>
                <button
                  type="button"
                  className="hb-chipx"
                  aria-label="移除收藏"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    toggleFav(f);
                  }}
                >
                  ×
                </button>
              </a>
            ))}
          </div>
        </div>
      )}
      {/* 榜单主体 */}
      <div className="hb-list-wrap">
        {loading && !list.length ? (
          <div>
            {Array.from({ length: 10 }).map((_, i) => (
              <div key={i} className="hb-sk" />
            ))}
          </div>
        ) : err[active] && !list.length ? (
          <div className="hb-state err">
            <span className="icon-slot">
              <Svg d={ICON_ALERT} size={22} />
            </span>
            {activeSrc.label}加载失败
            <span className="msg">{err[active]}</span>
            <button type="button" className="hb-btn" onClick={() => load(activeSrc)}>
              重试
            </button>
          </div>
        ) : list.length === 0 ? (
          <div className="hb-state">
            <span className="icon-slot">
              <Svg d={ICON_INBOX} size={22} />
            </span>
            本榜单暂无数据
            <span className="msg">上游可能调整了接口结构，或网络受限</span>
          </div>
        ) : (
          <div className="hb-body">
            {/* TOP 3：独立卡片区，固定在顶部不参与滚动 */}
            <ul className="hb-top3">
              {list.slice(0, 3).map((it) => (
                <TopCard
                  key={favKey(it) || it.title}
                  item={it}
                  ratio={heatRatio(it)}
                  faved={favedKeys.has(favKey(it))}
                  onToggleFav={() => toggleFav(it)}
                />
              ))}
            </ul>
            <div className="hb-scroll">
              <ul className="hb-list">
              {list.slice(3).map((it) => {
                const faved = favedKeys.has(favKey(it));
                const tagCls = it.tag === "新" ? " ok" : it.tag === "沸" ? " boil" : "";
                return (
                  <li key={favKey(it) || it.title} className="hb-row">
                    <span className="hb-rank">{it.rank}</span>
                    {it.url ? (
                      <a className="hb-item-title" href={it.url} target="_blank" rel="noreferrer" title={it.title}>
                        {it.title}
                      </a>
                    ) : (
                      <span className="hb-item-title" style={{ cursor: "default" }}>
                        {it.title}
                      </span>
                    )}
                    {it.tag && <span className={`hb-tag${tagCls}`}>{it.tag}</span>}
                    {it.hot > 0 && <span className="hb-hot">{fmtHot(it.hot)}</span>}
                    <span className="hb-bar" aria-hidden="true">
                      <i style={{ width: `${heatRatio(it)}%` }} />
                    </span>
                    <span className="hb-act">
                      {it.url && (
                        <a className="hb-iconbtn" href={it.url} target="_blank" rel="noreferrer" title="打开原链接">
                          <Svg d={ICON_EXT} size={15} />
                        </a>
                      )}
                      <button
                        type="button"
                        className={`hb-iconbtn${faved ? " faved" : ""}`}
                        title={faved ? "取消收藏" : "收藏热点"}
                        onClick={() => toggleFav(it)}
                      >
                        <Svg d={ICON_BOOKMARK} size={15} filled={faved} />
                      </button>
                    </span>
                  </li>
                );
              })}
              </ul>
            </div>
          </div>
        )}
      </div>
      {/* 脚注 */}
      <div className="hb-foot">
        <Svg d={FLAME} size={12} />
        公开接口经统一外联服务直达 · 采集于 {fmtClock(tick)}
      </div>
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 12,
      viewId: "hotboard",
      title: "热榜聚合",
      // lucide flame
      icon: FLAME,
    }),
    ctx.slots.inject("surface.main", {
      viewId: "hotboard",
      title: "热榜聚合",
      component: async () => ({ default: () => <HotboardView ctx={ctx} /> }),
    }),
  ];

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
