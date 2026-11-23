// butler 构建脚本：esbuild 单包打包 → dist/（即安装源目录）。
// 关键不变式：react 系一律 external，运行时由宿主 import map 供给（单实例 React）。
import * as esbuild from "esbuild";
import { copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));

mkdirSync(path.join(root, "dist"), { recursive: true });
await esbuild.build({
  entryPoints: [path.join(root, "src/main.tsx")],
  bundle: true,
  format: "esm",
  target: "es2022",
  jsx: "automatic",
  outfile: path.join(root, "dist/main.js"),
  external: ["react", "react-dom", "react/jsx-runtime", "react/jsx-dev-runtime", "react-dom/client"],
  logLevel: "info",
});
copyFileSync(path.join(root, "manifest.json"), path.join(root, "dist/manifest.json"));
console.log("butler build ok -> dist/ (main.js + manifest.json)");
