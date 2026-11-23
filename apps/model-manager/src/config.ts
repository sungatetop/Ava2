// 配置自管（宿主零策略）：键名/存储格式/默认值全部由本 App 决定。
// App 与宿主同源，直接存 localStorage（对齐 chatapp / task-manager 设置面板模式）。
const CFG_KEY = "model-manager:cfg";

export interface ModelManagerConfig {
  /** 删除供应商/模型前是否弹出气泡确认 */
  confirmDelete: boolean;
}

const DEFAULTS: ModelManagerConfig = { confirmDelete: true };

export function readConfig(): ModelManagerConfig {
  try {
    return { ...DEFAULTS, ...(JSON.parse(localStorage.getItem(CFG_KEY) || "{}") as Partial<ModelManagerConfig>) };
  } catch {
    return DEFAULTS;
  }
}

export function writeConfig(patch: Partial<ModelManagerConfig>): void {
  localStorage.setItem(CFG_KEY, JSON.stringify({ ...readConfig(), ...patch }));
}
