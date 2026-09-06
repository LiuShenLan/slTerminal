// mdPreviewStyle.ts — md 预览排版 CSS 生成（配色单点收编）
//
// md 预览经 srcdoc iframe 渲染（opaque origin），无法引用宿主 CSS 变量——CSS
// 只能以字符串内联装配。原实现以色值字面量手抄 schemes/linear.ts（双轨靠纪律
// 同步，曾登记「iframe 内容色板豁免」）；2026-09-06 收编：色值一律取 active
// 方案的 editor.overrides（正文/语法/底色）与 preview 组（文档结构色）现场拼装，
// 改主题即跟随——颜色定义单点 = schemes/<scheme>.ts（硬约束 #6）。
//
// 生成函数每次调用取 schemeRegistry.getActive() 现拼（拼装为微秒级模板，渲染
// 热路径无需缓存）——比 CM editorTheme 常量（模块快照）更活：方案切换后下次
// 预览重建即用新值。
//
// 视觉门槛：值 = 收编前字面量原样（唯一变化 = body 底色修复为深底 + 来源单点化）。

import { schemeRegistry } from "../../theme";

/**
 * 装配 md 预览完整排版 CSS（排版布局段 + hljs 语法高亮段，供 buildPreviewDocument
 * 内联进 <style>）。色值全部来自 active 方案：正文/底色 = editor.overrides
 * （plainText/background）、代码语法 = syntax 9 键、结构色 = preview 组、
 * 复选框强调 = ui.focusBorder、mermaid 错误文案 = ui.errorFg。
 */
export function buildMdPreviewStyleCss(): string {
  const { ui, editor } = schemeRegistry.getActive();
  const { overrides } = editor;
  const s = overrides.syntax;
  const p = overrides.preview;

  // ── 排版布局（正文/标题/表格/引用/代码块/任务列表/分割线/滚动条）──
  const layout = `
html, body { margin: 0; padding: 0; }
body {
  color: ${overrides.plainText};
  background: ${overrides.background};
  font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
  font-size: 14px;
  line-height: 1.7;
  padding: 20px 24px 40px;
  word-wrap: break-word;
}
h1, h2, h3, h4, h5, h6 {
  color: ${p.heading};
  font-weight: 600;
  line-height: 1.35;
  margin: 1.4em 0 0.6em;
}
h1 { font-size: 1.7em; padding-bottom: 0.3em; border-bottom: 1px solid ${p.borderLow}; }
h2 { font-size: 1.4em; padding-bottom: 0.25em; border-bottom: 1px solid ${p.borderLow}; }
h3 { font-size: 1.2em; }
h4, h5, h6 { font-size: 1.05em; }
p { margin: 0.6em 0; }
a { color: ${p.link}; text-decoration: none; }
a:hover { text-decoration: underline; }
img { max-width: 100%; }
code {
  font-family: "JetBrains Mono", "Cascadia Mono", Consolas, monospace;
  font-size: 0.9em;
}
:not(pre) > code {
  background: ${p.inlineCodeBg};
  border-radius: 3px;
  padding: 0.1em 0.35em;
  color: ${p.codeText};
}
pre {
  background: ${p.codeBlockBg};
  border: 1px solid ${p.borderLow};
  border-radius: 6px;
  padding: 12px 14px;
  overflow-x: auto;
  line-height: 1.5;
}
pre code { background: none; padding: 0; color: ${p.codeText}; }
blockquote {
  margin: 0.8em 0;
  padding: 0.1em 1em;
  border-left: 3px solid ${p.quoteBorder};
  color: ${p.quoteText};
}
blockquote p { margin: 0.4em 0; }
hr { border: none; border-top: 1px solid ${p.borderLow}; margin: 1.6em 0; }
table { border-collapse: collapse; margin: 0.8em 0; }
th, td { border: 1px solid ${p.tableBorder}; padding: 6px 12px; }
th { background: ${p.tableHeaderBg}; color: ${p.heading}; }
tr:nth-child(2n) td { background: ${p.zebraBg}; }
ul, ol { padding-left: 1.6em; }
li { margin: 0.25em 0; }
li.task-list-item { list-style: none; margin-left: -1.2em; }
li.task-list-item input[type="checkbox"] { margin-right: 0.4em; accent-color: ${ui.focusBorder}; }
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: ${p.scrollbarThumb}; border-radius: 5px; }
::-webkit-scrollbar-thumb:hover { background: ${p.scrollbarThumbHover}; }
.slterm-mermaid-error { color: ${ui.errorFg}; font-size: 0.9em; }
.slterm-mermaid-error pre { color: ${p.codeText}; }
`;

  // ── 语法高亮主题（hljs 12 类 → editor.overrides.syntax 9 键映射；deletion/emphasis/strong 无 syntax 槽位者走 preview/ui）──
  const hljs = `
.hljs-comment, .hljs-quote { color: ${s.comment}; font-style: italic; }
.hljs-keyword, .hljs-selector-tag, .hljs-literal { color: ${s.keyword}; }
.hljs-string, .hljs-regexp, .hljs-addition { color: ${s.string}; }
.hljs-number, .hljs-symbol, .hljs-bullet, .hljs-link { color: ${s.number}; }
.hljs-title, .hljs-title.function_, .hljs-section, .hljs-name { color: ${s.function}; }
.hljs-type, .hljs-class .hljs-title, .hljs-built_in, .hljs-title.class_ { color: ${s.type}; }
.hljs-attr, .hljs-attribute, .hljs-variable, .hljs-property { color: ${s.property}; }
.hljs-operator, .hljs-punctuation { color: ${s.operator}; }
.hljs-meta, .hljs-selector-id, .hljs-selector-class { color: ${s.function}; }
.hljs-deletion { color: ${p.deletion}; }
.hljs-emphasis { font-style: italic; }
.hljs-strong { font-weight: 600; }
`;

  return `${layout}${hljs}`;
}
