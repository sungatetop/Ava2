// 画布文件预览视图：文件路径来自总线（present 交付物 / 工具结果 / 文件树双击），
// 内容经 ctx.agent.readWorkspaceFile（GET /v1/workspace/file）读取，只读渲染。
// 多文件同时打开：每个路径一个 tab（tab 显示文件名，可切换/关闭），对齐编辑器式多标签预览。
// 按文件类型分支：图片 data URL、Markdown 富文本、HTML iframe（sandbox allow-scripts，opaque origin）、
// Office 文档（docx→mammoth、xlsx→SheetJS、pptx→jszip 提取文本）、其余等宽文本。
import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
// mammoth 浏览器 bundle（主入口含 node 依赖，browser 子路径为自包含 ESM）
// @ts-ignore - mammoth 主包类型不含 browser 子路径声明
import mammoth from "mammoth/mammoth.browser";
import * as XLSX from "xlsx";
import JSZip from "jszip";
import type { WorkspaceFile } from "@ava-ai/app-sdk";
import { getCanvasAgent, useCanvasTarget, type CanvasTarget } from "./canvas-store";
import { fileViewerCss } from "./styles";

/** 路径末段（标题显示） */
function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

/** 扩展名判定（小写，无扩展返回 ""） */
function extOf(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  const name = parts.length > 0 ? parts[parts.length - 1] : path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** Markdown 家族扩展名 → 富文本渲染 */
function isMarkdown(path: string): boolean {
  const ext = extOf(path);
  return ext === "md" || ext === "markdown" || ext === "mdx";
}

/** HTML 家族扩展名 → iframe sandbox 渲染 */
function isHtml(path: string): boolean {
  const ext = extOf(path);
  return ext === "html" || ext === "htm";
}

/** Office OpenXML 扩展名（服务端按原始字节返回） */
function isOffice(path: string): boolean {
  const ext = extOf(path);
  return ext === "docx" || ext === "xlsx" || ext === "pptx";
}

/** HTML 预览基础样式：iframe 是独立文档，宿主的全局滚动条皮肤与令牌不生效，需注入。
    去默认边距、媒体/表格/代码块不横向溢出、滚动条为 5px 全圆角、随宿主主题。 */
function previewBaseCss(): string {
  const dark = document.documentElement.dataset.theme === "dark";
  const thumb = dark ? "#45474d" : "#dde0e5";
  const thumbHover = dark ? "#585b62" : "#c8cdd4";
  return `
    html, body { margin: 0; }
    body { overflow-wrap: break-word; }
    img, video, canvas, svg, table { max-width: 100%; }
    pre { white-space: pre-wrap; word-break: break-word; }
    ::-webkit-scrollbar { width: 5px; height: 5px; }
    ::-webkit-scrollbar-track { background: transparent; }
    ::-webkit-scrollbar-thumb { border-radius: 999px; background: ${thumb}; }
    ::-webkit-scrollbar-thumb:hover { background: ${thumbHover}; }
    ::-webkit-scrollbar-corner { background: transparent; }
  `;
}

/** HTML 预览装载（对齐 ava-app canvas 的 HtmlContent 做法）：
    片段一律包成 `<!doctype html><html><body>…</body></html>` 标准模式文档（quirks 模式是布局溢出的常见根源）；
    完整文档则在 <head> 开头注入基础样式（正则要求 head/html 后紧跟空白或闭合尖括号，避免误匹配 <header>）。 */
function withPreviewBase(html: string): string {
  const style = `<style>${previewBaseCss()}</style>`;
  if (/<html[\s>]/i.test(html)) {
    return /<head(?=[\s>])[^>]*>/i.test(html)
      ? html.replace(/<head(?=[\s>])[^>]*>/i, (m) => `${m}${style}`)
      : html.replace(/<html(?=[\s>])[^>]*>/i, (m) => `${m}<head>${style}</head>`);
  }
  return `<!doctype html><html><head>${style}</head><body>${html}</body></html>`;
}

/** 视频家族：mime video/* 或常见视频扩展名（mime_guess 兜底，如 mkv/avi/mov） */
function isVideo(path: string, mime?: string): boolean {
  return (
    !!mime?.startsWith("video/") ||
    ["mp4", "webm", "ogv", "mov", "m4v", "mkv", "avi"].includes(extOf(path))
  );
}

/** 音频家族：mime audio/* 或常见音频扩展名 */
function isAudio(path: string, mime?: string): boolean {
  return (
    !!mime?.startsWith("audio/") ||
    ["mp3", "wav", "ogg", "oga", "m4a", "flac", "aac"].includes(extOf(path))
  );
}

/** PDF：mime application/pdf 或 .pdf 扩展名 */
function isPdf(path: string, mime?: string): boolean {
  return mime === "application/pdf" || extOf(path) === "pdf";
}

/** pptx 幻灯片文本：按 slide 序号排序，逐张提取 <a:t> 文本 */
async function pptxSlides(buffer: ArrayBuffer): Promise<string[]> {
  const zip = await JSZip.loadAsync(buffer);
  const names = Object.keys(zip.files)
    .filter((name) => /^ppt\/slides\/slide\d+\.xml$/.test(name))
    .sort((a, b) => {
      const n = (s: string) => Number(s.match(/slide(\d+)/)?.[1] ?? 0);
      return n(a) - n(b);
    });
  const slides: string[] = [];
  for (const name of names) {
    const xml = await zip.files[name].async("string");
    const lines = [...xml.matchAll(/<a:t>([\s\S]*?)<\/a:t>/g)]
      .map((m) => m[1].replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").trim())
      .filter(Boolean);
    slides.push(lines.join("\n"));
  }
  return slides;
}

/** Office 文档解析结果 */
interface ParsedOffice {
  kind: "docx" | "xlsx" | "pptx";
  html?: string;
  slides?: string[];
}

/** 已打开文件 tab：path 去重，sessionId 随首开保留 */
interface OpenTab extends CanvasTarget {
  key: string;
}

export function CanvasView() {
  const target = useCanvasTarget();
  const agent = getCanvasAgent();
  /** 多标签状态：tabs 有序列表 + active 下标；总线事件 → 追加或激活 */
  const [state, setState] = useState<{ tabs: OpenTab[]; active: number }>({ tabs: [], active: 0 });
  const [file, setFile] = useState<WorkspaceFile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  /** 刷新触发（同一路径重读） */
  const [nonce, setNonce] = useState(0);
  /** Office 文档解析结果（docx/xlsx/pptx） */
  const [office, setOffice] = useState<ParsedOffice | null>(null);
  const [officeError, setOfficeError] = useState<string | null>(null);
  /** 媒体 Blob URL（视频/音频/PDF）：buffer → Blob，对齐 ava-app「媒体禁走 base64」 */
  const [mediaUrl, setMediaUrl] = useState<string | null>(null);
  /** 播放失败的 URL：video onError 后按当前 URL 判定「不支持格式」 */
  const [failedUrl, setFailedUrl] = useState<string | null>(null);

  // 总线目标 → tab：已打开则激活，否则追加为新 tab
  useEffect(() => {
    if (!target) return;
    setState((cur) => {
      const idx = cur.tabs.findIndex((t) => t.path === target.path);
      if (idx >= 0) return { ...cur, active: idx };
      return {
        tabs: [...cur.tabs, { key: target.path, path: target.path, sessionId: target.sessionId }],
        active: cur.tabs.length,
      };
    });
  }, [target]);

  const activeTab = state.tabs[state.active] ?? null;
  const path = activeTab?.path ?? "";
  const sessionId = activeTab?.sessionId;
  /** 文本行（代码预览行号渲染用；超长文件退回纯文本，避免万级行 DOM 卡顿） */
  const textLines = (file?.text ?? "").split("\n");
  /** 当前媒体 URL 播放失败 */
  const playbackFailed = mediaUrl !== null && failedUrl === mediaUrl;

  const closeTab = (i: number) =>
    setState((cur) => {
      const tabs = cur.tabs.filter((_, j) => j !== i);
      const active = cur.active === i ? Math.max(0, i - 1) : cur.active > i ? cur.active - 1 : cur.active;
      return { tabs, active: Math.min(active, Math.max(0, tabs.length - 1)) };
    });

  useEffect(() => {
    if (!path || !agent) return;
    let alive = true;
    // 切换路径立即清空旧内容，避免上一文件残留闪烁
    setFile(null);
    setOffice(null);
    setOfficeError(null);
    setLoading(true);
    setError(null);
    agent
      .readWorkspaceFile(path, sessionId)
      .then((loaded) => {
        if (alive) setFile(loaded);
      })
      .catch((e) => {
        if (!alive) return;
        setFile(null);
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [path, sessionId, agent, nonce]);

  // Office 文档解析（二进制 buffer 就绪后异步解析）
  useEffect(() => {
    const buffer = file?.buffer;
    if (!buffer || !isOffice(path)) return;
    let alive = true;
    setOffice(null);
    setOfficeError(null);
    const ext = extOf(path);
    const run = async () => {
      try {
        if (ext === "docx") {
          const result = await mammoth.convertToHtml({ arrayBuffer: buffer });
          if (alive) setOffice({ kind: "docx", html: result.value });
        } else if (ext === "xlsx") {
          const wb = XLSX.read(buffer, { type: "array" });
          const ws = wb.Sheets[wb.SheetNames[0]];
          if (alive) setOffice({ kind: "xlsx", html: ws ? XLSX.utils.sheet_to_html(ws) : "<p>（空表格）</p>" });
        } else if (ext === "pptx") {
          const slides = await pptxSlides(buffer);
          if (alive) setOffice({ kind: "pptx", slides });
        }
      } catch (e) {
        if (alive) setOfficeError(e instanceof Error ? e.message : String(e));
      }
    };
    void run();
    return () => {
      alive = false;
    };
  }, [file, path]);

  // 媒体 Blob URL：buffer → Blob（type 取响应 mime）；切换文件/卸载时 revoke 防内存泄漏
  useEffect(() => {
    if (!file?.buffer || (!isVideo(path) && !isAudio(path) && !isPdf(path))) {
      setMediaUrl(null);
      return;
    }
    const url = URL.createObjectURL(new Blob([file.buffer], { type: file.mime || "application/octet-stream" }));
    setMediaUrl(url);
    setFailedUrl(null);
    return () => URL.revokeObjectURL(url);
  }, [file, path]);

  return (
    <div className="fv-root">
      <style>{fileViewerCss}</style>
      <header className="fv-head">
        <div className="fv-tabs" role="tablist" aria-label="已打开的文件">
          {state.tabs.length === 0 && <span className="fv-tabs-empty">未打开文件</span>}
          {state.tabs.map((t, i) => (
            <div
              key={t.key}
              role="tab"
              aria-selected={i === state.active}
              className={"fv-tab" + (i === state.active ? " on" : "")}
              onClick={() => setState((cur) => ({ ...cur, active: i }))}
              title={t.path}
            >
              <span className="fv-tab-name">{fileName(t.path)}</span>
              <button
                className="fv-tab-close"
                title="关闭"
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(i);
                }}
              >
                ×
              </button>
            </div>
          ))}
        </div>
        <button
          className="fv-btn"
          onClick={() => setNonce((v) => v + 1)}
          disabled={!path || loading}
          title="重新读取文件"
        >
          {loading ? "读取中…" : "刷新"}
        </button>
      </header>
      <div className="fv-body">
        {!path ? (
          <div className="fv-empty">
            <span className="fv-empty-ico">
              <svg
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
                <path d="M14 2v4a2 2 0 0 0 2 2h4" />
              </svg>
            </span>
            <span className="fv-empty-title">等待打开文件</span>
            <span>双击文件树中的文件，或在对话里点击交付物卡片的「在画布打开」。</span>
          </div>
        ) : error ? (
          <div className="fv-error">预览失败：{error}</div>
        ) : !file ? (
          <div className="fv-empty">读取中…</div>
        ) : file.dataUrl ? (
          <div className="fv-image">
            <img src={file.dataUrl} alt={fileName(path)} />
          </div>
        ) : isMarkdown(path) ? (
          <div className="fv-markdown">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{file.text ?? ""}</ReactMarkdown>
          </div>
        ) : isHtml(path) ? (
          <iframe
            className="fv-iframe"
            sandbox="allow-scripts"
            srcDoc={withPreviewBase(file.text ?? "")}
            title={fileName(path)}
          />
        ) : isVideo(path, file.mime) ? (
          !file.buffer ? (
            <div className="fv-error">媒体文件无法读取（可能超出服务端大小上限）</div>
          ) : mediaUrl && playbackFailed ? (
            <div className="fv-error">当前浏览器不支持此视频格式</div>
          ) : mediaUrl ? (
            <div className="fv-media">
              <video
                className="fv-video"
                src={mediaUrl}
                controls
                title={fileName(path)}
                onError={() => setFailedUrl(mediaUrl)}
              />
            </div>
          ) : (
            <div className="fv-empty">媒体加载中…</div>
          )
        ) : isAudio(path, file.mime) ? (
          !file.buffer ? (
            <div className="fv-error">媒体文件无法读取（可能超出服务端大小上限）</div>
          ) : mediaUrl ? (
            <div className="fv-audio">
              <audio src={mediaUrl} controls />
            </div>
          ) : (
            <div className="fv-empty">媒体加载中…</div>
          )
        ) : isPdf(path, file.mime) ? (
          !file.buffer ? (
            <div className="fv-error">PDF 文件无法读取（可能超出服务端大小上限）</div>
          ) : mediaUrl ? (
            <iframe className="fv-iframe" src={mediaUrl} title={fileName(path)} />
          ) : (
            <div className="fv-empty">文档加载中…</div>
          )
        ) : isOffice(path) ? (
          officeError ? (
            <div className="fv-error">文档解析失败：{officeError}</div>
          ) : !office ? (
            <div className="fv-empty">文档解析中…</div>
          ) : office.kind === "pptx" ? (
            <div className="fv-pptx">
              {(office.slides ?? []).map((text, i) => (
                <section className="fv-slide" key={i}>
                  <div className="fv-slide-no">第 {i + 1} 页</div>
                  <pre className="fv-slide-text">{text || "（空白页）"}</pre>
                </section>
              ))}
            </div>
          ) : (
            <div className="fv-doc" dangerouslySetInnerHTML={{ __html: office.html ?? "" }} />
          )
        ) : textLines.length > 10000 ? (
          <pre className="fv-text">{file.text ?? ""}</pre>
        ) : (
          <div className="fv-code">
            {textLines.map((line, i) => (
              <div className="fv-code-line" key={i}>
                <span className="fv-code-no">{i + 1}</span>
                <span className="fv-code-text">{line || "\u00a0"}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
