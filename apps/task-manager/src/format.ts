// 时间格式化：RFC3339 → 本地展示；datetime-local → 本地时区 RFC3339。

/** 本地 IANA 时区（daily/cron 频率注出用） */
export const TZ = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";

/** RFC3339 → `YYYY-MM-DD HH:mm` 本地展示（等宽渲染保持对齐）；非法值原样回显 */
export function fmtWhen(rfc3339: string): string {
  if (!rfc3339) return "—";
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return rfc3339;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** datetime-local 值 → 本地时区 RFC3339（浏览器本地偏移）；非法值返回空串 */
export function localToRfc3339(datetimeLocal: string): string {
  const date = new Date(datetimeLocal);
  if (Number.isNaN(date.getTime())) return "";
  const tzo = -date.getTimezoneOffset();
  const sign = tzo >= 0 ? "+" : "-";
  const abs = Math.abs(tzo);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${datetimeLocal.length === 16 ? `${datetimeLocal}:00` : datetimeLocal}${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

/** 频率类型 → 中文标签（kind 覆盖边车全部 FrequencySpec 变体） */
export const KIND_LABEL: Record<string, string> = {
  at: "一次性",
  after: "延时",
  every: "间隔",
  daily: "每天",
  weekly: "每周",
  cron: "Cron",
};
