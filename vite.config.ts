import { defineConfig, type Plugin } from "vite";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";
// @ts-expect-error type error without @types/node package
import process from "node:process";

const root = path.dirname(fileURLToPath(import.meta.url));
const vendor = (f: string) => path.join(root, "src", "vendor", f);
const host = process.env.TAURI_DEV_HOST;

// 生产构建：index.html 的 import map 指向 /src/vendor/*.js（与 host alias 同一 URL，
// 保证单实例 React）。打包态 Vite 不会把该目录当作公开资源，需原样复制到 dist 同名路径，
// 否则发布版 App 的裸导入 react 会 404。src/jsx 下的 JSX 运行时 shim 同样按 URL 引用，一并复制。
function copyVendor(): Plugin {
  return {
    name: "ava2:copy-vendor",
    apply: "build",
    closeBundle() {
      for (const dir of ["vendor", "jsx"]) {
        const from = path.join(root, "src", dir);
        const to = path.join(root, "dist", "src", dir);
        if (!fs.existsSync(from)) continue;
        fs.rmSync(to, { recursive: true, force: true });
        fs.cpSync(from, to, { recursive: true });
      }
    },
  };
}

// 关键：host 自身也 alias 到 vendor ESM，与运行时 import 的 App bundle 共享同一个 React 实例，
// 否则出现双 React → hooks 崩溃。这是"import map 供给基线依赖"必须配套的 host 侧约束。
// 不用 @vitejs/plugin-react：其 jsx 自动运行时会触发 optimizeDeps 预打包 react 系依赖，
// 预打包 chunk 内部自带一份 react → 双实例。改用 esbuild JSX + 本地 jsxImportSource shim。
export default defineConfig({
  plugins: [copyVendor()],
  esbuild: { jsx: "automatic", jsxImportSource: "/src/jsx" },
  build: {
    rollupOptions: {
      // 关键：react 系与 vendor/jsx 一律 external，产物保留裸导入 / 绝对 URL 导入，
      // 由 index.html 的 import map 与静态 /src/vendor 目录在运行时解析。
      // 若让 rollup 内联，host 会自带一份 react，与 App bundle 共享的第二份冲突（双实例 → hooks 崩）。
      external: [/^react(\/.*)?$/, /^react-dom(\/.*)?$/, /^\/src\/(vendor|jsx)\//],
      output: {
        // esbuild 的 jsxImportSource 产出无扩展名导入（/src/jsx/jsx-runtime），
        // 静态服务不保证按扩展名兜底，构建期补全为真实文件 URL。
        paths: {
          "/src/jsx/jsx-runtime": "/src/jsx/jsx-runtime.js",
          "/src/jsx/jsx-dev-runtime": "/src/jsx/jsx-dev-runtime.js",
        },
      },
    },
  },
  resolve: {
    // 注意：alias 必须用正则锚定精确匹配。字符串对象形式是前缀匹配，
    // "react" 会吞掉 "react/jsx-runtime" 映射成不存在的子路径。
    // replacement 用文件系统路径指向 src/vendor（Vite 唯一 URL 空间 /src/vendor/...，
    // 与 index.html import map 的映射目标一致，保证 host 与 App bundle 单实例 react）。
    alias: [
      { find: /^react$/, replacement: vendor("react.js") },
      { find: /^react\/jsx-runtime$/, replacement: vendor("react-jsx-runtime.js") },
      { find: /^react\/jsx-dev-runtime$/, replacement: vendor("react-jsx-dev-runtime.js") },
      { find: /^react-dom$/, replacement: vendor("react-dom.js") },
      { find: /^react-dom\/client$/, replacement: vendor("react-dom-client.js") },
    ],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
