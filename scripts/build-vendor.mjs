// 产出 vendor ESM chunk（宿主"基线依赖供给"）。
// 原型实测踩坑记录（ava/prototypes/app-host-proto/scripts/build-vendor.mjs）：
// 1) react/react-dom 包入口是 CJS 且按 NODE_ENV 条件 require——esbuild 对 CJS entry
//    只产出 default（无静态命名导出）；用 ESM wrapper 也没用。解法：用 Node require 枚举
//    真实导出名，生成"显式命名导出"的 wrapper（本脚本每次构建时自动生成）。
// 2) 多入口分别构建 + external ["react"] 会生成运行时 __require shim → 浏览器报
//    "Dynamic require of react is not supported"。解法：单次构建 + splitting:true、零 external。
// 3) react 19 exports map 封锁 ./cjs/* 子路径，wrapper 里用相对文件路径直指 cjs。
import { build } from "esbuild";
import { createRequire } from "node:module";
import { writeFileSync, mkdirSync, rmSync } from "node:fs";

const req = createRequire(import.meta.url);

/** 用 Node require 枚举包的真实导出名，生成显式命名导出的 ESM wrapper */
function genWrapper({ cjsRel, pkg, out }) {
  const names = Object.keys(req(pkg)).filter((k) => k !== "default" && /^[A-Za-z_$][\w$]*$/.test(k));
  const body = [`import * as M from "${cjsRel}";`, `export default M;`, ...names.map((n) => `export const ${n} = M.${n};`)].join("\n");
  writeFileSync(out, body);
}

// wrapper 文件名 = 期望的输出 chunk 名
rmSync("vendor-src", { recursive: true, force: true });
mkdirSync("vendor-src", { recursive: true });
genWrapper({ cjsRel: "../node_modules/react/cjs/react.development.js", pkg: "react", out: "vendor-src/react.js" });
genWrapper({ cjsRel: "../node_modules/react/cjs/react-jsx-runtime.development.js", pkg: "react/jsx-runtime", out: "vendor-src/react-jsx-runtime.js" });
genWrapper({ cjsRel: "../node_modules/react/cjs/react-jsx-dev-runtime.development.js", pkg: "react/jsx-dev-runtime", out: "vendor-src/react-jsx-dev-runtime.js" });
genWrapper({ cjsRel: "../node_modules/react-dom/cjs/react-dom.development.js", pkg: "react-dom", out: "vendor-src/react-dom.js" });
genWrapper({ cjsRel: "../node_modules/react-dom/cjs/react-dom-client.development.js", pkg: "react-dom/client", out: "vendor-src/react-dom-client.js" });

// 产物必须落 src/vendor（而非 public/）：Vite 源码模块不允许 import public 目录文件，
// 且 public 目录存在 /public/... 与 /... 双 URL——同一 chunk 会被 host 与 import map
// 加载成两个模块实例（双 react → hooks 报 null.useState）。
rmSync("src/vendor", { recursive: true, force: true });

await build({
  bundle: true,
  format: "esm",
  splitting: true,
  outdir: "src/vendor",
  entryPoints: {
    react: "vendor-src/react.js",
    "react-jsx-runtime": "vendor-src/react-jsx-runtime.js",
    "react-jsx-dev-runtime": "vendor-src/react-jsx-dev-runtime.js",
    "react-dom": "vendor-src/react-dom.js",
    "react-dom-client": "vendor-src/react-dom-client.js",
  },
  jsx: "automatic",
  define: { "process.env.NODE_ENV": '"development"' },
  logLevel: "info",
});

console.log("vendor ESM -> src/vendor/*.js");
