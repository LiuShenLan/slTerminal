// gen-katex-inline.mjs — KaTeX 字体 CSS 内联生成脚本（构建时一次性）
//
// 背景（md 预览 iframe opaque origin）：KaTeX 数学输出引用自定义字体族
// （KaTeX_Main/AMS/...），字体文件若以相对 url(fonts/...) 外链，srcdoc iframe
// 内无法解析（opaque origin 无相对资产路径）；iframe 继承宿主 CSP（font-src
// 'self' data:），asset 协议跨源字体 CORS 行为未实证。故构建期把 katex.min.css
// 内全部 woff2 url 替换为 data:font/woff2;base64, 内联，产物随源码提交
// （~0.9MB 文本），katex 升级后重跑本脚本 + git diff 审阅。
//
// 用法：node scripts/gen-katex-inline.mjs
// 产物：src/panels/markdown/generated/katexInlineCss.ts（导出 KATEX_INLINE_CSS）

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const cssPath = resolve(root, "node_modules/katex/dist/katex.min.css");
const outDir = resolve(root, "src/panels/markdown/generated");
const outPath = resolve(outDir, "katexInlineCss.ts");

const css = readFileSync(cssPath, "utf8");

// 每 @font-face 声明三种格式（woff2/woff/ttf）。woff2 全覆盖现代 WebView2，
// woff/ttf 仅为旧浏览器回退——iframe 内无法外链字体（见文件头），回退分支
// 直接剔除（浏览器使用已内联的 data woff2）：
//   url(fonts/KaTeX_*.woff2) → url(data:font/woff2;base64,...)
//   url(fonts/KaTeX_*.woff) format("woff") / .ttf → 删除
let replaced = css.replace(/url\(fonts\/(KaTeX_[^)]+\.woff2)\)/g, (_m, name) => {
  const fontPath = resolve(root, `node_modules/katex/dist/fonts/${name}`);
  const data = readFileSync(fontPath).toString("base64");
  return `url(data:font/woff2;base64,${data})`;
});
replaced = replaced.replace(/url\(fonts\/[^)]+\.woff\) format\("woff"\),?/g, "");
replaced = replaced.replace(/url\(fonts\/[^)]+\.ttf\) format\("truetype"\),?/g, "");

const remaining = replaced.match(/url\(fonts\//g)?.length ?? 0;
if (remaining > 0) {
  console.error(`[gen-katex] 仍有未内联字体引用 ${remaining} 处——katex 目录结构变化？`);
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });
const ts = `// katexInlineCss.ts — 构建产物（scripts/gen-katex-inline.mjs 生成，勿手改）
//
// katex.min.css 的 woff2 字体 url 已全部内联为 data:font/woff2;base64（srcdoc
// iframe opaque origin 下无法外链相对字体）。katex 升级后重跑生成脚本并审阅
// git diff（ADR-0018 配套）。

export const KATEX_INLINE_CSS = ${JSON.stringify(replaced)};
`;
writeFileSync(outPath, ts, "utf8");

const kb = Math.round(Buffer.byteLength(replaced) / 1024);
console.log(`[gen-katex] 产物写入 ${outPath}（CSS ${kb}KB）`);
