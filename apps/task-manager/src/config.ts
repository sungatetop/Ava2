// 配置自管（宿主零策略）：键名/存储格式/默认值全部由本 App 决定。
// App 与宿主同源，直接存 localStorage（对齐 chatapp 设置面板模式）。
const CFG_KEY = "task-manager:cfg";

export interface TaskManagerConfig {
  /** 删除任务前是否弹出气泡确认 */
  confirmDelete: boolean;
}

const DEFAULTS: TaskManagerConfig = { confirmDelete: true };

export function readConfig(): TaskManagerConfig {
  try {
    return { ...DEFAULTS, ...(JSON.parse(localStorage.getItem(CFG_KEY) || "{}") as Partial<TaskManagerConfig>) };
  } catch {
    return DEFAULTS;
  }
}

export function writeConfig(patch: Partial<TaskManagerConfig>): void {
  localStorage.setItem(CFG_KEY, JSON.stringify({ ...readConfig(), ...patch }));
}
