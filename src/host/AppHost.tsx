// React 绑定：useSyncExternalStore 订阅注册表版本 + 惰性 surface + 错误边界 + App 管理面板。
// App 组件一律经 React.lazy 惰性 import（工厂在 setup 时登记），宿主不静态引用任何 App 代码。
import { Component, Suspense, lazy, useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { registry } from "./registry";
import { AppIconView, ICON_LG, ICON_SM } from "./app-icon";
import { getLazy, lazyId, setLazy, clearLazyCache } from "./lazy-cache";
import { installFromDir, installFromHub, listHub, setEnabled as setEnabledApi, uninstall as uninstallApi, type HubApp } from "./apps";
import { loadInstalled } from "./loader";
import type { AppRecord, UserMenuAction, UserMenuItemSpec } from "./types";

function useRegistryVersion(): number {
  return useSyncExternalStore(registry.subscribe, registry.getVersion);
}

/** 单 App 渲染隔离：一个 App 崩溃不影响其他 App（主区/画布同构） */
export class ErrorBoundary extends Component<{ children: ReactNode; appId: string }, { error?: Error }> {
  state: { error?: Error } = {};
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (this.state.error) {
      return (
        <div className="fail">
          App「{this.props.appId}」渲染出错，已隔离：{this.state.error.message}
        </div>
      );
    }
    return this.props.children;
  }
}

/** 主舞台：按 viewId 取 surface.main 贡献并惰性渲染。
 *  lazy 组件按 appId::viewId 键控（lazy-cache）：工厂每次渲染重建会反复 import 并重置组件状态。 */
export function MainSurface({ viewId }: { viewId: string }) {
  useRegistryVersion();
  const spec = registry.surfaceFor(viewId);
  if (!spec) return <div className="empty">该视图暂无内容</div>;
  let Comp = getLazy(spec.appId, viewId);
  if (!Comp) {
    Comp = lazy(spec.component);
    setLazy(spec.appId, viewId, Comp);
  }
  // ErrorBoundary 以 lazy 实例 id 为 key：热重载换新模块 → 边界 remount、error 重置
  return (
    <ErrorBoundary key={`${spec.appId}:${viewId}:${lazyId(Comp)}`} appId={spec.appId}>
      <Suspense fallback={<div className="empty">加载中…</div>}>
        <Comp />
      </Suspense>
    </ErrorBoundary>
  );
}

/** 画布舞台视图：按 viewId 取 surface.canvas 贡献并惰性渲染（机制同主区 MainSurface）。
 *  lazy 键 `canvas:<viewId>` 与主区隔离：同一 App 可同时持有主区与画布两份组件状态。 */
export function CanvasSurface({ viewId }: { viewId: string }) {
  useRegistryVersion();
  const spec = registry.canvasSurfaceFor(viewId);
  if (!spec) return <div className="empty">该视图暂无内容</div>;
  let Comp = getLazy(spec.appId, `canvas:${viewId}`);
  if (!Comp) {
    Comp = lazy(spec.component);
    setLazy(spec.appId, `canvas:${viewId}`, Comp);
  }
  return (
    <ErrorBoundary key={`${spec.appId}:${viewId}:${lazyId(Comp)}`} appId={spec.appId}>
      <Suspense fallback={<div className="empty">加载中…</div>}>
        <Comp />
      </Suspense>
    </ErrorBoundary>
  );
}

/** 侧边栏面板：当前 viewId 的 panel.sidebar 贡献 */
export function SidebarPanels({ viewId }: { viewId: string }) {
  useRegistryVersion();
  const panels = registry.sidebarPanels(viewId);
  return (
    <>
      {panels.map((p) => {
        const cacheKey = `${p.viewId}:sidebar`;
        let Comp = getLazy(p.appId, cacheKey);
        if (!Comp) {
          Comp = lazy(p.component);
          setLazy(p.appId, cacheKey, Comp);
        }
        return (
          <ErrorBoundary key={`${p.appId}:${cacheKey}:${lazyId(Comp)}`} appId={p.appId}>
            <Suspense fallback={<div className="empty">加载中…</div>}>
              <Comp />
            </Suspense>
          </ErrorBoundary>
        );
      })}
    </>
  );
}

/** 画布面板：当前画布视图 viewId 的 panel.canvas 贡献（键 `canvaspanel:<viewId>`，与主区/画布隔离） */
export function CanvasPanels({ viewId }: { viewId: string }) {
  useRegistryVersion();
  const panels = registry.canvasPanels(viewId);
  return (
    <>
      {panels.map((p) => {
        const cacheKey = `canvaspanel:${p.viewId}`;
        let Comp = getLazy(p.appId, cacheKey);
        if (!Comp) {
          Comp = lazy(p.component);
          setLazy(p.appId, cacheKey, Comp);
        }
        return (
          <ErrorBoundary key={`${p.appId}:${cacheKey}:${lazyId(Comp)}`} appId={p.appId}>
            <Suspense fallback={<div className="empty">加载中…</div>}>
              <Comp />
            </Suspense>
          </ErrorBoundary>
        );
      })}
    </>
  );
}

/** App 设置面板弹层：挂载 App 在 setup 期经 ctx.registerSettingsPanel 登记的组件。
 *  宿主只提供入口 + 惰性挂载 + 错误隔离；表单、存储、校验全部由 App 自理。
 *  弹层状态由壳单点持有（AppList 详情与 UserBar 菜单两个入口共用，不并存两份弹层）。 */
export function AppSettingsModal({ appId, name, onClose }: { appId: string; name: string; onClose: () => void }) {
  useRegistryVersion();
  const factory = registry.settingsPanelFor(appId);
  let Comp = factory ? getLazy(appId, "settings") : undefined;
  if (factory && !Comp) {
    Comp = lazy(factory);
    setLazy(appId, "settings", Comp);
  }
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="app-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-body">
          <div className="drawer-name">「{name}」设置</div>
          {Comp ? (
            <ErrorBoundary key={`${appId}:settings:${lazyId(Comp)}`} appId={appId}>
              <Suspense fallback={<div className="empty">加载中…</div>}>
                <Comp />
              </Suspense>
            </ErrorBoundary>
          ) : (
            <div className="empty">该 App 未提供设置界面</div>
          )}
          <div className="drawer-divider" />
          <div className="drawer-actions">
            <button className="btn-ghost" onClick={onClose}>
              关闭
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

/** user 栏回调：宿主快捷项 + App 菜单项动作执行器（路由与事件发射由壳持有） */
export interface UserBarProps {
  /** 宿主快捷项：打开 App 管理 */
  onOpenApps: () => void;
  /** 当前主题（驱动菜单项文案与图标） */
  theme: "light" | "dark";
  /** 切换主题（主题 state 由壳持有，App 只发意图） */
  onToggleTheme: () => void;
  /** 执行 App 菜单项动作（openView/openSettings/event 的解释执行） */
  onAction: (appId: string, action: UserMenuAction | undefined) => void;
}

/** 侧边栏底部 user 栏：宿主自有身份区（当前为本地占位用户，接账号体系后原位替换）
 *  + 点击向上弹出的上浮菜单。菜单 = 用户信息头 + 宿主快捷区 + App 经 user.menu
 *  slot 贡献的菜单项（按 App 分组；App 禁用/卸载自动随 slot 管道隐去）。
 *  弹层用 fixed 定位：sidebar overflow-y 会裁切 absolute 子元素。 */
export function UserBar({ onOpenApps, theme, onToggleTheme, onAction }: UserBarProps) {
  useRegistryVersion();
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const [pos, setPos] = useState({ left: 0, bottom: 0 });
  const items = registry.userMenuItems();

  // 相邻同 appId 归组（registry 已按 App 名 + 组内 order 排好序）
  const groups: Array<{ appId: string; name: string; items: UserMenuItemSpec[] }> = [];
  for (const it of items) {
    const last = groups[groups.length - 1];
    if (last && last.appId === it.appId) last.items.push(it);
    else
      groups.push({
        appId: it.appId,
        name: registry.getRecord(it.appId)?.manifest.name ?? it.appId,
        items: [it],
      });
  }

  const toggle = () => {
    if (!open && btnRef.current) {
      const r = btnRef.current.getBoundingClientRect();
      // 弹层锚在按钮左缘、向上展开（bottom = 视口高 - 按钮顶 + 间距）
      setPos({ left: r.left, bottom: window.innerHeight - r.top + 6 });
    }
    setOpen((v) => !v);
  };

  // Escape 关闭
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  // 外点关闭（按钮与弹层之外的 pointerdown）
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      const el = e.target as HTMLElement;
      if (!el.closest(".user-pop") && !el.closest(".user-bar")) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open]);

  const run = (appId: string, action: UserMenuAction | undefined) => {
    setOpen(false);
    onAction(appId, action);
  };

  return (
    <>
      <button
        ref={btnRef}
        className={"user-bar" + (open ? " on" : "")}
        onClick={toggle}
        title="用户菜单"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <span className="user-bar-avatar">本</span>
        <span className="user-bar-text">
          <span className="user-bar-name">本地用户</span>
          <span className="user-bar-sub">Ava · 本地模式</span>
        </span>
        <AppIconView icon="M18 15l-6-6-6 6" size={14} />
      </button>
      {open && (
        <div className="user-pop" role="menu" style={pos}>
          <div className="user-pop-head">
            <span className="user-pop-avatar">本</span>
            <div className="user-pop-meta">
              <div className="user-pop-name">本地用户</div>
              <div className="user-pop-sub">未登录 · 数据存于本机</div>
            </div>
          </div>
          <div className="user-pop-divider" />
          <button
            className="user-pop-item"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onOpenApps();
            }}
          >
            <span className="user-pop-ico">
              <AppIconView icon="M3 3h7v7H3z M14 3h7v7h-7z M14 14h7v7h-7z M3 14h7v7H3z" />
            </span>
            App 管理
          </button>
          <button
            className="user-pop-item"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onToggleTheme();
            }}
            title={theme === "light" ? "切换深色" : "切换浅色"}
          >
            <span className="user-pop-ico">
              <AppIconView
                icon={
                  theme === "light"
                    ? "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z"
                    : "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M12 2v2 M12 20v2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M2 12h2 M20 12h2 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4"
                }
              />
            </span>
            {theme === "light" ? "切换深色" : "切换浅色"}
          </button>
          {groups.length > 0 && (
            <>
              <div className="user-pop-divider" />
              {groups.map((g) => (
                <div key={g.appId} className="user-pop-group">
                  <div className="user-pop-group-title">{g.name}</div>
                  {g.items.map((it) => (
                    <button
                      key={it.id}
                      className="user-pop-item"
                      role="menuitem"
                      onClick={() => run(g.appId, it.action)}
                    >
                      <span className="user-pop-ico">
                        {it.icon ? <AppIconView icon={it.icon} /> : <span className="user-pop-dot" />}
                      </span>
                      {it.title}
                    </button>
                  ))}
                </div>
              ))}
            </>
          )}
        </div>
      )}
    </>
  );
}

/** App 管理面板（应用市场形态）：图标网格 + 底部详情抽屉。
 *  启停与卸载的权威在边车登记表（重启后保持）；App 配置由 App 自管（宿主零策略）。
 *  设置弹层状态由壳单点持有：详情「设置」按钮经 onOpenSettings 回调触发。 */
export function AppList({
  onOpen,
  onOpenSettings,
}: {
  onOpen: (viewId: string) => void;
  onOpenSettings: (appId: string, name: string) => void;
}) {
  useRegistryVersion();
  const [actionErr, setActionErr] = useState<string>();
  const [installPath, setInstallPath] = useState("");
  const [installOpen, setInstallOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string>();
  /** 视图：我的应用（已安装） / 应用中心（远程仓库 App Hub） */
  const [tab, setTab] = useState<"installed" | "hub">("installed");
  /** 应用中心：远程可安装列表（按需拉取）+ 来源仓库 + 加载/错误态 */
  const [hubApps, setHubApps] = useState<HubApp[]>([]);
  const [hubSource, setHubSource] = useState("");
  const [hubLoading, setHubLoading] = useState(false);
  const [hubErr, setHubErr] = useState<string>();
  /** 卸载确认（AlertDialog）：删除 App 目录不可逆，对话框确认而非一键直删 */
  const [confirmUninstall, setConfirmUninstall] = useState<AppRecord | null>(null);
  const apps = registry.listApps();
  const selected = apps.find((a) => a.manifest.id === selectedId);

  /** 拉取应用中心列表（关键字由服务端过滤） */
  const refreshHub = async (keyword = "") => {
    setHubErr(undefined);
    setHubLoading(true);
    try {
      const data = await listHub(keyword);
      setHubApps(data.apps);
      setHubSource(data.source);
    } catch (e) {
      setHubErr(e instanceof Error ? e.message : String(e));
    } finally {
      setHubLoading(false);
    }
  };

  // 应用中心：进入或关键字变化时拉取（300ms 防抖）
  useEffect(() => {
    if (tab !== "hub") return;
    const timer = setTimeout(() => void refreshHub(query), 300);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, query]);

  const setEnabled = async (a: AppRecord, enabled: boolean) => {
    setActionErr(undefined);
    setBusy(true);
    try {
      await setEnabledApi(a.manifest.id, enabled);
      if (enabled && !registry.isLoaded(a.manifest.id)) await registry.load(a.manifest);
      else registry.setStatus(a.manifest.id, enabled ? "active" : "suspended");
    } catch (e) {
      setActionErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const uninstall = async (a: AppRecord) => {
    setActionErr(undefined);
    setBusy(true);
    try {
      await uninstallApi(a.manifest.id);
      // App 配置由 App 自管（宿主零策略），卸载只移除目录与登记项
      clearLazyCache(a.manifest.id);
      await registry.unload(a.manifest.id);
      setSelectedId(undefined);
    } catch (e) {
      setActionErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const install = async () => {
    setActionErr(undefined);
    setBusy(true);
    try {
      const installed = await installFromDir(installPath.trim());
      setInstallPath("");
      setInstallOpen(false);
      await loadInstalled(installed);
    } catch (e) {
      setActionErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  /** 从应用中心下载安装：成功后增量装配并刷新远程列表（已安装标记） */
  const installHubApp = async (app: HubApp) => {
    setActionErr(undefined);
    setBusy(true);
    try {
      const installed = await installFromHub(app.manifest.id);
      await loadInstalled(installed);
      await refreshHub(query);
    } catch (e) {
      setActionErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  /** 打开 App 主视图（抽屉收起） */
  const openApp = (a: AppRecord) => {
    const viewId = registry.appPrimaryView(a.manifest.id);
    if (viewId) onOpen(viewId);
    setSelectedId(undefined);
  };

  const shown = query
    ? apps.filter((a) =>
        (a.manifest.name + " " + a.manifest.id).toLowerCase().includes(query.toLowerCase()),
      )
    : apps;

  return (
    <section className="app-store">
      <div className="store-head">
        <h1>{tab === "installed" ? "我的应用" : "应用中心"}</h1>
        <div className="store-tabs">
          <button
            className={"store-tab" + (tab === "installed" ? " on" : "")}
            onClick={() => setTab("installed")}
          >
            已安装
          </button>
          <button
            className={"store-tab" + (tab === "hub" ? " on" : "")}
            onClick={() => setTab("hub")}
          >
            应用中心
          </button>
        </div>
        <div className="store-search">
          <AppIconView icon="M21 21l-4.3-4.3M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0z" size={ICON_SM} />
          <input
            placeholder={tab === "installed" ? "搜索应用…" : "搜索应用中心…"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        {tab === "installed" && (
          <button className="store-install-btn" onClick={() => setInstallOpen((v) => !v)} disabled={busy} title="安装 App（产物目录）">
            {installOpen ? "收起" : "安装"}
          </button>
        )}
        {tab === "hub" && (
          <button
            className="store-install-btn"
            onClick={() => void refreshHub(query)}
            disabled={hubLoading}
            title="刷新应用中心列表"
          >
            {hubLoading ? "刷新中…" : "刷新"}
          </button>
        )}
      </div>
      {installOpen && tab === "installed" && (
        <div className="store-install">
          <input
            placeholder="App 产物目录（绝对路径）"
            value={installPath}
            onChange={(e) => setInstallPath(e.target.value)}
            disabled={busy}
          />
          <button className="btn-solid" onClick={() => void install()} disabled={busy || installPath.trim() === ""}>
            安装
          </button>
        </div>
      )}
      {actionErr && <div className="fail">操作失败：{actionErr}</div>}

      {tab === "hub" ? (
        <>
          <div className="store-source">
            {hubSource && <>来源仓库：{hubSource}（apps/ 目录，仅可安装预构建产物）</>}
          </div>
          {hubErr && <div className="fail">加载应用中心失败：{hubErr}</div>}
          {hubLoading && hubApps.length === 0 && <div className="empty">正在获取应用中心列表…</div>}
          {!hubLoading && !hubErr && hubApps.length === 0 && (
            <div className="empty">应用中心暂无可用 App</div>
          )}
          {hubApps.length > 0 && (
            <div className="hub-grid">
              {hubApps.map((h) => (
                <div key={h.manifest.id} className="hub-card">
                  <span className="tile-icon">
                    <AppIconView icon={h.manifest.icon} size={ICON_LG} />
                  </span>
                  <div className="hub-meta">
                    <div className="hub-name">
                      {h.manifest.name}
                      <span className="hub-ver">v{h.manifest.version}</span>
                    </div>
                    {h.manifest.description && <div className="hub-desc">{h.manifest.description}</div>}
                    {h.manifest.author && <div className="hub-sub">开发者：{h.manifest.author}</div>}
                  </div>
                  <button
                    className={"btn-solid hub-btn" + (h.installed ? " installed" : "")}
                    disabled={busy || h.installed}
                    onClick={() => void installHubApp(h)}
                  >
                    {h.installed ? "已安装" : busy ? "安装中…" : "安装"}
                  </button>
                </div>
              ))}
            </div>
          )}
        </>
      ) : shown.length === 0 ? (
        <div className="empty">尚未安装任何 App（安装源需含 manifest.json 与预构建产物）</div>
      ) : (
        <div className="app-tile-grid">
          {shown.map((a) => (
            <button
              key={a.manifest.id}
              className={"app-tile" + (a.status === "suspended" ? " off" : "")}
              onClick={() => setSelectedId(a.manifest.id)}
              title={a.manifest.name}
            >
              <span className="tile-icon">
                <AppIconView icon={registry.appIconFor(a.manifest.id)} size={ICON_LG} />
                <span className={"tile-dot" + (a.status === "active" ? "" : " off")} />
              </span>
              <span className="tile-name">{a.manifest.name}</span>
            </button>
          ))}
        </div>
      )}

      {/* 详情弹窗（居中 modal：桌面就近操作，避免底部抽屉大范围移动鼠标） */}
      {selected && (
        <div className="modal-backdrop" onClick={() => setSelectedId(undefined)}>
          <div className="app-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
            <div className="drawer-body">
              <div className="drawer-app">
                <span className="drawer-icon">
                  <AppIconView icon={registry.appIconFor(selected.manifest.id)} size={ICON_LG} />
                </span>
                <div>
                  <div className="drawer-name">{selected.manifest.name}</div>
                  {selected.manifest.description && (
                    <div className="drawer-desc">{selected.manifest.description}</div>
                  )}
                  {(selected.manifest.author || selected.manifest.license) && (
                    <div className="drawer-desc">
                      {selected.manifest.author && <>开发者：{selected.manifest.author}</>}
                      {selected.manifest.author && selected.manifest.license && " · "}
                      {selected.manifest.license && <>协议：{selected.manifest.license}</>}
                    </div>
                  )}
                  {selected.manifest.homepage &&
                    (/^https?:\/\//.test(selected.manifest.homepage) ? (
                      <div className="drawer-desc">
                        主页：
                        <a href={selected.manifest.homepage} target="_blank" rel="noreferrer">
                          {selected.manifest.homepage}
                        </a>
                      </div>
                    ) : (
                      <div className="drawer-desc">主页：{selected.manifest.homepage}</div>
                    ))}
                </div>
              </div>
              {selected.error && <div className="fail">{selected.error}</div>}
              <div className="drawer-divider" />
              <div className="drawer-row">
                <span className="drawer-lbl">启用</span>
                <span className="spacer" />
                <button
                  className={"drawer-switch" + (selected.status === "active" ? " on" : "")}
                  role="switch"
                  aria-checked={selected.status === "active"}
                  onClick={() => void setEnabled(selected, selected.status !== "active")}
                  disabled={busy || selected.status === "loading"}
                  title={selected.status === "active" ? "禁用" : "启用"}
                />
              </div>
              <div className="drawer-actions">
                {registry.appPrimaryView(selected.manifest.id) && (
                  <button className="btn-solid" onClick={() => openApp(selected)} disabled={busy}>
                    打开
                  </button>
                )}
                {registry.settingsPanelFor(selected.manifest.id) && (
                  <button
                    className="btn-ghost"
                    onClick={() => onOpenSettings(selected.manifest.id, selected.manifest.name)}
                    disabled={busy}
                  >
                    设置
                  </button>
                )}
                <button className="btn-ghost" onClick={() => setSelectedId(undefined)}>
                  完成
                </button>
                <button className="btn-danger" onClick={() => setConfirmUninstall(selected)} disabled={busy}>
                  卸载
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {/* 卸载确认（AlertDialog：不可逆操作，叠于详情 modal 之上） */}
      {confirmUninstall && (
        <div className="modal-backdrop" onClick={() => setConfirmUninstall(null)}>
          <div className="confirm-modal" role="alertdialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
            <div className="confirm-title">卸载「{confirmUninstall.manifest.name}」？</div>
            <div className="confirm-desc">App 目录将从磁盘移除，此操作不可恢复。</div>
            <div className="confirm-actions">
              <button className="btn-ghost" onClick={() => setConfirmUninstall(null)}>
                取消
              </button>
              <button
                className="btn-danger"
                onClick={() => {
                  const target = confirmUninstall;
                  setConfirmUninstall(null);
                  void uninstall(target);
                }}
                disabled={busy}
              >
                卸载
              </button>
            </div>
          </div>
        </div>
      )}
      <div className="dim">总线监听器 {registry.bus.listenerCount}</div>
    </section>
  );
}
