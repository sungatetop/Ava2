// 添加模型表单：已有供应商单选 / 新建供应商双路（对齐 ava-app AddModelModal）。
// 新建供应商时顺带填 baseUrl + apiKey（后端按 AddModelInput 一并创建供应商）。
import type { AddModelInput } from "@ava-ai/app-sdk";
import { Field } from "./field";
import { TestBadge } from "./test-badge";
import { BTN, BTN_PRIMARY, CARD } from "./styles";
import type { FormState, ModelFormState, TestState } from "./types";

interface AddModelFormProps {
  /** 已有供应商 id 列表（内容路由按钮） */
  providers: string[];
  fields: ModelFormState;
  onChange: (next: FormState | null) => void;
  onSubmit: (input: AddModelInput) => void;
  onTest: (fields: ModelFormState) => void;
  testState?: TestState;
}

export function AddModelForm({ providers, fields, onChange, onSubmit, onTest, testState }: AddModelFormProps) {
  const set = (patch: Partial<ModelFormState>) => onChange({ ...fields, ...patch });
  const isNew = fields.newProvider;
  const canTest = Boolean(isNew ? fields.baseUrl && fields.apiKey && fields.id : fields.id);
  const valid = Boolean(isNew ? fields.provider && fields.baseUrl && fields.apiKey : fields.provider) && Boolean(fields.id);

  return (
    <div style={{ ...CARD, display: "flex", flexDirection: "column", gap: "var(--ava-space-3, 10px)" }}>
      <b style={{ fontSize: 13 }}>添加模型</b>
      {/* 供应商路由选择：已有供应商直选，或走新建分支 */}
      <div style={{ display: "flex", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }}>
        {providers.map((id) => (
          <button
            key={id}
            style={{
              ...BTN,
              ...(fields.provider === id && !isNew
                ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
                : {}),
            }}
            onClick={() => set({ provider: id, newProvider: false })}
          >
            {id}
          </button>
        ))}
        <button
          style={{
            ...BTN,
            ...(isNew ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}),
          }}
          onClick={() => set({ provider: "", newProvider: true })}
        >
          新建供应商…
        </button>
      </div>
      <div style={{ display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }}>
        {isNew ? (
          <Field label="供应商 ID" value={fields.provider} onChange={(v) => set({ provider: v })} placeholder="如 my-provider" />
        ) : null}
        {isNew ? (
          <Field
            label="Base URL"
            value={fields.baseUrl}
            onChange={(v) => set({ baseUrl: v })}
            placeholder="https://api.example.com/v1"
          />
        ) : null}
        {isNew ? (
          <Field label="API Key" value={fields.apiKey} onChange={(v) => set({ apiKey: v })} placeholder="sk-…" password />
        ) : null}
        <Field label="模型 ID" value={fields.id} onChange={(v) => set({ id: v })} placeholder="provider 侧模型名，如 gpt-4o-mini" />
        <Field label="显示名（可选）" value={fields.name} onChange={(v) => set({ name: v })} />
        <Field label="描述（可选）" value={fields.description} onChange={(v) => set({ description: v })} />
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)" }}>
        <button style={BTN} disabled={!canTest} onClick={() => onTest(fields)}>
          测试连接
        </button>
        <TestBadge state={testState} />
        <span style={{ flex: 1 }} />
        <button style={BTN} onClick={() => onChange(null)}>
          取消
        </button>
        <button
          style={BTN_PRIMARY}
          disabled={!valid}
          onClick={() =>
            onSubmit({
              provider: fields.provider,
              id: fields.id,
              ...(fields.name ? { name: fields.name } : {}),
              ...(fields.description ? { description: fields.description } : {}),
              ...(isNew ? { baseUrl: fields.baseUrl, apiKey: fields.apiKey } : {}),
            })
          }
        >
          保存
        </button>
      </div>
    </div>
  );
}
