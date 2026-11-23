// 视图本地类型：表单态与测试态是 App 私有状态，不进宿主 SDK 契约。
import type { ProviderInfo, TestModelResult } from "@ava-ai/app-sdk";

/** 模型目录快照（GET /v1/models 投影：默认模型 + 供应商分组） */
export interface ModelCatalog {
  defaultModel: string;
  providers: ProviderInfo[];
}

/** 添加模型表单态：newProvider 为真时 provider 字段充当新供应商 ID */
export interface ModelFormState {
  kind: "model";
  provider: string;
  newProvider: boolean;
  id: string;
  name: string;
  description: string;
  baseUrl: string;
  apiKey: string;
}

/** 添加供应商表单态 */
export interface ProviderFormState {
  kind: "provider";
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
}

export type FormState = ModelFormState | ProviderFormState;

/** 连接测试 UI 态（loading 与 result 单态） */
export interface TestState {
  loading?: boolean;
  result?: TestModelResult;
}
