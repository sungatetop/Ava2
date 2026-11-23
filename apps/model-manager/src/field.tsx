// 表单字段（标签 + 单行输入，password 走 type=password 遮蔽密钥）。
import { FIELD, INPUT, LABEL } from "./styles";

interface FieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  password?: boolean;
}

export function Field({ label, value, onChange, placeholder, password }: FieldProps) {
  return (
    <label style={FIELD}>
      <span style={LABEL}>{label}</span>
      <input
        style={INPUT}
        value={value}
        placeholder={placeholder}
        type={password ? "password" : "text"}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}
