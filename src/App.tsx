// AppHost 壳：三栏布局（对齐原型 ava/prototypes/app-host-proto/Shell.tsx）。
// 结构：标题栏 36px ｜ 侧边栏（264–420 拖拽调宽，可折叠 56px）｜ 主区（tab 化 + 舞台）
// ｜ 画布第二舞台（canvas:home 启动器 + 独立 tab 编排，折叠仅收宽度、视图状态驻留）。
// 壳自身不含业务视图——主区视图来自 App 的 surface.main 贡献，画布视图来自 surface.canvas。
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { CSSProperties } from "react";
import { AppList, AppSettingsModal, CanvasPanels, CanvasSurface, MainSurface, SidebarPanels, UserBar } from "./host/AppHost";
import { AppIconView, ICON_XL } from "./host/app-icon";
import { getBootInfo, discoverAndBoot } from "./host/loader";
import { registry } from "./host/registry";
import { useUpdateNotice } from "./host/updater";
import type { UserMenuAction } from "./host/types";
import "./App.css";

function useRegistryVersion(): number {
  return useSyncExternalStore(registry.subscribe, registry.getVersion);
}

const THEME_KEY = "ava2.theme";
const SIDEBAR_WIDTH_KEY = "ava2.sidebar.width";
const SIDEBAR_COLLAPSED_KEY = "ava2.sidebar.collapsed";
const CANVAS_WIDTH_KEY = "ava2.canvas.width";
const CANVAS_PANELS_KEY = "ava2.canvas.panels";
type Theme = "light" | "dark";

/** 侧栏宽度区间：默认 280，min 264 / max 420 */
const SIDEBAR_DEFAULT = 280;
const SIDEBAR_MIN = 264;
const SIDEBAR_MAX = 420;
const SIDEBAR_COLLAPSED = 56;

/** 画布宽度区间（拖拽调宽；默认 420，min 360 / max 760） */
const CANVAS_DEFAULT = 420;
const CANVAS_MIN = 360;
const CANVAS_MAX = 760;

/** 浮层尺寸区间（右下角手柄缩放；默认 640×560，min 320×240） */
const OVERLAY_DEFAULT_W = 640;
const OVERLAY_DEFAULT_H = 560;
const OVERLAY_MIN_W = 320;
const OVERLAY_MIN_H = 240;

/** 单个悬浮窗状态：viewId 唯一键 + 独立位置/尺寸（不同 App、同 App 不同视图均互不影响） */
type OverlayWin = {
  viewId: string;
  pos: { x: number; y: number };
  size: { w: number; h: number };
};

/** 新悬浮窗默认尺寸（视口内兜底夹取） */
function overlayDefaultSize(): OverlayWin["size"] {
  return {
    w: Math.min(OVERLAY_DEFAULT_W, window.innerWidth - 40),
    h: Math.min(OVERLAY_DEFAULT_H, window.innerHeight - 108),
  };
}

/** 壳自有导航图标（24×24 viewBox 的 lucide 同源 path，壳统一 18px 描边渲染） */
const NAV_ICON = {
  apps: "M3 3h7v7H3z M14 3h7v7h-7z M14 14h7v7h-7z M3 14h7v7H3z",
  panelLeft: "M3 3h18v18H3z M9 3v18",
  panelRight: "M3 3h18v18H3z M15 3v18",
  folderTree: "M3 7h4v10H3z M7 7h4v4H7z M7 13h4v4H7z M11 9h10v2H11z M11 15h10v2H11z",
  pip: "M21 9V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h4 M13 12h8a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1z",
  restore: "M4 14h6v6 M20 10h-6V4 M14 10l7-7 M3 21l7-7",
} as const;

/** 壳自有视图 id（App 管理） */
const APPS_VIEW = "__apps";

/** 画布启动器（canvas:home，壳自有视图）：桌面启动器形态——大图标网格，点击打开画布视图 */
const CANVAS_HOME = "canvas:home";

function CanvasLauncher({ onOpen }: { onOpen: (viewId: string) => void }) {
  useRegistryVersion();
  const surfaces = registry.canvasSurfaces();
  return (
    <div className="canvas-launcher">
      <h5 className="launcher-head">画布</h5>
      {surfaces.length === 0 ? (
        <div className="empty">暂无可打开的画布视图（App 经 surface.canvas 认领后在此出现）</div>
      ) : (
        <div className="launch-grid">
          {surfaces.map((s) => (
            <button key={`${s.appId}:${s.viewId}`} className="launch-tile" onClick={() => onOpen(s.viewId)} title={s.title}>
              <span className="tile-icon"><AppIconView icon={s.icon} size={ICON_XL} /></span>
              <span className="tile-name">{s.title}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export default function App() {
  useRegistryVersion();
  const update = useUpdateNotice();
  const [ready, setReady] = useState(false);
  const [viewId, setViewId] = useState<string>();
  const [openViews, setOpenViews] = useState<string[]>([]);
  const [theme, setTheme] = useState<Theme>(() =>
    localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light",
  );
  // 侧栏：宽度（拖拽调宽）+ 折叠（折叠态 56px 图标-only），状态持久化
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const raw = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY));
    return raw >= SIDEBAR_MIN && raw <= SIDEBAR_MAX ? raw : SIDEBAR_DEFAULT;
  });
  const [sidebarCollapsed, setSidebarCollapsed] = useState(
    () => localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1",
  );
  // 画布第二舞台：独立 tab 编排（canvas:home 启动器常驻；关闭不卸载、禁用隐去、失效兜底回启动器）
  const [canvasViews, setCanvasViews] = useState<string[]>([CANVAS_HOME]);
  const [canvasViewId, setCanvasViewId] = useState(CANVAS_HOME);
  const [canvasOpen, setCanvasOpen] = useState(false);
  // 画布宽度（左缘拖拽调宽），状态持久化
  const [canvasWidth, setCanvasWidth] = useState(() => {
    const raw = Number(localStorage.getItem(CANVAS_WIDTH_KEY));
    return raw >= CANVAS_MIN && raw <= CANVAS_MAX ? raw : CANVAS_DEFAULT;
  });
  // 画布右面板列（文件树）独立折叠（收成窄条），状态持久化
  const [panelsCollapsed, setPanelsCollapsed] = useState(
    () => localStorage.getItem(CANVAS_PANELS_KEY) === "1",
  );
  // 浮层舞台（多悬浮窗）：每个 surface.main 视图可独立悬浮（复用 MainSurface，独立拖动/缩放）。
  // 数组顺序即 DOM 顺序 → 末尾的窗叠在最上；点击窗体（focusOverlay）置顶。
  const [overlays, setOverlays] = useState<OverlayWin[]>([]);
  // tab 列表容器：多 tab 溢出时横向滚动（滚轮纵向手势映射为横向）
  const tabsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let alive = true;
    void discoverAndBoot().finally(() => {
      if (alive) setReady(true);
    });
    return () => {
      alive = false;
    };
  }, []);

  // 主题由壳持有（App 只发意图），落 documentElement 属性 + localStorage
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(THEME_KEY, theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, sidebarCollapsed ? "1" : "0");
  }, [sidebarCollapsed]);

  useEffect(() => {
    localStorage.setItem(CANVAS_WIDTH_KEY, String(canvasWidth));
  }, [canvasWidth]);

  useEffect(() => {
    localStorage.setItem(CANVAS_PANELS_KEY, panelsCollapsed ? "1" : "0");
  }, [panelsCollapsed]);

  // D3：App/Agent 经总线 canvas.open 事件打开 canvas tab（折叠中被打开 → 自动展开）
  useEffect(() => {
    const off = registry.bus.on("canvas.open", (e) => {
      const target = (e as { target?: string } | null)?.target;
      if (target && registry.canvasSurfaceFor(target)) {
        setCanvasViews((cur) => (cur.includes(target) ? cur : [...cur, target]));
        setCanvasViewId(target);
        setCanvasOpen(true);
      }
    });
    return () => off.dispose();
  }, []);

  // 侧栏拖拽调宽（264–420；折叠态不响应）
  const startResize = (e: React.MouseEvent) => {
    if (sidebarCollapsed) return;
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidth;
    const move = (ev: MouseEvent) => {
      setSidebarWidth(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, startW + (ev.clientX - startX))));
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("resizing");
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    document.body.classList.add("resizing");
  };

  // 画布左缘拖拽调宽（拖拽中 canvas 宽度跟随；与侧栏拖拽同套路）
  const startCanvasResize = (e: React.MouseEvent) => {
    if (!canvasOpen) return;
    e.preventDefault();
    const startX = e.clientX;
    const startW = canvasWidth;
    const move = (ev: MouseEvent) => {
      // 手柄在画布左缘：向左拖（clientX 变小）→ 画布变宽
      setCanvasWidth(
        Math.min(CANVAS_MAX, Math.max(CANVAS_MIN, startW - (ev.clientX - startX))),
      );
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("resizing");
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    document.body.classList.add("resizing");
  };

  const nav = registry.navItems();
  const navIds = nav.map((n) => n.viewId);
  const active = viewId && (navIds.includes(viewId) || viewId === APPS_VIEW) ? viewId : navIds[0];
  const boot = getBootInfo();

  const openView = (id: string) => {
    setOpenViews((cur) => (cur.includes(id) ? cur : [...cur, id]));
    setViewId(id);
  };
  const closeView = (id: string) => {
    setOpenViews((cur) => cur.filter((v) => v !== id));
    if (active === id) setViewId(undefined);
    setOverlays((cur) => cur.filter((o) => o.viewId !== id));
  };
  const tabTitle = (v: string): string =>
    v === APPS_VIEW ? "App 管理" : (registry.surfaceFor(v)?.title ?? v);
  // tab 渲染期过滤：App 被禁用/卸载时 tab 自动隐去（渲染期过滤而非 effect 剪枝）
  const tabs = openViews.filter((v) => v === APPS_VIEW || registry.surfaceFor(v));
  const shown = active && !tabs.includes(active) ? [active, ...tabs] : tabs;

  // canvas tab 渲染期过滤：仅保留启动器与注册表可解析的视图；失效兜底回启动器
  const canvasTabs = canvasViews.filter((v) => v === CANVAS_HOME || registry.canvasSurfaceFor(v));
  const canvasActive = canvasTabs.includes(canvasViewId) ? canvasViewId : CANVAS_HOME;
  const canvasTitle = (v: string): string =>
    v === CANVAS_HOME ? "画布" : (registry.canvasSurfaceFor(v)?.title ?? v);
  const openCanvasView = (id: string) => {
    setCanvasViews((cur) => (cur.includes(id) ? cur : [...cur, id]));
    setCanvasViewId(id);
    setCanvasOpen(true);
  };
  const closeCanvasView = (id: string) => {
    setCanvasViews((cur) => cur.filter((v) => v !== id));
    setCanvasViewId((cur) => (cur === id ? CANVAS_HOME : cur));
  };

  // ── user 栏菜单 ──
  // 设置弹层单点持有（AppList 详情与 UserBar 菜单两个入口共用）
  const [settingsTarget, setSettingsTarget] = useState<{ appId: string; name: string }>();

  /** 执行 App 菜单项动作（user.menu）：路由与事件发射由壳持有，App 只传纯数据动作 */
  const runMenuAction = (appId: string, action: UserMenuAction | undefined) => {
    if (!action) return;
    if (action.kind === "openView") {
      // 与 AppList「打开」同路由：画布视图走画布舞台，否则主区 tab
      if (registry.canvasSurfaceFor(action.viewId)) openCanvasView(action.viewId);
      else openView(action.viewId);
    } else if (action.kind === "openSettings") {
      const rec = registry.getRecord(appId);
      if (registry.settingsPanelFor(appId)) setSettingsTarget({ appId, name: rec?.manifest.name ?? appId });
    } else {
      // event 动作：宿主代为广播，App 自行 on 监听处理
      registry.bus.emit(action.type, action.payload);
    }
  };

  // 浮层舞台标题栏拖动 / 右下角手柄缩放：Pointer Events + setPointerCapture——
  // 捕获后 move/up 事件持续派发到起始元素（光标快速移动、移出窗口也不丢事件），比 window 监听更稳。
  // 多窗：以 win.viewId 定位目标条目，函数式更新避免闭包旧状态。
  const startOverlayDrag = (e: React.PointerEvent, win: OverlayWin) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startY = e.clientY;
    const { x, y } = win.pos;
    const move = (ev: PointerEvent) => {
      const nx = Math.min(window.innerWidth - 140, Math.max(8, x + ev.clientX - startX));
      const ny = Math.min(window.innerHeight - 60, Math.max(8, y + ev.clientY - startY));
      setOverlays((cur) =>
        cur.map((o) => (o.viewId === win.viewId ? { ...o, pos: { x: nx, y: ny } } : o)),
      );
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      document.body.classList.remove("resizing");
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    document.body.classList.add("resizing");
  };

  // 浮层缩放（右下角手柄）：宽度/高度夹取到视口内（受该窗当前位置约束）
  const startOverlayResize = (e: React.PointerEvent, win: OverlayWin) => {
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startY = e.clientY;
    const { w, h } = win.size;
    const move = (ev: PointerEvent) => {
      const nw = Math.min(window.innerWidth - win.pos.x - 8, Math.max(OVERLAY_MIN_W, w + ev.clientX - startX));
      const nh = Math.min(window.innerHeight - win.pos.y - 8, Math.max(OVERLAY_MIN_H, h + ev.clientY - startY));
      setOverlays((cur) =>
        cur.map((o) => (o.viewId === win.viewId ? { ...o, size: { w: nw, h: nh } } : o)),
      );
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      document.body.classList.remove("resizing");
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    document.body.classList.add("resizing");
  };

  // 悬浮 / 还原（tab 按钮）：存在即移除，否则追加新窗（位置按窗数级联错开，避免完全重叠）
  const toggleOverlay = (viewId: string) => {
    setOverlays((cur) => {
      if (cur.some((o) => o.viewId === viewId)) {
        return cur.filter((o) => o.viewId !== viewId);
      }
      const n = cur.length;
      return [
        ...cur,
        {
          viewId,
          pos: { x: Math.max(8, Math.max(24, window.innerWidth - OVERLAY_DEFAULT_W) + (n % 5) * 28), y: 92 + (n % 5) * 28 },
          size: overlayDefaultSize(),
        },
      ];
    });
  };

  // 点击窗体置顶：把该窗移到数组末尾（DOM 末尾者叠最上）
  const focusOverlay = (viewId: string) => {
    setOverlays((cur) => {
      const target = cur.find((o) => o.viewId === viewId);
      return target ? [...cur.filter((o) => o.viewId !== viewId), target] : cur;
    });
  };

  return (
    <div className="app-root">
      {/* 桌面壳发现新版本时由主进程推送事件，这里给一条可操作的横幅（浏览器模式不出现） */}
      {update.info && (
        <div className="update-banner">
          <span className="update-banner-text">
            发现新版本 v{update.info.version}（当前 v{update.info.currentVersion}）
          </span>
          {update.error && <span className="update-banner-error">{update.error}</span>}
          <button className="update-banner-btn" onClick={() => void update.install()} disabled={update.installing}>
            {update.installing ? "更新中…" : "立即更新"}
          </button>
          <button className="update-banner-close" onClick={update.dismiss} title="稍后再说" aria-label="稍后再说">
            ✕
          </button>
        </div>
      )}
      {/* 顶部合并条（40px）：身份区（徽标/名称/状态点/折叠钮）+ 0.5px 竖分隔
          + 视图 tab 列表 + 画布开关。身份区宽度跟随侧栏，分隔线落在侧栏右缘。 */}
      <header
        className={"appbar" + (sidebarCollapsed ? " collapsed" : "")}
        style={{ "--ava-sidebar-w": `${sidebarCollapsed ? SIDEBAR_COLLAPSED : sidebarWidth}px` } as CSSProperties}
      >
        <div className="appbar-lead">
          <div className="appbar-id">
            <span className="appbar-badge">A</span>
            <span className="appbar-name">Ava</span>
            <span className={"status-dot " + (boot?.backendReady ? "ok" : boot ? "bad" : "idle")} />
          </div>
          <button
            className="icon-btn"
            onClick={() => setSidebarCollapsed((c) => !c)}
            title={sidebarCollapsed ? "展开侧边栏" : "折叠侧边栏"}
          >
            <AppIconView icon={NAV_ICON.panelLeft} />
          </button>
          <span className="appbar-sep" />
        </div>
        <div
          className="appbar-tabs"
          role="tablist"
          aria-label="打开的视图"
          ref={tabsRef}
          onWheel={(e) => {
            // tab 超出时把滚轮纵向手势映射为横向滚动（滚动条已隐藏，鼠标用户也能翻 tab）
            const el = tabsRef.current;
            if (!el || el.scrollWidth <= el.clientWidth) return;
            if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
            el.scrollLeft += e.deltaY;
          }}
        >
          {shown.map((v) => {
            const floating = overlays.some((o) => o.viewId === v);
            return (
              <div
                key={v}
                role="tab"
                aria-selected={active === v}
                className={"tab" + (active === v ? " on" : "")}
                onClick={(e) => {
                  setViewId(v);
                  e.currentTarget.scrollIntoView({ block: "nearest", inline: "nearest" });
                }}
              >
                <span className="tab-title">{tabTitle(v)}</span>
                {v !== APPS_VIEW && (
                  <button
                    className={"tab-float" + (floating ? " on" : "")}
                    title={floating ? "已悬浮（点击还原）" : "悬浮打开"}
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleOverlay(v);
                    }}
                  >
                    <AppIconView icon={NAV_ICON.pip} size={12} />
                  </button>
                )}
                <button
                  className="tab-close"
                  title="关闭"
                  onClick={(e) => {
                    e.stopPropagation();
                    closeView(v);
                  }}
                >
                  ×
                </button>
              </div>
            );
          })}
          {shown.length === 0 && <span className="tabbar-empty">暂无打开的视图</span>}
        </div>
        <span className="appbar-spacer" />
        <button
          className={"icon-btn" + (canvasOpen ? " on" : "")}
          title={canvasOpen ? "收起画布" : "展开画布"}
          onClick={() => setCanvasOpen((v) => !v)}
        >
          <AppIconView icon={NAV_ICON.panelRight} />
        </button>
      </header>

      <div className="shell">
        <aside
          className={"sidebar" + (sidebarCollapsed ? " collapsed" : "")}
          style={{ width: sidebarCollapsed ? SIDEBAR_COLLAPSED : sidebarWidth }}
        >
          <nav className="side-nav">
            {nav.map((item) => (
              <button
                key={`${item.appId}:${item.viewId}`}
                className={"nav-link" + (active === item.viewId ? " on" : "")}
                onClick={() => openView(item.viewId)}
                title={item.title}
              >
                <span className="nav-ico">
                  {item.icon ? <AppIconView icon={item.icon} /> : <span className="nav-dot" />}
                </span>
                <span className="nav-label">{item.title}</span>
              </button>
            ))}
            <button
              className={"nav-link" + (active === APPS_VIEW ? " on" : "")}
              onClick={() => openView(APPS_VIEW)}
              title="App 管理"
            >
              <span className="nav-ico">
                <AppIconView icon={NAV_ICON.apps} />
              </span>
              <span className="nav-label">App 管理</span>
            </button>
            {nav.length === 0 && <div className="empty-inline">暂无导航项</div>}
          </nav>
          {!sidebarCollapsed && (
            <div className="side-panels">{active ? <SidebarPanels viewId={active} /> : null}</div>
          )}
          {/* user 栏（宿主自有身份区）：点击弹出上浮菜单，App 经 user.menu slot 贡献菜单项。
              折叠态仅显示头像（CSS 随 .sidebar.collapsed 收敛）；主题切换也收在菜单中。 */}
          <UserBar
            onOpenApps={() => openView(APPS_VIEW)}
            theme={theme}
            onToggleTheme={() => setTheme((t) => (t === "light" ? "dark" : "light"))}
            onAction={runMenuAction}
          />
          {!sidebarCollapsed && (
            <div className="sidebar-resize" onMouseDown={startResize} title="拖拽调整宽度（264–420）" />
          )}
        </aside>

        <main className="main-area">
          <section className="stage">
            {!ready ? (
              <div className="empty">宿主启动中…</div>
            ) : active === APPS_VIEW ? (
              <div className="pane">
                <AppList
                  onOpen={(viewId) => {
                    if (registry.canvasSurfaceFor(viewId)) openCanvasView(viewId);
                    else openView(viewId);
                  }}
                  onOpenSettings={(appId, name) => setSettingsTarget({ appId, name })}
                />
              </div>
            ) : active && overlays.some((o) => o.viewId === active) ? (
              <div className="empty">该视图已悬浮于主界面之上（在悬浮窗内操作；点 tab 上的悬浮按钮可还原）</div>
            ) : active ? (
              <MainSurface viewId={active} />
            ) : (
              <div className="empty">暂无可用视图（请在 App 管理中安装并启用 App）</div>
            )}
          </section>
        </main>

        {/* 画布第二舞台：折叠仅收宽度（视图不卸载），展开即恢复；左缘手柄可拖拽调宽 */}
        <aside
          className={"canvas-side" + (canvasOpen ? " open" : "")}
          style={{ width: canvasOpen ? canvasWidth : 0 }}
          aria-hidden={!canvasOpen}
        >
          {canvasOpen && (
            <>
              <div className="canvas-resizer" onMouseDown={startCanvasResize} title="拖拽调整画布宽度" />
              <header className="canvas-head">
                <div className="canvas-tabbar" role="tablist" aria-label="画布视图">
                  {canvasTabs.map((v) => (
                    <div
                      key={v}
                      role="tab"
                      aria-selected={canvasActive === v}
                      className={"tab" + (canvasActive === v ? " on" : "")}
                      onClick={() => setCanvasViewId(v)}
                    >
                      <span className="tab-title">{canvasTitle(v)}</span>
                      {v !== CANVAS_HOME && (
                        <button
                          className="tab-close"
                          title="关闭"
                          onClick={(e) => {
                            e.stopPropagation();
                            closeCanvasView(v);
                          }}
                        >
                          ×
                        </button>
                      )}
                    </div>
                  ))}
                </div>
                <button
                  className={"icon-btn canvas-tree-toggle" + (!panelsCollapsed ? " on" : "")}
                  title={panelsCollapsed ? "展开文件树" : "折叠文件树"}
                  aria-expanded={!panelsCollapsed}
                  onClick={() => setPanelsCollapsed((v) => !v)}
                >
                  <AppIconView icon={NAV_ICON.folderTree} />
                </button>
              </header>
              <div className="canvas-stage">
                <div className="canvas-surface">
                  {canvasActive === CANVAS_HOME ? (
                    <CanvasLauncher onOpen={openCanvasView} />
                  ) : (
                    <CanvasSurface viewId={canvasActive} />
                  )}
                </div>
                {!panelsCollapsed && (
                  <div className="canvas-panels">
                    <CanvasPanels viewId={canvasActive} />
                  </div>
                )}
              </div>
            </>
          )}
        </aside>
      </div>

      {/* 浮层舞台（多悬浮窗）：App 主舞台悬浮于主界面之上——overlay 层直接挂 MainSurface（与 tab 舞台同构）。
          每个视图独立一窗（跨 App 互不影响）；窗框（拖动/缩放/还原/关闭）由壳承担；无遮罩，主界面仍可操作；
          数组顺序即层叠顺序，点击窗体置顶；层级高于舞台内容、低于宿主模态。 */}
      {overlays.map((win) => (
        <section
          key={win.viewId}
          className="stage-overlay"
          style={{ left: win.pos.x, top: win.pos.y, width: win.size.w, height: win.size.h }}
          role="dialog"
          aria-label={tabTitle(win.viewId)}
          onPointerDown={() => focusOverlay(win.viewId)}
        >
          <header className="stage-overlay-head" onPointerDown={(e) => startOverlayDrag(e, win)}>
            <AppIconView icon={NAV_ICON.pip} />
            <span className="stage-overlay-title">{tabTitle(win.viewId)}</span>
            <button className="tab-close" title="还原到标签页" onClick={() => toggleOverlay(win.viewId)}>
              <AppIconView icon={NAV_ICON.restore} size={12} />
            </button>
            <button className="tab-close" title="关闭视图" onClick={() => closeView(win.viewId)}>
              ×
            </button>
          </header>
          <div className="stage-overlay-body">
            <MainSurface viewId={win.viewId} />
          </div>
          <div
            className="stage-overlay-resize"
            title="拖拽缩放"
            onPointerDown={(e) => startOverlayResize(e, win)}
          />
        </section>
      ))}

      {/* App 设置弹层（壳级单点持有）：AppList 详情与 UserBar 菜单两个入口共用 */}
      {settingsTarget && (
        <AppSettingsModal
          appId={settingsTarget.appId}
          name={settingsTarget.name}
          onClose={() => setSettingsTarget(undefined)}
        />
      )}
    </div>
  );
}
