// App 契约类型（ava2 落地版，蓝本 ava/prototypes/app-host-proto/src/host/types.ts）
// 与 ava 冻结契约的差异：
// - 去掉 PluginSource / backend plugin 通道（onion-server 无 plugin 概念，第一版纯静态预置 App）；
// - AgentChannel 重写为 onion-server 端点（会话/messages/WS/pending 回填/中断压缩 + 能力枚举）；
// - 去掉 ava 特有的 workspace 注册表 / 附件上传 / 文件搜索 / 审批档位 / workflow-state。
import type { ComponentType } from "react";

export interface Disposable {
  dispose(): void;
}

/** 注出能力键（manifest.inject 声明宿主注出哪些能力面；hostApi 0 = 旧契约全量） */
export type InjectKey = "slots" | "events" | "agent" | "storage" | "net";

/** L0：App manifest（预置与三方同构） */
export interface AppManifest {
  id: string;
  name: string;
  version: string;
  /** 一句话简介（应用市场/抽屉展示；缺省隐藏简介行） */
  description?: string;
  /** 开发者/团队展示名（详情面板「开发者：xxx」；缺省不渲染）。可选展示字段，不涉及 hostApi 升级。 */
  author?: string;
  /** 主页/仓库 URL（详情面板可点击；仅 http(s):// 前缀渲染为链接，其余按纯文本防注入）。 */
  homepage?: string;
  /** 开源协议：推荐 SPDX 标识符（如 MIT / Apache-2.0）；详情面板「协议：xxx」展示，缺省不渲染。 */
  license?: string;
  /** 应用展示图标（AppIcon）：应用形象的权威声明，安装期静态可得；
   *  slot 内 icon 为局部导航/面板场景，未声明时宿主回落 slot 遍历。 */
  icon?: AppIcon;
  /** plugin 根入口（ESM）。UI 页由 contributes.apps[] 声明；此字段为兼容位——
   *  等价于「首页入口」，缺省时取 contributes.apps[0].entry；纯能力 plugin（无 UI）可为空。 */
  entry: string;
  /** plugin 根 URL（宿主注入，非清单声明）：相对路径解析基准（skills/mcp/agents/UI 页）。 */
  baseUrl?: string;
  permissions?: string[];
  /** 宿主契约版本：0 = 旧契约（全量注入）；1 = inject 显式化；2 = 含 contributes */
  hostApi?: number;
  /** 声明式注出面（hostApi ≥ 1 生效）：未声明的能力面访问即响亮报错 */
  inject?: InjectKey[];
  /** 联网服务授权（hostApi ≥ 1 且 inject 含 "net" 生效）：App 级域白名单。
   *  缺省 = 全部 https 公网；声明时仅 allow 列表内的域名后缀可访问。 */
  net?: { allow?: string[] };
  /** 声明可注入的 slot key（越权 inject 加载期报错） */
  surfaces?: SlotKey[];
  /** 发现清单携带：预置 App 的启停状态（生产落本地配置） */
  enabled?: boolean;
  /** 展示归类：preset = 平台预置；demo = 演示示例。缺省视为 preset。 */
  kind?: "preset" | "demo";
  /** 能力贡献声明（hostApi ≥ 2 生效）：plugin 顶层唯一权威声明处。
   *  五类彼此独立、可组合；未声明 apps = 无 UI 页（纯能力 plugin）。 */
  contributes?: AppContributes;
}

/** UI 页声明（plugin 内的一个页面）：纯 UI，不含工具；同一 plugin 可声明多页 */
export interface AppPageDecl {
  /** 页标识（plugin 内唯一） */
  id: string;
  /** 相对 plugin 目录的 ESM 入口 */
  entry: string;
  /** 该页的宿主契约版本（缺省继承 plugin 的 hostApi） */
  hostApi?: number;
  /** 该页可访问的能力面（缺省继承 plugin 的 inject） */
  inject?: InjectKey[];
  /** 该页可注入的 slot key（缺省继承 plugin 的 surfaces） */
  surfaces?: SlotKey[];
}

/** 已解析的 UI 页（缺省已按 plugin 补齐） */
export interface ResolvedPage {
  id: string;
  entry: string;
  hostApi: number;
  inject: InjectKey[];
  surfaces: SlotKey[];
}

/** MCP server 贡献声明（短名；宿主强制合成 runtime 名）。
 *  transport：command(+args/env) 为 stdio；url 为 http/sse（二者其一，command 优先）。 */
export interface McpContribution {
  name: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
}

/** hook 事件名（对齐 harness HookEvent 的 12 个变体） */
export type HookEventName =
  | "PreToolUse"
  | "PostToolUse"
  | "PostToolUseFailure"
  | "UserPromptSubmit"
  | "SessionStart"
  | "SessionEnd"
  | "Stop"
  | "SubagentStart"
  | "SubagentStop"
  | "PreCompact"
  | "PostCompact"
  | "Notification";

/** hook 处理形态：bus = App 内回调；command = 外部进程 */
export type HookHandler =
  | { type: "bus" }
  | { type: "command"; command: string };

export interface HookContribution {
  event: HookEventName;
  matcher?: string;
  handler: HookHandler;
}

/** 能力贡献声明面（hostApi 2）：plugin 是清单，宿主是执行者。
 *  工具不在此列——一律经 `mcpServers` 声明的 MCP server 提供。
 *  每类均支持「内联声明数组」与「文件/目录引用 `{ path }`」两种形态，加载期归一。 */
export interface AppContributes {
  /** 技能：SKILL.md 路径或技能目录 */
  skills?: Array<string | { path: string }>;
  /** MCP server：内联数组或 `{ path: "./mcp.json" }` 文件引用 */
  mcpServers?: McpContribution[] | { path: string };
  /** 智能体：智能体定义文件或目录 */
  agents?: Array<string | { path: string }>;
  /** hooks：内联数组或 `{ path }` 文件引用 */
  hooks?: HookContribution[] | { path: string };
  /** UI 页（多页面） */
  apps?: AppPageDecl[];
}

/** 已解析的 MCP 贡献 */
export interface ResolvedMcp extends McpContribution {
  args: string[];
  env: Record<string, string>;
  fullName: string;
}

/** 已解析的技能贡献 */
export interface ResolvedSkill {
  path: string;
  dir: string;
  fullName: string;
  kind: "file" | "dir";
}

/** 已解析的智能体贡献 */
export interface ResolvedAgent {
  path: string;
  fullName: string;
  kind: "file" | "dir";
}

/** 已解析的 hook 贡献 */
export interface ResolvedHook extends HookContribution {
  key: string;
}

/** 宿主解析后的贡献面（命名空间已合成、缺省已补齐） */
export interface ResolvedContributes {
  skills: ResolvedSkill[];
  mcp: ResolvedMcp[];
  agents: ResolvedAgent[];
  hooks: ResolvedHook[];
  pages: ResolvedPage[];
}

/** L1：slot 目录（宿主声明；App 只能 inject，越权 key 加载期报错） */
export interface SurfaceSpec {
  viewId: string;
  title: string;
  /** 惰性工厂：首次渲染才 import 组件（setup 只登记，不做副作用） */
  component: () => Promise<{ default: ComponentType<Record<string, unknown>> }>;
}
/**
 * App 自定义图标（宿主统一渲染，约束见 `docs/references/icon-spec.md`）：
 * - `string`：单个 SVG path `d`（24×24 viewBox、stroke 1.8、currentColor，lucide 同源网格）；
 * - `{ d, fill }`：对象形态，`d` 可为单路径或多路径数组；`fill: true` 时按填充绘制
 *   （logo / 品牌标记类），默认描边。
 */
export type AppIcon =
  | string
  | { d: string | string[]; fill?: boolean };

export interface NavSpec {
  order: number;
  viewId: string;
  title: string;
  /** 导航图标（AppIcon，见类型注释与 icon-spec） */
  icon?: AppIcon;
}
export interface SidebarPanelSpec {
  order: number;
  viewId: string;
  title: string;
  component: () => Promise<{ default: ComponentType<Record<string, unknown>> }>;
}
export interface CanvasPanelSpec {
  order: number;
  viewId: string;
  title: string;
  /** 面板图标（应用市场展示用；AppIcon，见 icon-spec） */
  icon?: AppIcon;
  component: () => Promise<{ default: ComponentType<Record<string, unknown>> }>;
}
export interface CanvasSurfaceProps {
  viewId: string;
  title: string;
  icon?: AppIcon;
  description?: string;
  component: () => Promise<{ default: ComponentType<Record<string, unknown>> }>;
}

/** 用户菜单动作（user.menu）：纯数据、宿主解释执行——跨 App 边界只传数据不传函数。
 *  openView 优先走画布舞台（canvasSurfaceFor 可解析时），否则主区 tab，与 AppList「打开」同路由；
 *  openSettings 打开本 App 的设置面板（setup 期 registerSettingsPanel 登记者）；
 *  event 由宿主经总线广播，App 自行 on 监听处理（任意自定义动作）。 */
export type UserMenuAction =
  | { kind: "openView"; viewId: string }
  | { kind: "openSettings" }
  | { kind: "event"; type: string; payload?: unknown };

/** 用户菜单项（user.menu slot 贡献）：侧边栏 user 栏点击后上浮菜单中的 App 区菜单项 */
export interface UserMenuItemSpec {
  /** 菜单项 id（plugin 内唯一，展示 key 与排查标识） */
  id: string;
  title: string;
  /** 菜单项图标（AppIcon，见 icon-spec） */
  icon?: AppIcon;
  /** 组内排序，缺省 100 */
  order?: number;
  /** 点击动作（缺省为无动作静态项） */
  action?: UserMenuAction;
}

export interface SlotCatalog {
  "surface.main": SurfaceSpec;
  "surface.canvas": CanvasSurfaceProps;
  "nav.sidebar": NavSpec;
  "panel.sidebar": SidebarPanelSpec;
  "panel.canvas": CanvasPanelSpec;
  "user.menu": UserMenuItemSpec;
}
export type SlotKey = keyof SlotCatalog;

/** slot key 运行时白名单（注入期校验，防越权 key） */
export const SLOT_KEYS: readonly SlotKey[] = [
  "surface.main",
  "surface.canvas",
  "nav.sidebar",
  "panel.sidebar",
  "panel.canvas",
  "user.menu",
];

/** 认领型 slot：viewId 即路由键，同一舞台内**全局唯一**。
 *  导航型（nav.sidebar）与面板型（panel.*）的 viewId 是对主舞台视图的**引用**，不参与认领。 */
export const CLAIMING_SLOT_KEYS: readonly SlotKey[] = ["surface.main", "surface.canvas"];

/** 设置面板惰性工厂（App 在 setup 期经 ctx.registerSettingsPanel 登记）：
 *  宿主只提供「设置」入口与惰性挂载/错误隔离；表单 UI、存储（App 同源可直用
 *  localStorage/fetch）、校验、密钥处理全部由 App 开发者自行决定。 */
export type SettingsPanelFactory = () => Promise<{ default: ComponentType<Record<string, unknown>> }>;

// ---------------- onion-server DTO（对齐 crates/onion-server/src/rest/dto.rs） ----------------

/** 会话详情（GET /v1/sessions/{id}，camelCase 投影） */
export interface SessionInfo {
  id: string;
  agent: string;
  interactive: boolean;
  busy: boolean;
  workingDir: string;
  root: string;
  /** 下一请求模型目标（运行期 /model 切换生效） */
  model: string;
  /** 当前审批预设（workspace-write / auto / danger-full-access） */
  approvalPreset: string;
  createdAtMs: number;
  eventCount: number;
  /** 当前上下文估算占用（durable 事件 chars/4 估算，对齐 TUI 口径） */
  contextTokens: number;
  /** 上下文窗口上限（模型配置） */
  contextWindowTokens: number;
  lastTurn?: {
    turn: number;
    steps: number;
    reason: string;
    /** 该轮各步 usage 累计 */
    usage: TurnUsage;
  };
  plan: { active: boolean; pending: boolean | null };
}

/** 单轮 token 用量（onion `Usage` 的 camelCase 投影） */
export interface TurnUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
}

/** 会话列表项（GET /v1/sessions） */
export interface SessionListItem {
  id: string;
  live: boolean;
  eventCount: number;
  title: string;
  goalPhase: string;
  createdAtMs?: number;
  /** 所属工作区绝对路径（会话历史按工作区分组；旧会话无法判定时缺省） */
  workingDir?: string;
  /** 最近活跃时间（epoch ms），会话历史按此倒序 */
  updatedAtMs?: number;
}

/** 会话事件帧：onion `SessionNotice` 的 `type` 标签（snake_case），投影层未定前原样透传 */
export type AgentEventFrame = { type: string } & Record<string, unknown>;

/** 待回填输入项（GET /v1/sessions/{id}/pending） */
export interface PendingItem {
  requestId: string;
  kind: "approval" | "plan_review" | "question";
  createdAtMs: number;
  tool?: string;
  input?: unknown;
  /** 审批来源标记（`sandbox-retry` = 沙箱拒绝后的越界执行申请；缺省 = 首次审批） */
  reason?: string;
  plan?: string;
  callId?: string;
  questions?: QuestionInfo[];
}

/** 问询问题（camelCase 投影） */
export interface QuestionInfo {
  id: string;
  question: string;
  header?: string;
  options: { label: string; description?: string }[];
  multiSelect: boolean;
}

/** 问询答复条目 */
export interface AnswerInput {
  id: string;
  selected: string[];
  custom?: string;
}

/** 待办单条（onion `TodoItem` 直传；GET /v1/sessions/{id}/todos 与 durable todo_write 共用） */
export interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed";
}

/** goal 受阻原因（onion BlockedReason 投影） */
export interface GoalBlockedReason {
  code: string;
  message: string;
}

/** goal 快照（GET/PUT /v1/sessions/{id}/goal，camelCase 投影） */
export interface GoalInfo {
  id: string;
  revision: number;
  objective: string;
  /** active / paused / blocked / complete（onion GoalPhase 标签） */
  phase: string;
  blockedReason?: GoalBlockedReason;
  maxGoalRounds: number;
  roundsStarted: number;
  createdAtMs: number;
  updatedAtMs: number;
  armed: boolean;
}

/** goal 生命周期动作（POST /v1/sessions/{id}/goal/actions） */
export type GoalAction = "pause" | "resume" | "complete" | "block" | "clear";

/** goal 创建/编辑入参（PUT /v1/sessions/{id}/goal；已存在时必须携带 revision） */
export interface SetGoalInput {
  objective?: string;
  revision?: number;
  maxGoalRounds?: number;
}

/** plan 模式状态（GET/POST /v1/sessions/{id}/plan）：pending 非空表示切换将在
 *  当前轮次的步骤边界生效（onion PlanWanted 中间态）。 */
export interface PlanInfo {
  active: boolean;
  pending: boolean | null;
}

/** 斜杠命令执行结果（POST /v1/sessions/{id}/commands 的返回体）。 */
export interface CommandResultInfo {
  commandId: string;
  /** `success` / `error` */
  kind: "success" | "error";
  text: string;
}

/** 供应商分组模型（GET /v1/models） */
export interface ProviderInfo {
  id: string;
  name: string;
  baseUrl: string;
  models: ModelInfo[];
}
export interface ModelInfo {
  id: string;
  name: string;
  description?: string;
}

/** 模型管理：新增模型入参（POST /v1/models） */
export interface AddModelInput {
  provider: string;
  id: string;
  name?: string;
  description?: string;
  /** 供应商不存在时必填（顺带创建供应商） */
  baseUrl?: string;
  apiKey?: string;
}

/** 模型管理：新增/替换供应商入参（POST /v1/providers） */
export interface AddProviderInput {
  id: string;
  name?: string;
  baseUrl: string;
  apiKey: string;
}

/** 模型管理：连接测试入参（POST /v1/models/test） */
export interface TestModelInput {
  /** 目录解析路径：provider + model（密钥不出边车） */
  provider?: string;
  model?: string;
  /** 直连路径：表单未保存前测试 */
  baseUrl?: string;
  apiKey?: string;
}

/** 模型管理：连接测试结果 */
export interface TestModelResult {
  ok: boolean;
  latencyMs: number;
  error?: string;
}

/** 技能目录条目（GET /v1/skills 出参；**含禁用技能**，enabled 仅标记状态） */
export interface SkillInfo {
  name: string;
  description: string;
  whenToUse?: string;
  /** 来源桶：project-onion / project-claude / project-dsh / project-agents /
   *  runtime / custom / user-onion / user-claude / user-dsh / user-agents / bundled */
  source: string;
  provider: string;
  path?: string;
  modelInvocable: boolean;
  userInvocable: boolean;
  /** skills-manager 视角：未被禁用为 true；禁用仅影响注入，不从列表移除 */
  enabled: boolean;
}

/** 技能启停清单（GET/PUT /v1/skills-manager 出参） */
export interface SkillManagerInfo {
  /** 禁用技能名（排序稳定输出） */
  disabled: string[];
  /** 落盘文件绝对路径（`<projectRoot>/.onion/skills-manager.json`） */
  file: string;
}

/** 技能广场条目（App 经 ctx.net 统一联网服务请求 SkillHub 搜索 API 后自解析的原始条目） */
export interface SkillHubSkill {
  slug: string;
  name: string;
  summary: string;
  version: string;
  /** 上游来源桶：community（clawhub）/ enterprise 等 */
  source: string;
  downloads: number;
  stars: number;
  tags: string[];
  publisher?: string;
  verified: boolean;
  iconUrl?: string;
  homepage?: string;
  /** 上游命名空间坐标（如 `@clawhub_wu-uk/pdf`，可为 handle 或 canonical 全坐标） */
  namespace?: string;
}

/** 技能广场详情（App 经 ctx.net 统一联网服务请求 SkillHub resolve API 后自解析） */
export interface SkillHubDetail {
  slug: string;
  name: string;
  summary: string;
  version: string;
  source: string;
  downloads: number;
  installs: number;
  stars: number;
  owner?: string;
  namespace?: string;
  sourceUrl?: string;
  iconUrl?: string;
  verified: boolean;
  /** 上游更新时间（Unix 毫秒） */
  updatedAt?: number;
}

/** 外联请求（POST /v1/net/fetch）：method 仅 GET/POST；headers 白名单透传
 *  （accept / authorization / content-type / user-agent）；body 仅 POST；
 *  encoding 决定响应编码。安全边界（https 公网 / SSRF / 上限 / 超时）在服务端。 */
export interface NetFetchRequest {
  url: string;
  method?: "GET" | "POST";
  headers?: Record<string, string>;
  body?: string;
  /** text（缺省，UTF-8）| dataUrl（`data:<type>;base64,`，图片等二进制场景） */
  encoding?: "text" | "dataUrl";
}

/** 外联出参（GET /v1/net/fetch 与 POST 通用）：上游状态码透传，非 200 由调用方判定 */
export interface NetFetchResult {
  /** 上游 HTTP 状态码 */
  status: number;
  /** 上游 Content-Type（缺省空串） */
  contentType: string;
  /** 响应体（encoding=text 为 UTF-8 文本；dataUrl 为 `data:<type>;base64,`） */
  body: string;
}

/** 统一联网服务（能力面 `net`）：App 唯一外联出口。
 *  App 级授权：manifest `inject` 声明 "net" + 可选 `net.allow` 域白名单
 *  （缺省 = 全部 https 公网；host 层按调用 App 校验，越权 fail-loud）。 */
export interface NetApi {
  fetch(input: NetFetchRequest): Promise<NetFetchResult>;
}

/** 技能广场安装结果（POST /v1/skill-hub/install 出参） */
export interface SkillHubInstallResult {
  /** 安装目录名（slug） */
  slug: string;
  /** SKILL.md frontmatter name（会话内调用名，可能与 slug 不同） */
  name: string;
  /** 落盘目录绝对路径（`<projectRoot>/.onion/skills/<slug>`） */
  dir: string;
  /** zip 顶层条目名 */
  files: string[];
}

/** 定时任务视图（GET /v1/tasks 出参） */
export interface TaskInfo {
  id: string;
  title: string;
  prompt: string;
  /** single = 绑定会话投递；independent = 到期新建会话投递 */
  sessionMode: "single" | "independent";
  /** 绑定会话 id（independent 递送前为空） */
  sessionId: string;
  enabled: boolean;
  /** 频率类型：at / after / every / daily / weekly / cron */
  kind: string;
  /** 下次触发（RFC3339） */
  scheduledAt: string;
  /** 最近一次递送时间（RFC3339；无则缺省） */
  lastDeliveredAt?: string;
}

/** 定时任务创建入参（频率恰好一个；对齐边车 FrequencySpec） */
export type CreateTaskFrequency =
  | { kind: "at"; instant: string }
  | { kind: "every"; seconds: number }
  | { kind: "daily"; time: string; time_zone: string }
  | { kind: "weekly"; time: string; time_zone: string; weekdays: number[] }
  | { kind: "cron"; expression: string; time_zone: string };

export interface CreateTaskInput {
  title: string;
  prompt: string;
  /** 缺省 single */
  sessionMode?: "single" | "independent";
  /** single 模式的绑定会话 */
  sessionId?: string;
  frequency: CreateTaskFrequency;
}

/** 定时任务编辑入参（字段均可选；frequency 缺省表示保持原计划） */
export interface UpdateTaskInput {
  enabled?: boolean;
  title?: string;
  prompt?: string;
  frequency?: CreateTaskFrequency;
}

/** 智能体（GET /v1/agents，数组） */
export interface AgentInfo {
  id: string;
  name: string;
  tools: string[];
}

/** 目录枚举项（GET /v1/fs/dirs） */
export interface DirEntry {
  /** 目录名（末段） */
  name: string;
  /** 绝对路径（可直接作为会话 cwd） */
  path: string;
}

/** 文件枚举项（仅请求 files 选项时返回；仅名字与路径，不含内容） */
export interface FileEntry {
  name: string;
  /** 绝对路径（可传给 readWorkspaceFile / canvas.openPath 预览） */
  path: string;
}

/** 工作区文件预览结果（GET /v1/workspace/file） */
export interface WorkspaceFile {
  /** 请求路径原样回显 */
  path: string;
  /** 响应 Content-Type（图片 `image/*`，文本含 charset） */
  mime: string;
  /** 文本内容（非图片、非二进制媒体） */
  text?: string;
  /** 图片 data URL（仅图片） */
  dataUrl?: string;
  /** 二进制原始字节（视频/音频/PDF 与 Office 文档 docx/xlsx/pptx：媒体由前端
   *  Blob URL 播放，Office 由前端解析库渲染） */
  buffer?: ArrayBuffer;
}

/** 目录枚举结果（工作区选择器 / 文件浏览器的数据源） */
export interface DirListing {
  /** 当前目录绝对路径 */
  path: string;
  /** 上级目录；已是根时缺省 */
  parent?: string | null;
  entries: DirEntry[];
  /** 常规文件（仅 listDirs 传入 files 选项时非空） */
  files?: FileEntry[];
  /** 可用驱动器根（仅 Windows 非空） */
  roots: string[];
}

/** 消息附件（随用户消息提交）。当前服务端契约只把 `file` part 映射为
 *  Image 内容块（`ContentBlock::Image`），故仅图片可作附件；其余类型由 App 拒绝。 */
export interface MessageAttachment {
  /** 展示名（文件名） */
  name: string;
  /** MIME 类型（`image/png` / `image/jpeg` / `image/webp` / `image/gif`） */
  mimeType: string;
  /** 内容载体：data URL（`data:<mime>;base64,<payload>`），同时落盘进 user_message 供回放 */
  dataUrl: string;
}

/** L4 Agent 通道（onion 版）：host 平台服务，App 不直连 WS。
 *  事件流透传 onion `SessionNotice`（投影层位置见 design 文档 Q1，未定前原样下发）。 */
export interface AgentChannel {
  /** 订阅某会话的事件流（snapshot/durable/live/approval_requested/turn_report/session_closed…） */
  onSessionEvent(sessionId: string, fn: (e: AgentEventFrame) => void): Disposable;
  listSessions(): Promise<SessionListItem[]>;
  /** 枚举目录的子目录（GET /v1/fs/dirs；path 缺省为用户主目录）。
   *  `opts.files` 为真时同时返回当前目录的常规文件（文件浏览器语义）。 */
  listDirs(path?: string, opts?: { files?: boolean }): Promise<DirListing>;
  getSession(id: string): Promise<SessionInfo>;
  createSession(cwd?: string, agentId?: string): Promise<SessionInfo>;
  deleteSession(id: string): Promise<void>;
  getMessages(id: string): Promise<AgentEventFrame[]>;
  /** 读取工作区文件用于预览（GET /v1/workspace/file；相对路径按会话权限根解析，
   *  根内绝对路径亦可）。图片返回 data URL，其余按文本返回。 */
  readWorkspaceFile(path: string, sessionId?: string): Promise<WorkspaceFile>;
  /** 发送消息（POST /v1/sessions/{id}/messages，parts[text, file...]，wait=false 异步）。
   *  `attachments` 为空时等价于纯文本，`text` 为空但带附件时只提交附件 parts。 */
  send(
    id: string,
    text: string,
    attachments?: MessageAttachment[],
  ): Promise<{ sessionId: string; taskId: string; completed: boolean }>;
  listModels(): Promise<{ defaultModel: string; providers: ProviderInfo[] }>;
  /** 模型管理：新增（或替换同 id）模型（POST /v1/models）；供应商不存在时须带
   *  baseUrl + apiKey（顺带创建供应商）。 */
  addModel(input: AddModelInput): Promise<{ provider: string; id: string }>;
  /** 模型管理：移除模型（DELETE /v1/models/{provider}/{model}）；默认模型须先切换。 */
  removeModel(provider: string, model: string): Promise<void>;
  /** 模型管理：设默认模型（PUT /v1/models/default；仅影响之后装配的新会话）。 */
  setDefaultModel(model: string): Promise<{ defaultModel: string }>;
  /** 模型管理：新增（或替换同 id）供应商（POST /v1/providers）。 */
  addProvider(input: AddProviderInput): Promise<{ id: string }>;
  /** 技能目录（GET /v1/skills；含禁用技能，enabled 仅标记状态）。 */
  listSkills(sessionId?: string): Promise<SkillInfo[]>;
  /** 技能启停：读取工作区 `<projectRoot>/.onion/skills-manager.json`（GET /v1/skills-manager）。 */
  getSkillManager(sessionId?: string): Promise<SkillManagerInfo>;
  /** 技能启停：全量替换禁用名单（PUT /v1/skills-manager；非法名 400）。 */
  putSkillManager(disabled: string[], sessionId?: string): Promise<SkillManagerInfo>;
  /** 技能广场：安装到会话工作区 `<projectRoot>/.onion/skills/<slug>/`（POST /v1/skill-hub/install；
   *  已存在 409；企业源 / 未知 slug 404）。 */
  installSkillHub(slug: string, sessionId: string): Promise<SkillHubInstallResult>;
  /** 模型管理：移除供应商（DELETE /v1/providers/{id}）；组内有模型时 409。 */
  removeProvider(id: string): Promise<void>;
  /** 模型管理：连接测试（POST /v1/models/test）。provider+model 从目录解析
   *  （密钥不出边车）；或直接传 baseUrl+apiKey+model（表单未保存前测试）。 */
  testModel(input: TestModelInput): Promise<TestModelResult>;
  /** 定时任务：列表（GET /v1/tasks；全局聚合，按下次触发升序）。 */
  listTasks(): Promise<{ tasks: TaskInfo[] }>;
  /** 定时任务：创建（POST /v1/tasks）。sessionMode 缺省 single；
   *  independent 模式调度器到期新建会话投递。 */
  createTask(input: CreateTaskInput): Promise<TaskInfo>;
  /** 定时任务：启停（PATCH /v1/tasks/{id}）。 */
  toggleTask(id: string, enabled: boolean): Promise<TaskInfo>;
  /** 定时任务：编辑标题 / 指令 / 频率（PATCH /v1/tasks/{id}；频率缺省保持原计划）。 */
  updateTask(id: string, input: UpdateTaskInput): Promise<TaskInfo>;
  /** 定时任务：删除（DELETE /v1/tasks/{id}）。 */
  deleteTask(id: string): Promise<void>;
  /** 定时任务：立即执行（POST /v1/tasks/{id}/run；语义等同到期递送）。 */
  runTask(id: string): Promise<{ id: string; messageId: string }>;
  /** 运行期切换会话模型目标（PUT /v1/sessions/{id}/model，下一请求生效） */
  setSessionModel(id: string, model: string): Promise<SessionInfo>;
  /** 运行期切换会话审批预设（PUT /v1/sessions/{id}/approval） */
  setApprovalPreset(id: string, preset: "workspace-write" | "auto" | "danger-full-access"): Promise<SessionInfo>;
  listAgents(): Promise<AgentInfo[]>;
  interrupt(id: string): Promise<void>;
  compact(id: string): Promise<void>;
  /** 执行一条斜杠命令（POST /v1/sessions/{id}/commands）：宿主落 durable
   *  `command_run` / `command_done`，前端只消费事件流。 */
  runCommand(id: string, name: string, args?: string): Promise<CommandResultInfo>;
  getPending(id: string): Promise<PendingItem[]>;
  replyApproval(
    id: string,
    requestId: string,
    decision: "approve" | "approve_for_session" | "deny",
  ): Promise<void>;
  replyPlanReview(id: string, requestId: string, verdict: "approve" | "keep_planning" | "dismissed"): Promise<void>;
  answerQuestion(id: string, requestId: string, answers: AnswerInput[]): Promise<void>;
  /** 待办清单（GET /v1/sessions/{id}/todos；快照回放取末次 todo_write）。 */
  getTodos(id: string): Promise<TodoItem[]>;
  /** goal 读取（GET /v1/sessions/{id}/goal；未设置时 null）。 */
  getGoal(id: string): Promise<GoalInfo | null>;
  /** goal 创建/编辑（PUT /v1/sessions/{id}/goal；已存在时须带 revision）。 */
  setGoal(id: string, input: SetGoalInput): Promise<GoalInfo | null>;
  /** goal 生命周期动作（POST /v1/sessions/{id}/goal/actions；revision 为乐观锁）。 */
  goalAction(id: string, action: GoalAction, revision: number, reason?: string): Promise<GoalInfo | null>;
  /** plan 模式读取（GET /v1/sessions/{id}/plan）。 */
  getPlan(id: string): Promise<PlanInfo>;
  /** plan 模式开关（POST /v1/sessions/{id}/plan；busy 时 pending=true 延迟到步骤边界）。 */
  setPlan(id: string, active: boolean): Promise<PlanInfo>;
}

/** L3：AppContext——App 在 setup(ctx) 中拿到的唯一 API 面（组件拿不到 ctx） */
export interface AppContext {
  readonly manifest: AppManifest;
  /** 当前 setup 对应的 UI 页 id（多页面 plugin）：未声明 contributes.apps 时为主页 `.` */
  readonly pageId: string;
  slots: {
    inject<K extends SlotKey>(key: K, spec: SlotCatalog[K]): Disposable;
  };
  /** 平台事件总线（App↔壳、App↔App 同一总线）。
   *  canvas 舞台约定的总线事件：canvas.open / canvas.openPath / agent.invoke / agent.reply */
  events: {
    on(type: string, fn: (e: unknown) => void): Disposable;
    emit(type: string, e: unknown): void;
  };
  /** L4 Agent 通道平台服务（onion 版） */
  agent: AgentChannel;
  /** 统一联网服务（能力面 `net`）：App 唯一外联出口；manifest `net.allow`
   *  域白名单（缺省全部 https 公网）由宿主按调用 App 校验。 */
  net: NetApi;
  /** 按 appId 命名空间的 KV（localStorage，契约统一异步） */
  storage: {
    get<T>(key: string): Promise<T | undefined>;
    set<T>(key: string, value: T): Promise<void>;
  };
  /** 登记本 App 的设置面板（类比 Android 的 settings activity 声明）：
   *  应用市场详情页随之出现「设置」入口，点击弹层惰性挂载面板组件。
   *  纯声明，不占能力面；宿主不做任何配置存储/校验/回显处理。 */
  registerSettingsPanel(factory: SettingsPanelFactory): void;
}

/** App ESM 入口契约：export function setup(ctx): Disposable | void */
export interface AppModule {
  setup: (ctx: AppContext) => Disposable | void;
}

export type AppStatus = "loading" | "active" | "suspended" | "failed";

export interface AppRecord {
  manifest: AppManifest;
  status: AppStatus;
  error?: string;
}
