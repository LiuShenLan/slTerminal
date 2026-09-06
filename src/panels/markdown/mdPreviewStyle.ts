// mdPreviewStyle.ts — md 预览排版 CSS（内容色板单点）
//
// md 预览经 srcdoc iframe 渲染（opaque origin），无法引用宿主 theme token/
// CSS 变量——本文件以字面量承载预览排版与语法高亮主题色。色值映射 linear 方案
// 内容色（editor.syntax / appFg / accentFg 等）而非自估——iframe 内容色板
// 豁免登记见 .claude/test-exemptions.md（ADR-0003 双轨：壳层 token 面不覆盖
// iframe 文档内联样式面）。
//
// 改动值纪律：优先对齐 schemes/linear.ts 既有值（syntax 段 9 键 / ui 标量），
// 新增结构色不得偏离明度阶梯（l0-l5 + 半透明白）与单一强调色 #6e9ff2 家族。

// ── 排版布局（正文/标题/表格/引用/代码块/任务列表/分割线/滚动条）──

export const MD_PREVIEW_CSS = `
html, body { margin: 0; padding: 0; }
body {
  color: #b3aea6;
  background: transparent;
  font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
  font-size: 14px;
  line-height: 1.7;
  padding: 20px 24px 40px;
  word-wrap: break-word;
}
h1, h2, h3, h4, h5, h6 {
  color: #ece9e4;
  font-weight: 600;
  line-height: 1.35;
  margin: 1.4em 0 0.6em;
}
h1 { font-size: 1.7em; padding-bottom: 0.3em; border-bottom: 1px solid rgba(255,255,255,0.055); }
h2 { font-size: 1.4em; padding-bottom: 0.25em; border-bottom: 1px solid rgba(255,255,255,0.055); }
h3 { font-size: 1.2em; }
h4, h5, h6 { font-size: 1.05em; }
p { margin: 0.6em 0; }
a { color: #8fb4f5; text-decoration: none; }
a:hover { text-decoration: underline; }
img { max-width: 100%; }
code {
  font-family: "JetBrains Mono", "Cascadia Mono", Consolas, monospace;
  font-size: 0.9em;
}
:not(pre) > code {
  background: rgba(255,255,255,0.06);
  border-radius: 3px;
  padding: 0.1em 0.35em;
  color: #cfcac1;
}
pre {
  background: rgba(255,255,255,0.045);
  border: 1px solid rgba(255,255,255,0.055);
  border-radius: 6px;
  padding: 12px 14px;
  overflow-x: auto;
  line-height: 1.5;
}
pre code { background: none; padding: 0; color: #cfcac1; }
blockquote {
  margin: 0.8em 0;
  padding: 0.1em 1em;
  border-left: 3px solid rgba(110,159,242,0.55);
  color: #8a857d;
}
blockquote p { margin: 0.4em 0; }
hr { border: none; border-top: 1px solid rgba(255,255,255,0.055); margin: 1.6em 0; }
table { border-collapse: collapse; margin: 0.8em 0; }
th, td { border: 1px solid rgba(255,255,255,0.1); padding: 6px 12px; }
th { background: rgba(255,255,255,0.05); color: #ece9e4; }
tr:nth-child(2n) td { background: rgba(255,255,255,0.02); }
ul, ol { padding-left: 1.6em; }
li { margin: 0.25em 0; }
li.task-list-item { list-style: none; margin-left: -1.2em; }
li.task-list-item input[type="checkbox"] { margin-right: 0.4em; accent-color: #6e9ff2; }
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.10); border-radius: 5px; }
::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.20); }
.slterm-mermaid-error { color: #d9706b; font-size: 0.9em; }
.slterm-mermaid-error pre { color: #cfcac1; }
`;

// ── 语法高亮主题（映射 linear editor.syntax 9 键 + terminal 亮色系邻近值）──

export const MD_HLJS_CSS = `
.hljs-comment, .hljs-quote { color: #6b675f; font-style: italic; }
.hljs-keyword, .hljs-selector-tag, .hljs-literal { color: #b48ce0; }
.hljs-string, .hljs-regexp, .hljs-addition { color: #93b573; }
.hljs-number, .hljs-symbol, .hljs-bullet, .hljs-link { color: #d89a66; }
.hljs-title, .hljs-title.function_, .hljs-section, .hljs-name { color: #7fa8e8; }
.hljs-type, .hljs-class .hljs-title, .hljs-built_in, .hljs-title.class_ { color: #6fbfc4; }
.hljs-attr, .hljs-attribute, .hljs-variable, .hljs-property { color: #d9827e; }
.hljs-operator, .hljs-punctuation { color: #6fbfc4; }
.hljs-meta, .hljs-selector-id, .hljs-selector-class { color: #7fa8e8; }
.hljs-deletion { color: #d9706b; }
.hljs-emphasis { font-style: italic; }
.hljs-strong { font-weight: 600; }
`;
