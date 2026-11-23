// RepoAtlas 面板：侧栏「分析旅程」与画布「分级口径」说明。
import { Styles } from "./styles";

export function SidebarPanel() {
  return (
    <div className="ra-root">
      <Styles />
      <div className="ra-panel">
        <p className="ra-sec-title">分析旅程</p>
        <div className="ra-kv"><span>1 · 选仓库</span><b>浏览顶层目录</b></div>
        <div className="ra-kv"><span>2 · 定符号</span><b>函数名或类型名</b></div>
        <div className="ra-kv"><span>3 · 扫描</span><b>读取工作区源码</b></div>
        <div className="ra-kv"><span>4 · 聚合</span><b>按目录统计命中</b></div>
        <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8 }}>结果均取自仓库真实源码，单次最多扫描 80 个文件。</div>
      </div>
    </div>
  );
}

export function CanvasPanel() {
  return (
    <div className="ra-root">
      <Styles />
      <div className="ra-panel">
        <p className="ra-sec-title">分级口径</p>
        <div className="ra-kv"><span>直接</span><b>源码中直接出现该符号</b></div>
        <div className="ra-kv"><span>测试</span><b>测试文件中出现该符号</b></div>
        <div className="ra-kv"><span>波及</span><b>同目录下受牵连的文件</b></div>
        <div style={{ fontSize: 11, color: "var(--ava-label-tertiary)", marginTop: 8 }}>命中指文件内容包含该符号；回归优先级：直接 &gt; 测试 &gt; 波及。</div>
      </div>
    </div>
  );
}
