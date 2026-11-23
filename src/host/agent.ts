// L4 Agent 通道平台服务（onion 版）：host 持有「每会话一条」WS 连接并多路分发，
// 引用归零即关闭；REST 会话管理端点一并收敛于此。App 不直连 WS。
// 事件流原样透传 onion `SessionNotice`（type 标签见 crates/onion-server/src/session.rs）。
import type {
  AddModelInput,
  AddProviderInput,
  AgentChannel,
  AgentEventFrame,
  AgentInfo,
  AnswerInput,
  CommandResultInfo,
  CreateTaskInput,
  DirListing,
  Disposable,
  GoalAction,
  GoalInfo,
  MessageAttachment,
  PendingItem,
  PlanInfo,
  ProviderInfo,
  SessionInfo,
  SessionListItem,
  SetGoalInput,
  NetFetchRequest,
  NetFetchResult,
  SkillHubInstallResult,
  SkillInfo,
  SkillManagerInfo,
  TaskInfo,
  TestModelInput,
  TestModelResult,
  UpdateTaskInput,
  TodoItem,
  WorkspaceFile,
} from "./types";

/** 后端基址（boot 时由 loader 经 env.resolveBackendBase 校准） */
let base = "http://127.0.0.1:9800";

export function setAgentBase(value: string): void {
  base = value.replace(/\/$/, "");
}

export function getAgentBase(): string {
  return base;
}

interface SessionConn {
  ws: WebSocket;
  listeners: Set<(e: AgentEventFrame) => void>;
}

const conns = new Map<string, SessionConn>();

/** 每会话单条 WS：首个订阅者建立，最后一个退订者关闭 */
function connFor(sessionId: string): SessionConn {
  const existing = conns.get(sessionId);
  if (existing) return existing;
  const ws = new WebSocket(`${base.replace(/^http/, "ws")}/v1/sessions/${encodeURIComponent(sessionId)}/ws`);
  const conn: SessionConn = { ws, listeners: new Set() };
  ws.onmessage = (ev) => {
    try {
      const frame = JSON.parse(ev.data as string) as AgentEventFrame;
      conn.listeners.forEach((fn) => fn(frame));
    } catch {
      /* 非 JSON 帧（如 session_not_found 纯文本）忽略 */
    }
  };
  ws.onclose = () => conns.delete(sessionId);
  conns.set(sessionId, conn);
  return conn;
}

/** 非 2xx → 统一 Error（带响应文本，便于 App 层 fail-loud 展示） */
async function unwrap(res: Response): Promise<Response> {
  if (res.ok) return res;
  let detail = "";
  try {
    detail = await res.text();
  } catch {
    /* 忽略 */
  }
  throw new Error(`HTTP ${res.status}${detail ? `: ${detail}` : ""}`);
}

const service: AgentChannel = {
  onSessionEvent(sessionId: string, fn: (e: AgentEventFrame) => void): Disposable {
    const conn = connFor(sessionId);
    conn.listeners.add(fn);
    return {
      dispose: () => {
        conn.listeners.delete(fn);
        if (conn.listeners.size === 0) {
          conns.delete(sessionId);
          if (conn.ws.readyState === WebSocket.OPEN || conn.ws.readyState === WebSocket.CONNECTING) conn.ws.close();
        }
      },
    };
  },

  async listSessions(): Promise<SessionListItem[]> {
    const r = await unwrap(await fetch(`${base}/v1/sessions`, { cache: "no-store" }));
    const data = (await r.json()) as { sessions?: SessionListItem[] };
    return data.sessions ?? [];
  },

  async listDirs(path?: string, opts?: { files?: boolean }): Promise<DirListing> {
    const params = new URLSearchParams();
    if (path) params.set("path", path);
    if (opts?.files) params.set("files", "1");
    const query = params.size > 0 ? `?${params}` : "";
    const r = await unwrap(await fetch(`${base}/v1/fs/dirs${query}`, { cache: "no-store" }));
    return (await r.json()) as DirListing;
  },

  getSession(id: string): Promise<SessionInfo> {
    return fetch(`${base}/v1/sessions/${encodeURIComponent(id)}`, { cache: "no-store" })
      .then(unwrap)
      .then((r) => r.json() as Promise<SessionInfo>);
  },

  /** 创建会话：cwd 为工作目录；agentId 选 Agent 预设（如 `creator` 创造模式），缺省走服务端默认。 */
  createSession(cwd?: string, agentId?: string): Promise<SessionInfo> {
    const body: Record<string, string> = {};
    if (cwd) body.cwd = cwd;
    if (agentId) body.agent = agentId;
    return fetch(`${base}/v1/sessions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    })
      .then(unwrap)
      .then((r) => r.json() as Promise<SessionInfo>);
  },

  async deleteSession(id: string): Promise<void> {
    try {
      await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}`, { method: "DELETE" }));
    } catch (e) {
      // 磁盘与内存都无该会话时 404 属预期（幂等删除）
      if (!(e instanceof Error) || !e.message.startsWith("HTTP 404")) throw e;
    }
  },

  async getMessages(id: string): Promise<AgentEventFrame[]> {
    const r = await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/messages`, { cache: "no-store" });
    if (r.status === 404) return [];
    return (await unwrap(r)).json() as Promise<AgentEventFrame[]>;
  },

  /** 工作区文件预览：相对路径按会话权限根解析；图片转 data URL；视频/音频/PDF 与
   *  Office 文档返回原始字节（媒体由前端 Blob URL 播放、Office 由解析库渲染）、
   *  其余按文本返回。超过服务端 2 MiB 上限时抛错。 */
  async readWorkspaceFile(path: string, sessionId?: string): Promise<WorkspaceFile> {
    const query = `path=${encodeURIComponent(path)}${sessionId ? `&session=${encodeURIComponent(sessionId)}` : ""}`;
    const r = await unwrap(await fetch(`${base}/v1/workspace/file?${query}`, { cache: "no-store" }));
    const mime = r.headers.get("content-type") ?? "application/octet-stream";
    if (mime.startsWith("image/")) {
      const blob = await r.blob();
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(new Error("图片读取失败"));
        reader.readAsDataURL(blob);
      });
      return { path, mime, dataUrl };
    }
    const wantsBinary =
      mime.startsWith("video/") ||
      mime.startsWith("audio/") ||
      mime === "application/pdf" ||
      mime.startsWith("application/vnd.openxmlformats-officedocument");
    if (wantsBinary) {
      return { path, mime, buffer: await r.arrayBuffer() };
    }
    return { path, mime, text: await r.text() };
  },

  async send(
    id: string,
    text: string,
    attachments: MessageAttachment[] = [],
  ): Promise<{ sessionId: string; taskId: string; completed: boolean }> {
    // parts 顺序：文本在前、附件在后（onion 侧 file → ContentBlock::Image）
    const parts: unknown[] = [];
    if (text) parts.push({ kind: "text", text });
    for (const a of attachments) {
      parts.push({ kind: "file", file: { name: a.name, mimeType: a.mimeType, uri: a.dataUrl } });
    }
    if (parts.length === 0) parts.push({ kind: "text", text: "" });
    const r = await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/messages`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        // wait=false：202 异步受理，轮次进度经 WS 推送
        body: JSON.stringify({ parts, wait: false }),
      }),
    );
    const ack = (await r.json()) as { sessionId: string; taskId: string; completed: boolean };
    return { sessionId: ack.sessionId, taskId: ack.taskId, completed: ack.completed };
  },

  async listModels(): Promise<{ defaultModel: string; providers: ProviderInfo[] }> {
    const r = await unwrap(await fetch(`${base}/v1/models`, { cache: "no-store" }));
    const data = (await r.json()) as { defaultModel?: string; providers?: ProviderInfo[] };
    return { defaultModel: data.defaultModel ?? "", providers: data.providers ?? [] };
  },

  async addModel(input: AddModelInput): Promise<{ provider: string; id: string }> {
    const r = await unwrap(
      await fetch(`${base}/v1/models`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as { provider: string; id: string };
  },

  async removeModel(provider: string, model: string): Promise<void> {
    await unwrap(
      await fetch(
        `${base}/v1/models/${encodeURIComponent(provider)}/${encodeURIComponent(model)}`,
        { method: "DELETE" },
      ),
    );
  },

  async setDefaultModel(model: string): Promise<{ defaultModel: string }> {
    const r = await unwrap(
      await fetch(`${base}/v1/models/default`, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ model }),
      }),
    );
    return (await r.json()) as { defaultModel: string };
  },

  async addProvider(input: AddProviderInput): Promise<{ id: string }> {
    const r = await unwrap(
      await fetch(`${base}/v1/providers`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as { id: string };
  },

  async removeProvider(id: string): Promise<void> {
    await unwrap(
      await fetch(`${base}/v1/providers/${encodeURIComponent(id)}`, { method: "DELETE" }),
    );
  },

  async listSkills(sessionId?: string): Promise<SkillInfo[]> {
    const query = sessionId ? `?session=${encodeURIComponent(sessionId)}` : "";
    const r = await unwrap(await fetch(`${base}/v1/skills${query}`, { cache: "no-store" }));
    const data = (await r.json()) as { skills?: SkillInfo[] };
    return data.skills ?? [];
  },

  async getSkillManager(sessionId?: string): Promise<SkillManagerInfo> {
    const query = sessionId ? `?session=${encodeURIComponent(sessionId)}` : "";
    const r = await unwrap(await fetch(`${base}/v1/skills-manager${query}`, { cache: "no-store" }));
    return (await r.json()) as SkillManagerInfo;
  },

  async putSkillManager(disabled: string[], sessionId?: string): Promise<SkillManagerInfo> {
    const query = sessionId ? `?session=${encodeURIComponent(sessionId)}` : "";
    const r = await unwrap(
      await fetch(`${base}/v1/skills-manager${query}`, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ disabled }),
      }),
    );
    return (await r.json()) as SkillManagerInfo;
  },

  async installSkillHub(slug: string, sessionId: string): Promise<SkillHubInstallResult> {
    const r = await unwrap(
      await fetch(`${base}/v1/skill-hub/install?session=${encodeURIComponent(sessionId)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ slug }),
      }),
    );
    return (await r.json()) as SkillHubInstallResult;
  },

  async testModel(input: TestModelInput): Promise<TestModelResult> {
    const r = await unwrap(
      await fetch(`${base}/v1/models/test`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as TestModelResult;
  },

  async listTasks(): Promise<{ tasks: TaskInfo[] }> {
    const r = await unwrap(await fetch(`${base}/v1/tasks`, { cache: "no-store" }));
    const data = (await r.json()) as { tasks?: TaskInfo[] };
    return { tasks: data.tasks ?? [] };
  },

  async createTask(input: CreateTaskInput): Promise<TaskInfo> {
    const r = await unwrap(
      await fetch(`${base}/v1/tasks`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as TaskInfo;
  },

  async toggleTask(id: string, enabled: boolean): Promise<TaskInfo> {
    const r = await unwrap(
      await fetch(`${base}/v1/tasks/${encodeURIComponent(id)}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ enabled }),
      }),
    );
    return (await r.json()) as TaskInfo;
  },

  async updateTask(id: string, input: UpdateTaskInput): Promise<TaskInfo> {
    const r = await unwrap(
      await fetch(`${base}/v1/tasks/${encodeURIComponent(id)}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as TaskInfo;
  },

  async deleteTask(id: string): Promise<void> {
    await unwrap(
      await fetch(`${base}/v1/tasks/${encodeURIComponent(id)}`, { method: "DELETE" }),
    );
  },

  async runTask(id: string): Promise<{ id: string; messageId: string }> {
    const r = await unwrap(
      await fetch(`${base}/v1/tasks/${encodeURIComponent(id)}/run`, { method: "POST" }),
    );
    return (await r.json()) as { id: string; messageId: string };
  },

  setSessionModel(id: string, model: string): Promise<SessionInfo> {
    return fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/model`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ model }),
    })
      .then(unwrap)
      .then((r) => r.json() as Promise<SessionInfo>);
  },

  setApprovalPreset(
    id: string,
    preset: "workspace-write" | "auto" | "danger-full-access",
  ): Promise<SessionInfo> {
    return fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/approval`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ preset }),
    })
      .then(unwrap)
      .then((r) => r.json() as Promise<SessionInfo>);
  },

  listAgents(): Promise<AgentInfo[]> {
    // GET /v1/agents 顶层即数组
    return fetch(`${base}/v1/agents`, { cache: "no-store" })
      .then(unwrap)
      .then((r) => r.json() as Promise<AgentInfo[]>);
  },

  async interrupt(id: string): Promise<void> {
    await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/interrupt`, { method: "POST" }));
  },

  async compact(id: string): Promise<void> {
    await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/compact`, { method: "POST" }));
  },

  async runCommand(id: string, name: string, args = ""): Promise<CommandResultInfo> {
    const r = await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/commands`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name, args }),
      }),
    );
    return (await r.json()) as CommandResultInfo;
  },

  async getPending(id: string): Promise<PendingItem[]> {
    const r = await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/pending`, { cache: "no-store" }));
    const data = (await r.json()) as { pending?: PendingItem[] };
    return data.pending ?? [];
  },

  async replyApproval(
    id: string,
    requestId: string,
    decision: "approve" | "approve_for_session" | "deny",
  ): Promise<void> {
    await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/approvals/${encodeURIComponent(requestId)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision }),
      }),
    );
  },

  async replyPlanReview(id: string, requestId: string, verdict: "approve" | "keep_planning" | "dismissed"): Promise<void> {
    await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/plan-reviews/${encodeURIComponent(requestId)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ verdict }),
      }),
    );
  },

  async answerQuestion(id: string, requestId: string, answers: AnswerInput[]): Promise<void> {
    await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/questions/${encodeURIComponent(requestId)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ answers }),
      }),
    );
  },

  async getTodos(id: string): Promise<TodoItem[]> {
    const r = await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/todos`, { cache: "no-store" }));
    const data = (await r.json()) as { todos?: TodoItem[] };
    return data.todos ?? [];
  },

  async getGoal(id: string): Promise<GoalInfo | null> {
    const r = await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/goal`, { cache: "no-store" });
    if (r.status === 404) return null;
    return (await unwrap(r)).json() as Promise<GoalInfo | null>;
  },

  async setGoal(id: string, input: SetGoalInput): Promise<GoalInfo | null> {
    const r = await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/goal`, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      }),
    );
    return (await r.json()) as GoalInfo | null;
  },

  async goalAction(id: string, action: GoalAction, revision: number, reason?: string): Promise<GoalInfo | null> {
    const r = await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/goal/actions`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ action, revision, ...(reason ? { reason } : {}) }),
      }),
    );
    return (await r.json()) as GoalInfo | null;
  },

  async getPlan(id: string): Promise<PlanInfo> {
    const r = await unwrap(await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/plan`, { cache: "no-store" }));
    return (await r.json()) as PlanInfo;
  },

  async setPlan(id: string, active: boolean): Promise<PlanInfo> {
    const r = await unwrap(
      await fetch(`${base}/v1/sessions/${encodeURIComponent(id)}/plan`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ active }),
      }),
    );
    return (await r.json()) as PlanInfo;
  },
};

let instance: AgentChannel | undefined;

/** AppContext.agent 单例：所有 App 共享同一组 WS 连接与 REST 客户端 */
export function getAgent(): AgentChannel {
  return (instance ??= service);
}

/** 统一联网服务（POST /v1/net/fetch 通用形态）：App 级域白名单由 registry 的
 *  net 能力面包装校验，本函数只负责传输。 */
export async function netFetch(input: NetFetchRequest): Promise<NetFetchResult> {
  const r = await unwrap(
    await fetch(`${base}/v1/net/fetch`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
  return (await r.json()) as NetFetchResult;
}
