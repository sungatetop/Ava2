// 模型管理主视图：供应商分组卡片 + 模型表格。
// 能力面：设默认模型、连接测试（目录解析路径，密钥不出边车）、删除（气泡确认可关）、
// 添加模型（含新建供应商）/ 添加供应商。
import { useCallback, useEffect, useState } from "react";
import type { AddModelInput, AddProviderInput, AgentChannel, ProviderInfo, TestModelResult } from "@ava-ai/app-sdk";
import { readConfig } from "./config";
import { AddModelForm } from "./add-model-form";
import { AddProviderForm } from "./add-provider-form";
import { Popconfirm } from "./popconfirm";
import { TestBadge } from "./test-badge";
import { BTN, BTN_DANGER, BTN_PRIMARY, CHIP } from "./styles";
import type { FormState, ModelCatalog, ModelFormState, TestState } from "./types";

interface ModelsViewProps {
  agent: AgentChannel;
}

export function ModelsView({ agent }: ModelsViewProps) {
  const [data, setData] = useState<ModelCatalog | null>(null);
  const [err, setErr] = useState("");
  const [note, setNote] = useState("");
  const [confirmKey, setConfirmKey] = useState("");
  const [form, setForm] = useState<FormState | null>(null);
  const [tests, setTests] = useState<Record<string, TestState>>({});

  const load = useCallback(() => {
    agent
      .listModels()
      .then((r) => setData({ defaultModel: r.defaultModel || "", providers: Array.isArray(r.providers) ? r.providers : [] }))
      .catch((e: unknown) => setErr(e instanceof Error ? e.message : String(e)));
  }, [agent]);

  useEffect(() => {
    load();
  }, [load]);

  const run = async (fn: () => Promise<unknown>, okNote?: string): Promise<boolean> => {
    setErr("");
    setNote("");
    try {
      await fn();
      if (okNote) setNote(okNote);
      load();
      return true;
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
      return false;
    }
  };

  const setDefault = (model: string) => {
    void run(() => agent.setDefaultModel(model), `默认模型已切换为 ${model}`);
  };

  const removeModel = (provider: string, model: string) => {
    void run(() => agent.removeModel(provider, model), `已删除模型 ${model}`);
  };

  const removeProvider = (id: string) => {
    void run(() => agent.removeProvider(id), `已删除供应商 ${id}`);
  };

  /** 表格行测试：目录解析路径（provider + model，密钥留在边车） */
  const testRow = async (provider: string, model: string) => {
    const key = `${provider}/${model}`;
    setTests((t) => ({ ...t, [key]: { loading: true } }));
    try {
      const result = await agent.testModel({ provider, model });
      setTests((t) => ({ ...t, [key]: { result } }));
    } catch (e) {
      const result: TestModelResult = { ok: false, latencyMs: 0, error: e instanceof Error ? e.message : String(e) };
      setTests((t) => ({ ...t, [key]: { result } }));
    }
  };

  /** 表单内测试：未保存走直连路径（baseUrl + apiKey + model） */
  const testForm = async (fields: ModelFormState) => {
    setTests((t) => ({ ...t, __form: { loading: true } }));
    try {
      const result = await agent.testModel({ baseUrl: fields.baseUrl, apiKey: fields.apiKey, model: fields.id });
      setTests((t) => ({ ...t, __form: { result } }));
    } catch (e) {
      const result: TestModelResult = { ok: false, latencyMs: 0, error: e instanceof Error ? e.message : String(e) };
      setTests((t) => ({ ...t, __form: { result } }));
    }
  };

  const submitModel = async (input: AddModelInput) => {
    const ok = await run(() => agent.addModel(input), `已添加模型 ${input.id}`);
    if (ok) setForm(null);
  };

  const submitProvider = async (input: AddProviderInput) => {
    const ok = await run(() => agent.addProvider(input), `已添加供应商 ${input.id}`);
    if (ok) setForm(null);
  };

  /** 删除守卫：设置面板可关；配置每次点击实时读取（面板改动即时生效） */
  const deleteGuard = (key: string, action: () => void) => {
    if (readConfig().confirmDelete) setConfirmKey(key);
    else action();
  };

  if (err && !data) {
    return (
      <div style={{ padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }}>
        加载失败：{err}
        <button style={{ ...BTN, marginLeft: 12 }} onClick={load}>
          重试
        </button>
      </div>
    );
  }
  if (!data) {
    return <div style={{ padding: "var(--ava-space-5, 24px)", color: "var(--ava-label-tertiary)", fontSize: 13 }}>加载中…</div>;
  }

  return (
    <div
      style={{
        padding: "var(--ava-space-5, 24px)",
        overflow: "auto",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-4, 16px)",
      }}
    >
      {/* 头部：标题 + 默认模型 + 添加入口 */}
      <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)" }}>
        <h2 style={{ margin: 0, fontSize: 16 }}>模型管理</h2>
        <span
          title="新会话的初始模型（PUT /v1/models/default 持久化；已有会话在会话内切换）"
          style={CHIP()}
        >
          默认模型：
          <b style={{ fontFamily: "var(--ava-font-mono, monospace)", fontWeight: 500, marginLeft: 4 }}>
            {data.defaultModel || "（未配置）"}
          </b>
        </span>
        <span style={{ flex: 1 }} />
        <button
          style={BTN}
          onClick={() => setForm(form?.kind === "provider" ? null : { kind: "provider", id: "", name: "", baseUrl: "", apiKey: "" })}
        >
          + 添加供应商
        </button>
        <button
          style={BTN_PRIMARY}
          onClick={() =>
            setForm(
              form?.kind === "model"
                ? null
                : { kind: "model", provider: "", newProvider: false, id: "", name: "", description: "", baseUrl: "", apiKey: "" },
            )
          }
        >
          + 添加模型
        </button>
      </div>

      {err || note ? (
        <div style={{ fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }}>{err || note}</div>
      ) : null}

      {data.providers.map((provider) => (
        <ProviderCard
          key={provider.id}
          provider={provider}
          defaultModel={data.defaultModel}
          confirmKey={confirmKey}
          tests={tests}
          onSetDefault={setDefault}
          onRemoveModel={removeModel}
          onRemoveProvider={removeProvider}
          onTestRow={testRow}
          onConfirmCancel={() => setConfirmKey("")}
          deleteGuard={deleteGuard}
        />
      ))}

      {data.providers.length === 0 ? (
        <div style={{ padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }}>
          尚无供应商——右上角「+ 添加供应商」或「+ 添加模型」开始配置。
        </div>
      ) : null}

      {form?.kind === "model" ? (
        <AddModelForm
          providers={data.providers.map((p) => p.id)}
          fields={form}
          onChange={setForm}
          onSubmit={submitModel}
          onTest={testForm}
          testState={tests.__form}
        />
      ) : null}

      {form?.kind === "provider" ? (
        <AddProviderForm fields={form} onChange={setForm} onSubmit={submitProvider} />
      ) : null}
    </div>
  );
}

interface ProviderCardProps {
  provider: ProviderInfo;
  defaultModel: string;
  confirmKey: string;
  tests: Record<string, TestState>;
  onSetDefault: (model: string) => void;
  onRemoveModel: (provider: string, model: string) => void;
  onRemoveProvider: (id: string) => void;
  onTestRow: (provider: string, model: string) => void;
  onConfirmCancel: () => void;
  deleteGuard: (key: string, action: () => void) => void;
}

/** 供应商分组卡片：头部（名称/baseUrl/删除）+ 模型表格 */
function ProviderCard({
  provider,
  defaultModel,
  confirmKey,
  tests,
  onSetDefault,
  onRemoveModel,
  onRemoveProvider,
  onTestRow,
  onConfirmCancel,
  deleteGuard,
}: ProviderCardProps) {
  const providerKey = `provider:${provider.id}`;
  const title = provider.name || provider.id;
  return (
    <div
      style={{
        border: "0.5px solid var(--ava-border-l2)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)", marginBottom: "var(--ava-space-3, 10px)" }}>
        <b style={{ fontSize: 13 }}>{title}</b>
        <span style={{ fontSize: 11, color: "var(--ava-label-tertiary)", fontFamily: "var(--ava-font-mono, monospace)" }}>
          {provider.baseUrl}
        </span>
        <span style={{ flex: 1 }} />
        <span style={{ position: "relative" }}>
          <button style={BTN_DANGER} onClick={() => deleteGuard(providerKey, () => onRemoveProvider(provider.id))}>
            删除供应商
          </button>
          {confirmKey === providerKey ? (
            <Popconfirm
              title={`删除供应商「${title}」？组内需无模型方可删除。`}
              onCancel={onConfirmCancel}
              onConfirm={() => onRemoveProvider(provider.id)}
            />
          ) : null}
        </span>
      </div>
      <table style={{ borderCollapse: "collapse", width: "100%", fontSize: 12 }}>
        <tbody>
          {provider.models.map((model) => {
            const isDefault = model.id === defaultModel;
            const rowKey = `${provider.id}/${model.id}`;
            const modelKey = `model:${rowKey}`;
            const displayName = model.name && model.name !== model.id ? model.name : "";
            return (
              <tr key={model.id}>
                <td style={{ padding: "5px 8px 5px 0", fontFamily: "var(--ava-font-mono, monospace)" }}>{model.id}</td>
                <td style={{ padding: "5px 8px", color: "var(--ava-label-secondary)" }}>{displayName}</td>
                <td style={{ padding: "5px 8px", color: "var(--ava-label-tertiary)" }}>{model.description ?? ""}</td>
                <td style={{ padding: "5px 8px", textAlign: "right" }}>
                  {isDefault ? (
                    <span style={CHIP()}>默认</span>
                  ) : (
                    <button style={BTN} onClick={() => onSetDefault(model.id)}>
                      设默认
                    </button>
                  )}
                </td>
                <td style={{ padding: "5px 8px", textAlign: "right" }}>
                  <TestBadge state={tests[rowKey]} />
                </td>
                <td style={{ padding: "5px 0 5px 8px", textAlign: "right", whiteSpace: "nowrap" }}>
                  <button style={{ ...BTN, marginRight: 6 }} onClick={() => onTestRow(provider.id, model.id)}>
                    测试
                  </button>
                  <span style={{ position: "relative" }}>
                    <button style={BTN_DANGER} onClick={() => deleteGuard(modelKey, () => onRemoveModel(provider.id, model.id))}>
                      删除
                    </button>
                    {confirmKey === modelKey ? (
                      <Popconfirm
                        title={`删除模型「${model.id}」？`}
                        onCancel={onConfirmCancel}
                        onConfirm={() => onRemoveModel(provider.id, model.id)}
                      />
                    ) : null}
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
