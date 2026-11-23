// AppIcon 统一渲染器：全壳唯一图标出口（24×24 网格，lucide 同源）。
// 尺寸阶梯与绘制约束见 docs/references/icon-spec.md §5；App 只提供形状，
// 尺寸/配色由宿主决定（currentColor 继承文字色）。
import type { AppIcon } from "./types";

/** 尺寸阶梯（px）：行内小图标 / 导航与工具按钮 / 市场网格与详情 / 启动器大卡片 */
export const ICON_SM = 14;
export const ICON_MD = 18;
export const ICON_LG = 28;
export const ICON_XL = 36;

/** AppIcon 缺省退化为占位符（与无图标场景一致） */
export function AppIconView({ icon, size = ICON_MD }: { icon?: AppIcon; size?: number }) {
  if (!icon) return <span className="app-tile-placeholder">▣</span>;
  const fill = typeof icon === "object" ? icon.fill : undefined;
  const paths = typeof icon === "string" ? icon : icon.d;
  const pathList = Array.isArray(paths) ? paths : [paths];
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke={fill ? "none" : "currentColor"}
      // 24 网格内固定值，SVG 等比缩放下视觉粗细随尺寸一致（1.8 ≈ lucide 同源观感）
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {pathList.map((p, i) => (
        <path key={i} d={p} />
      ))}
    </svg>
  );
}
