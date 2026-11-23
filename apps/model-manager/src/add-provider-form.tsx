// 添加供应商表单：id / 显示名 / baseUrl / apiKey，三者必填方可保存。
import type { AddProviderInput } from "@ava-ai/app-sdk";
import { Field } from "./field";
import { BTN, BTN_PRIMARY, CARD } from "./styles";
import type { FormState, ProviderFormState } from "./types";

interface AddProviderFormProps {
  fields: ProviderFormState;
  onChange: (next: FormState | null) => void;
  onSubmit: (input: AddProviderInput) => void;
}

export function AddProviderForm({ fields, onChange, onSubmit }: AddProviderFormProps) {
  const set = (patch: Partial<ProviderFormState>) => onChange({ ...fields, ...patch });
  const valid = Boolean(fields.id && fields.baseUrl && fields.apiKey);
  return (
    <div style={{ ...CARD, display: "flex", flexDirection: "column", gap: "var(--ava-space-3, 10px)" }}>
      <b style={{ fontSize: 13 }}>添加供应商</b>
      <div style={{ display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }}>
        <Field label="ID" value={fields.id} onChange={(v) => set({ id: v })} placeholder="如 openai" />
        <Field label="显示名（可选）" value={fields.name} onChange={(v) => set({ name: v })} />
        <Field
          label="Base URL"
          value={fields.baseUrl}
          onChange={(v) => set({ baseUrl: v })}
          placeholder="https://api.example.com/v1"
        />
        <Field label="API Key" value={fields.apiKey} onChange={(v) => set({ apiKey: v })} placeholder="sk-…" password />
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: "var(--ava-space-2, 8px)" }}>
        <button style={BTN} onClick={() => onChange(null)}>
          取消
        </button>
        <button
          style={BTN_PRIMARY}
          disabled={!valid}
          onClick={() =>
            onSubmit({
              id: fields.id,
              ...(fields.name ? { name: fields.name } : {}),
              baseUrl: fields.baseUrl,
              apiKey: fields.apiKey,
            })
          }
        >
          保存
        </button>
      </div>
    </div>
  );
}
