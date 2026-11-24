// AppHost 内核：slot 注册表 + App 状态机 + 平台服务（框架无关，React 绑定见 AppHost.tsx）
// 与 ava 原型的差异：
// - 无 backend plugin 通道（onion-server 无插件概念），storage / 启停状态统一落 localStorage；
// - 前端只装配 UI 页贡献（contributes.apps）；技能 / MCP server / hooks 由边车在会话
//   装配期读取 manifest 注入（见 crates/onion-server/src/app_contrib.rs）；
// - UI 页清单由 resolvePages 归一：contributes.apps 优先，缺省回落顶层 entry 单页。
import type {
  AppContext,
  AppIcon,
  AppManifest,
  AppModule,
  AppRecord,
  AppStatus,
  CanvasPanelSpec,
  CanvasSurfaceProps,
  Disposable,
  InjectKey,
  NavSpec,
  ResolvedPage,
  SettingsPanelFactory,
  SidebarPanelSpec,
  SlotCatalog,
  SlotKey,
  SurfaceSpec,
  UserMenuItemSpec,
} from "./types";
import { CLAIMING_SLOT_KEYS, SLOT_KEYS } from "./types";
import { INJECT_KEYS } from "./manifest";
import { getAgent, netFetch } from "./agent";
import { isDesktop } from "./env";

/** net.allow 域匹配：host 等于域或为域的子域（`api.skillhub.cn` 匹配
 *  `skillhub.cn`；域项忽略前导点）。 */
function hostAllowed(host: string, domain: string): boolean {
  const d = domain.trim().toLowerCase().replace(/^\./, "");
  if (d === "") return false;
  return host === d || host.endsWith(`.${d}`);
}

/** AppIcon 合法性校验：string 非空；对象形态 d 为 string 或非空 string[]（fill 可选）。
 *  约束见 docs/references/icon-spec.md。 */
function isValidAppIcon(icon: AppIcon): boolean {
  if (typeof icon === "string") return icon.trim().length > 0;
  if (typeof icon !== "object" || icon === null) return false;
  const paths = icon.d;
  if (typeof paths === "string") return paths.trim().length > 0;
  return Array.isArray(paths) && paths.length > 0 && paths.every((p) => typeof p === "string" && p.trim().length > 0);
}

type Listener = () => void;

/** 认领型 slot：viewId 即路由键，同一舞台内全局唯一 */
const CLAIMING_SLOTS: ReadonlySet<SlotKey> = new Set<SlotKey>(CLAIMING_SLOT_KEYS);

/** 能力面 → ctx 成员映射（注出收窄时按此校验访问） */
const CAPABILITY_SURFACES = ["slots", "events", "agent", "storage", "net"] as const;
type CapabilitySurface = (typeof CAPABILITY_SURFACES)[number];

/** 总线监听项：appId 缺省为宿主级（接收全部），否则为 App 级（只收广播 + 定向到自己的事件） */
interface BusListener {
  fn: (e: unknown) => void;
  appId?: string;
}

/** 只回宿主的事件（App 侧回执不进其他 App） */
const HOST_ONLY_EVENTS = new Set(["agent.hookResult"]);

/** 平台事件总线（带监听计数，供泄漏观测） */
class EventBus {
  private map = new Map<string, Set<BusListener>>();
  constructor(private onCountChange?: () => void) {}

  /** 宿主级订阅（壳与宿主内部模块）：接收全部事件 */
  on(type: string, fn: (e: unknown) => void): Disposable {
    return this.add(type, { fn });
  }

  /** App 级订阅：接收广播事件 + 定向到本 App 的事件（载荷已剥离 appId） */
  onApp(appId: string, type: string, fn: (e: unknown) => void): Disposable {
    return this.add(type, { fn, appId });
  }

  private add(type: string, listener: BusListener): Disposable {
    let set = this.map.get(type);
    if (!set) this.map.set(type, (set = new Set()));
    set.add(listener);
    this.onCountChange?.();
    return {
      dispose: () => {
        set!.delete(listener);
        this.onCountChange?.();
      },
    };
  }

  /** 宿主广播：全体可见 */
  emit(type: string, e: unknown): void {
    this.map.get(type)?.forEach((l) => l.fn(e));
  }

  /** 宿主定向投递：载荷带 appId；App 侧收到的是剥离 appId 后的副本 */
  emitDirected(type: string, e: unknown & { appId: string }): void {
    const { appId, ...rest } = e;
    this.map.get(type)?.forEach((l) => (l.appId === undefined ? l.fn(e) : l.appId === appId ? l.fn(rest) : undefined));
  }

  /** App 侧发射：回执类事件只回宿主，其余广播类事件全体可见 */
  emitFromApp(type: string, e: unknown): void {
    this.map.get(type)?.forEach((l) => (l.appId === undefined || !HOST_ONLY_EVENTS.has(type) ? l.fn(e) : undefined));
  }

  get listenerCount(): number {
    let n = 0;
    this.map.forEach((s) => (n += s.size));
    return n;
  }
}

/** 解析 App 的 UI 页清单（缺省按 plugin 级补齐）。
 *  contributes.apps 非空 → 逐页；否则顶层 entry 视为唯一主页（pageId = "."）。 */
export function resolvePages(manifest: AppManifest): ResolvedPage[] {
  const hostApi = manifest.hostApi ?? 0;
  const declared = manifest.contributes?.apps;
  if (Array.isArray(declared) && declared.length > 0) {
    return declared.map((p) => ({
      id: p.id,
      entry: p.entry,
      hostApi: p.hostApi ?? hostApi,
      inject: p.inject ?? manifest.inject ?? [],
      surfaces: p.surfaces ?? manifest.surfaces ?? [],
    }));
  }
  if (!manifest.entry) return [];
  return [
    {
      id: ".",
      entry: manifest.entry,
      hostApi,
      inject: manifest.inject ?? [],
      surfaces: manifest.surfaces ?? [],
    },
  ];
}

class Registry {
  private listeners = new Set<Listener>();
  private version = 0;

  readonly bus = new EventBus(() => this.touch());

  /** slotKey -> appId -> specs（有序） */
  private slots = new Map<SlotKey, Map<string, SlotCatalog[SlotKey][]>>();
  /** 认领型 slot 的路由键归属：slotKey -> viewId -> 认领者集合 */
  private claims = new Map<SlotKey, Map<string, Set<string>>>();
  private apps = new Map<
    string,
    AppRecord & { disposers: Disposable[]; module?: AppModule; loaded: boolean; settingsPanel?: SettingsPanelFactory }
  >();

  subscribe = (fn: Listener) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getVersion = () => this.version;

  private touch() {
    this.version++;
    this.listeners.forEach((fn) => fn());
  }

  /** 强制订阅组件重渲染（bootInfo 等注册表外快照更新后调用） */
  poke() {
    this.touch();
  }

  // ── slot 侧 ──
  /** 越权拦截：slot key 必须在宿主目录白名单内；页声明了 surfaces 时还须在其声明内 */
  inject(appId: string, key: SlotKey, spec: SlotCatalog[SlotKey], surfaces?: SlotKey[]): Disposable {
    if (!SLOT_KEYS.includes(key)) {
      throw new Error(`App「${appId}」注入越权 slot key：${key}（合法值 ${SLOT_KEYS.join("/")}）`);
    }
    const declared = surfaces ?? this.apps.get(appId)?.manifest.surfaces;
    if (declared && !declared.includes(key)) {
      throw new Error(`App「${appId}」未声明 slot「${key}」（manifest.surfaces：${declared.join("/")}）`);
    }
    // icon 校验（nav.sidebar / surface.canvas / panel.canvas / user.menu）：非法图标降级为无图标并告警，不阻断装配
    const icon = (spec as NavSpec | CanvasSurfaceProps | UserMenuItemSpec).icon;
    if (icon !== undefined && !isValidAppIcon(icon)) {
      console.warn(`[apphost] App「${appId}」icon 非法，已降级为无图标：`, icon);
      (spec as { icon?: AppIcon }).icon = undefined;
    }
    // 认领冲突检测：认领型 slot 的 viewId 是路由键，同一舞台内只允许一个 App 认领
    const viewId = CLAIMING_SLOTS.has(key) ? (spec as { viewId?: string }).viewId : undefined;
    let claimOwners: Set<string> | undefined;
    if (viewId) {
      let byView = this.claims.get(key);
      if (!byView) this.claims.set(key, (byView = new Map()));
      claimOwners = byView.get(viewId);
      if (!claimOwners) byView.set(viewId, (claimOwners = new Set()));
      const other = [...claimOwners].find((id) => id !== appId);
      if (other) {
        throw new Error(`viewId 冲突：「${viewId}」已被 App「${other}」认领（${key} 的 viewId 即路由键，全局唯一）`);
      }
      claimOwners.add(appId);
    }
    let byApp = this.slots.get(key);
    if (!byApp) this.slots.set(key, (byApp = new Map()));
    const list = byApp.get(appId) ?? [];
    byApp.set(appId, [...list, spec]);
    this.touch();
    return {
      dispose: () => {
        const cur = byApp!.get(appId);
        if (!cur) return;
        const next = cur.filter((s) => s !== spec);
        if (next.length === 0) byApp!.delete(appId);
        else byApp!.set(appId, next);
        if (claimOwners && viewId) {
          // 同一 App 可能对同 viewId 注入多次，最后一个撤回才释放认领
          const stillClaimed = (byApp!.get(appId) ?? []).some((s) => (s as { viewId?: string }).viewId === viewId);
          if (!stillClaimed) {
            claimOwners.delete(appId);
            if (claimOwners.size === 0) this.claims.get(key)!.delete(viewId);
          }
        }
        this.touch();
      },
    };
  }

  /** 仅返回 active App 的贡献（suspended/failed 自动不可见，模块保留） */
  navItems(): Array<NavSpec & { appId: string }> {
    return this.collect("nav.sidebar")
      .map((s) => ({ ...(s.spec as NavSpec), appId: s.appId }))
      .sort((a, b) => a.order - b.order);
  }

  surfaceFor(viewId: string): (SurfaceSpec & { appId: string }) | undefined {
    const hit = this.collect("surface.main").find((s) => (s.spec as SurfaceSpec).viewId === viewId);
    return hit ? { ...(hit.spec as SurfaceSpec), appId: hit.appId } : undefined;
  }

  sidebarPanels(viewId: string): Array<SidebarPanelSpec & { appId: string }> {
    return this.collect("panel.sidebar")
      .map((s) => ({ ...(s.spec as SidebarPanelSpec), appId: s.appId }))
      .filter((p) => p.viewId === viewId)
      .sort((a, b) => a.order - b.order);
  }

  canvasPanels(viewId: string): Array<CanvasPanelSpec & { appId: string }> {
    return this.collect("panel.canvas")
      .map((s) => ({ ...(s.spec as CanvasPanelSpec), appId: s.appId }))
      // viewId "*" = 全局画布面板：跟随任意画布视图（含 canvas:home），如文件树
      .filter((p) => p.viewId === "*" || p.viewId === viewId)
      .sort((a, b) => a.order - b.order);
  }

  /** 画布启动器：枚举全部 active App 的第二舞台贡献（预置在前、演示在后） */
  canvasSurfaces(): Array<CanvasSurfaceProps & { appId: string }> {
    const kindOrder = { preset: 0, demo: 1 } as const;
    return this.collect("surface.canvas")
      .map((s) => ({ ...(s.spec as CanvasSurfaceProps), appId: s.appId }))
      .sort(
        (a, b) =>
          kindOrder[this.appKind(a.appId)] - kindOrder[this.appKind(b.appId)] ||
          a.title.localeCompare(b.title, "zh"),
      );
  }

  /** App 展示归类：demo = 演示示例；preset = 平台预置（缺省） */
  appKind(appId: string): "preset" | "demo" {
    return this.apps.get(appId)?.manifest.kind ?? "preset";
  }

  /** App 展示图标（应用市场 / 启动器）：优先 manifest.icon（应用形象的权威声明，
   *  安装期静态可得，纯能力 App 也能有图标）；未声明时回落遍历该 App 注入的 slot，
   *  取第一个非空 icon（优先导航 nav.sidebar，其次画布视图 surface.canvas，再面板 panel.canvas）。包含禁用态 App。 */
  appIconFor(appId: string): AppIcon | undefined {
    const manifestIcon = this.apps.get(appId)?.manifest.icon;
    if (manifestIcon) return manifestIcon;
    const nav = this.slots.get("nav.sidebar")?.get(appId) ?? [];
    for (const spec of nav) {
      const icon = (spec as NavSpec).icon;
      if (icon) return icon;
    }
    const canvas = this.slots.get("surface.canvas")?.get(appId) ?? [];
    for (const spec of canvas) {
      const icon = (spec as CanvasSurfaceProps).icon;
      if (icon) return icon;
    }
    const panels = this.slots.get("panel.canvas")?.get(appId) ?? [];
    for (const spec of panels) {
      const icon = (spec as CanvasPanelSpec).icon;
      if (icon) return icon;
    }
    return undefined;
  }

  /** App 主视图（应用市场「打开」目标）：优先主区导航视图，其次画布视图 */
  appPrimaryView(appId: string): string | undefined {
    const nav = this.navItems().find((n) => n.appId === appId);
    if (nav) return nav.viewId;
    const canvas = this.canvasSurfaces().find((s) => s.appId === appId);
    return canvas?.viewId;
  }

  canvasSurfaceFor(viewId: string): (CanvasSurfaceProps & { appId: string }) | undefined {
    const hit = this.collect("surface.canvas").find((s) => (s.spec as CanvasSurfaceProps).viewId === viewId);
    return hit ? { ...(hit.spec as CanvasSurfaceProps), appId: hit.appId } : undefined;
  }

  /** 用户菜单项（user.menu）：active App 的菜单项，按 App 名称分组排序、组内按 order。
   *  展示层据此按相邻 appId 归组渲染（组头 = App 名，渲染层经 getRecord 取名）。 */
  userMenuItems(): Array<UserMenuItemSpec & { appId: string }> {
    const items = this.collect("user.menu").map((s) => ({ ...(s.spec as UserMenuItemSpec), appId: s.appId }));
    const nameOf = (appId: string) => this.apps.get(appId)?.manifest.name ?? appId;
    return items.sort(
      (a, b) =>
        nameOf(a.appId).localeCompare(nameOf(b.appId), "zh") || (a.order ?? 100) - (b.order ?? 100),
    );
  }

  private collect<K extends SlotKey>(key: K): Array<{ appId: string; spec: SlotCatalog[K] }> {
    const out: Array<{ appId: string; spec: SlotCatalog[K] }> = [];
    this.slots.get(key)?.forEach((specs, appId) => {
      const rec = this.apps.get(appId);
      // 未注册 appId = 宿主自身（如壳的"首页"），其贡献始终可见；
      // 已注册 App 仅 active 可见（suspended/failed 自动不可见，模块保留）
      if (!rec || rec.status === "active") specs.forEach((spec) => out.push({ appId, spec: spec as SlotCatalog[K] }));
    });
    return out;
  }

  // ── App 侧 ──
  listApps(): AppRecord[] {
    return [...this.apps.values()].map(({ manifest, status, error }) => ({ manifest, status, error }));
  }

  getRecord(id: string) {
    return this.apps.get(id);
  }

  /** 是否已完成过一次成功装配（纯能力 App 无 UI 页但已装配，不应重复 load） */
  isLoaded(id: string): boolean {
    return this.apps.get(id)?.loaded === true;
  }

  /** App 在 setup 期登记的设置面板工厂（未登记 = 无「设置」入口） */
  settingsPanelFor(id: string): SettingsPanelFactory | undefined {
    return this.apps.get(id)?.settingsPanel;
  }

  makeContext(
    manifest: AppManifest,
    pageId = ".",
    pageInject?: InjectKey[],
    pageSurfaces?: SlotKey[],
    pageHostApi?: number,
  ): AppContext {
    const self = this;
    // L4 Agent 通道：host 平台服务单例（每会话一条 WS 多路复用），所有 App 共享
    const agent = getAgent();
    const storage: AppContext["storage"] = {
      get: async (key) => {
        const raw = localStorage.getItem(`app:${manifest.id}:${key}`);
        // JSON.parse 返回 any：交给上下文泛型 T 推断
        return raw === null ? undefined : JSON.parse(raw);
      },
      set: async (key, value) => localStorage.setItem(`app:${manifest.id}:${key}`, JSON.stringify(value)),
    };
    // 按注出面收窄——未声明的能力面访问即响亮报错（hostApi 0 = 旧契约全量）。
    // 页级 hostApi 优先：plugin 0（旧契约全量）但页显式声明 1 时，该页按显式注出收窄。
    const granted =
      (pageHostApi ?? manifest.hostApi ?? 0) < 1
        ? new Set<InjectKey>(INJECT_KEYS)
        : new Set<InjectKey>(pageInject ?? manifest.inject ?? []);
    const deny = (surface: CapabilitySurface): never => {
      throw new Error(
        `App「${manifest.id}」页「${pageId}」访问未声明的能力面 ${surface}（inject：${[...granted].join(",") || "空"}）`,
      );
    };
    const slotsApi: AppContext["slots"] = {
      inject: (key, spec) => self.inject(manifest.id, key, spec, pageSurfaces),
    };
    const eventsApi: AppContext["events"] = {
      on: (type, fn) => {
        const d = self.bus.onApp(manifest.id, type, fn);
        self.apps.get(manifest.id)?.disposers.push(d); // 随 App 卸载统一排空
        return d;
      },
      emit: (type, e) => self.bus.emitFromApp(type, e),
    };
    // 统一联网服务：按 App 的 manifest `net.allow` 域白名单校验（缺省 = 全部
    // https 公网，服务端安全层兜底）；越权 fail-loud（与能力面收窄同语义）。
    const netApi: AppContext["net"] = {
      fetch: async (input) => {
        const allow = manifest.net?.allow;
        if (allow && allow.length > 0) {
          let host = "";
          try {
            host = new URL(input.url).host;
          } catch {
            /* 非法 URL 交给服务端报 invalid_url */
          }
          if (host && !allow.some((domain) => hostAllowed(host, domain))) {
            throw new Error(
              `App「${manifest.id}」外联越权：${host} 不在 net.allow 白名单（${allow.join(", ")}）`,
            );
          }
        }
        return netFetch(input);
      },
    };
    // 桌面能力桥：仅 Tauri 桌面端提供（浏览器端 undefined，App 须可选链消费）。
    // 原生对话框由用户交互驱动，不属于特权能力面，无需按注出面收窄。
    const desktop: AppContext["desktop"] | undefined = isDesktop()
      ? {
          pickFolder: async () => {
            try {
              const { invoke } = await import("@tauri-apps/api/core");
              return await invoke<string | null>("pick_folder");
            } catch (e) {
              console.warn("[apphost] 原生选择文件夹失败：", e);
              return null;
            }
          },
        }
      : undefined;
    return {
      manifest,
      pageId,
      desktop,
      get slots() {
        if (!granted.has("slots")) deny("slots");
        return slotsApi;
      },
      get events() {
        if (!granted.has("events")) deny("events");
        return eventsApi;
      },
      get agent() {
        if (!granted.has("agent")) deny("agent");
        return agent;
      },
      get net() {
        if (!granted.has("net")) deny("net");
        return netApi;
      },
      get storage() {
        if (!granted.has("storage")) deny("storage");
        return storage;
      },
      registerSettingsPanel: (factory: SettingsPanelFactory) => {
        const rec = self.apps.get(manifest.id);
        if (!rec) throw new Error(`App「${manifest.id}」未注册，无法登记设置面板`);
        rec.settingsPanel = factory;
        self.touch();
      },
    };
  }

  /** load：解析 UI 页 → 逐页 import → setup。
   *  任何失败 → failed（fail-loud，不影响其他 App）。幂等仅对「已加载 / 加载中」生效。 */
  async load(manifest: AppManifest): Promise<void> {
    const existing = this.apps.get(manifest.id);
    if (existing?.loaded || existing?.status === "loading") return;
    const record: AppRecord & {
      disposers: Disposable[];
      module?: AppModule;
      loaded: boolean;
      settingsPanel?: SettingsPanelFactory;
    } = {
      manifest,
      status: "loading",
      disposers: [],
      loaded: false,
    };
    this.apps.set(manifest.id, record);
    this.touch();
    try {
      const base = new URL(manifest.baseUrl ?? "/", location.origin);
      let first = true;
      for (const page of resolvePages(manifest)) {
        const url = new URL(page.entry, base);
        // URL 带 ?v=<version> 指纹：ESM 按 URL 缓存，缺指纹会导致热重载后仍跑旧模块
        if (!url.search && manifest.version) url.searchParams.set("v", manifest.version);
        const mod = (await import(/* @vite-ignore */ url.href)) as AppModule;
        if (typeof mod.setup !== "function") {
          throw new Error(`页「${page.id}」入口缺少 setup(ctx) 导出`);
        }
        if (first) {
          record.module = mod;
          first = false;
        }
        const disposer = mod.setup(this.makeContext(manifest, page.id, page.inject, page.surfaces, page.hostApi));
        if (disposer?.dispose) record.disposers.push(disposer);
      }
      record.status = "active";
      record.loaded = true;
    } catch (e) {
      record.status = "failed";
      record.error = e instanceof Error ? e.message : String(e);
      this.uninjectAll(manifest.id);
    }
    this.touch();
  }

  /** unload：排空订阅 → 移除 slot → setup disposer → 注销 */
  async unload(id: string): Promise<void> {
    const rec = this.apps.get(id);
    if (!rec) return;
    rec.status = "loading"; // 先不可见
    this.touch();
    this.uninjectAll(id);
    for (const d of rec.disposers.splice(0)) {
      try {
        d.dispose();
      } catch {
        /* 清理错误不阻断排空 */
      }
    }
    this.apps.delete(id);
    this.touch();
  }

  /** 热重载：unload 旧实例 → 以新 manifest 重新 load（调用方负责先 clearLazyCache） */
  async reload(manifest: AppManifest): Promise<void> {
    await this.unload(manifest.id);
    await this.load(manifest);
  }

  /** suspend：仅隐藏贡献，模块与注册保留（零成本恢复） */
  setStatus(id: string, status: AppStatus) {
    const rec = this.apps.get(id);
    if (rec && rec.status !== "loading") {
      rec.status = status;
      this.touch();
    }
  }

  /** 禁用态占位登记（模块未加载；"加载"按钮走 load()） */
  registerSuspended(manifest: AppManifest) {
    if (this.apps.has(manifest.id)) return;
    this.apps.set(manifest.id, { manifest, status: "suspended", disposers: [], loaded: false });
    this.touch();
  }

  private uninjectAll(appId: string) {
    this.slots.forEach((byApp) => {
      if (byApp.delete(appId)) this.touch();
    });
    // 同步回收认领（否则 failed/unload 的 App 会永久占住 viewId）
    this.claims.forEach((byView) => {
      byView.forEach((owners, viewId) => {
        if (owners.delete(appId) && owners.size === 0) byView.delete(viewId);
      });
    });
  }
}

export const registry = new Registry();
