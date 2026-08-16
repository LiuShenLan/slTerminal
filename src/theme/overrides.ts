// overrides — 组件库配色注入（active 方案 → 库专用 CSS 变量 / CM6 扩展）
//
// 五导出签名：
//   dockviewVarStyle(): Record<string, string>    active 方案 libraries.dockview 20 条
//     CSS 变量，键为变量名原样（如 --dv-group-view-background-color），供 React style 内联注入
//   allotmentVarStyle(): Record<string, string>   2 键（--separator-border / --focus-border，
//     --sash-size 等尺寸变量不动）
//   editorTheme: Extension                        = schemeRegistry.getActive().editor.theme
//     （linear 为 oneDark 底座直 import 透出）
//   editorColorOverrides(): Extension             active 方案 editor.overrides → CM6
//     EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background / 正文与行号）
//   editorSyntaxHighlight(): Extension            active 方案 editor.overrides.syntax → CM6
//     syntaxHighlighting 扩展（9 组 tag → 色映射），消费点须置于 editorTheme 之前
//     （mountStyles reverse 层叠——后声明的自定义规则排最后=恒胜，ACC-05）
//
// 函数形导出每次调用取当前 active 方案（支持 D2 热切换）；editorTheme 为模块级常量
// （求值时机由 main.tsx 启动序列保证）。

import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { schemeRegistry } from "./schemeRegistry";

/** dockview 20 条 CSS 变量 → React style 对象（键为变量名原样） */
export function dockviewVarStyle(): Record<string, string> {
  return schemeRegistry.getActive().libraries.dockview;
}

/** allotment 2 键 CSS 变量 → style 对象（--separator-border / --focus-border） */
export function allotmentVarStyle(): Record<string, string> {
  const { allotment } = schemeRegistry.getActive().libraries;
  return {
    "--separator-border": allotment.separatorBorder,
    "--focus-border": allotment.focusBorder,
  };
}

/** CM 主题扩展透出（= active 方案 editor.theme，linear 为 oneDark 底座） */
export const editorTheme: Extension = schemeRegistry.getActive().editor.theme;

/** active 方案 editor.overrides → CM6 EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background / 正文与行号 4 键） */
export function editorColorOverrides(): Extension {
  const { overrides } = schemeRegistry.getActive().editor;
  return EditorView.theme(
    {
      // lint 诊断波浪线——backgroundImage 复刻 @codemirror/lint baseTheme 的
      // underline() 同款 SVG（width 6 height 3 波浪 path），仅色值参数化。
      // 必须同形覆盖：若改用 backgroundColor，值=库默认时也会由
      // underline 波浪线变为实心色块，违反 D1 零视觉变化。
      ".cm-lintRange-error": { backgroundImage: lintUnderline(overrides.lint.error) },
      ".cm-lintRange-warning": { backgroundImage: lintUnderline(overrides.lint.warning) },
      ".cm-lintRange-info": { backgroundImage: lintUnderline(overrides.lint.info) },
      ".cm-lintRange-hint": { backgroundImage: lintUnderline(overrides.lint.hint) },
      ".cm-lintRange-active": { backgroundColor: overrides.lint.activeBackground },
      // lint tooltip——特异性（0,2,0）高于 oneDark 的 .cm-tooltip（0,1,0），
      // 背景与边框均由方案决定
      ".cm-tooltip.cm-tooltip-lint": {
        backgroundColor: overrides.lint.tooltipBackground,
        border: `1px solid ${overrides.lint.tooltipBorder}`,
      },
      // 搜索匹配高亮——与 oneDark 同选择器同属性形态（match + outline 两键）。
      // 选择器带 "&.cm-editor " 前缀提升特异性（ACC-05 修复）：@codemirror/view 的
      // mountStyles 把 styleModule facet 数组 reverse() 后挂载，扩展数组
      // [editorTheme(oneDark), editorColorOverrides()] 编译后 oneDark 规则在 <style>
      // 标签内排在 overrides 之后（同特异性下后声明者胜）→ oneDark 恒赢，覆盖全失效。
      // 前缀使 .ͼx.cm-editor .cm-searchMatch（0,3,0）> oneDark .ͼo .cm-searchMatch
      // （0,2,0），胜负与扩展数组顺序无关。
      "&.cm-editor .cm-searchMatch": {
        backgroundColor: overrides.searchMatch.match,
        outline: `1px solid ${overrides.searchMatch.matchOutline}`,
      },
      "&.cm-editor .cm-searchMatch.cm-searchMatch-selected": {
        backgroundColor: overrides.searchMatch.selected,
      },
      "&.cm-editor .cm-selectionMatch": {
        backgroundColor: overrides.searchMatch.selectionMatch,
      },
      // 编辑器背景——对齐 ui.editorBg；"&.cm-editor" 编译为 .ͼx.cm-editor（0,2,0）>
      // oneDark 的 "&" 编译 .ͼo（0,1,0），原因同上（mountStyles 反转顺序，ACC-05 修复）
      "&.cm-editor": { backgroundColor: overrides.background },
      // 正文前景——对齐 overrides.plainText；oneDark 的正文色在其 "&" 规则
      // （.ͼo，0,1,0），前缀后 .ͼx.cm-editor .cm-content（0,3,0）恒胜（ACC-05）
      "&.cm-editor .cm-content": { color: overrides.plainText },
      // 行号区背景/前景/右发丝线——oneDark 的 .cm-gutters（0,1,0）会被前缀形态
      // .ͼx.cm-editor .cm-gutters（0,3,0）覆盖（ACC-05）；发丝线对齐 ui.separatorBg
      "&.cm-editor .cm-gutters": {
        backgroundColor: overrides.background,
        color: overrides.lineNumber,
        borderRight: `1px solid ${schemeRegistry.getActive().ui.separatorBg}`,
      },
      // 行号元素前景（显式落值，防继承链条被 oneDark 干扰）
      "&.cm-editor .cm-lineNumbers .cm-gutterElement": {
        color: overrides.lineNumber,
      },
      // 活跃行行号前景——CM6 baseTheme 的 .cm-activeLineGutter 只有背景，
      // 前景色由本规则显式落值（oneDark 无此规则，无层叠竞争）
      "&.cm-editor .cm-lineNumbers .cm-gutterElement.cm-activeLineGutter": {
        color: overrides.lineNumberActive,
      },
    },
    { dark: true },
  );
}

/** active 方案 editor.overrides.syntax → CM6 syntaxHighlighting 扩展（9 组 tag → 色映射）
 *
 * 消费点必须在扩展数组中置于 editorTheme 之前——mountStyles reverse 层叠下，
 * 后声明的自定义规则在 <style> 标签内排最后=恒胜（ACC-05，与 editorColorOverrides 相反：
 * 后者靠 &.cm-editor 前缀提升特异性，本扩展与 oneDark 的 HighlightStyle 是同机制竞争，
 * 只能靠数组顺序决胜）。
 */
export function editorSyntaxHighlight(): Extension {
  const { syntax } = schemeRegistry.getActive().editor.overrides;
  return syntaxHighlighting(
    HighlightStyle.define([
      { tag: tags.propertyName, color: syntax.property },
      { tag: tags.string, color: syntax.string },
      { tag: tags.number, color: syntax.number },
      { tag: tags.keyword, color: syntax.keyword },
      { tag: tags.function(tags.variableName), color: syntax.function },
      { tag: tags.typeName, color: syntax.type },
      { tag: tags.operator, color: syntax.operator },
      { tag: tags.punctuation, color: syntax.punctuation },
      { tag: tags.comment, color: syntax.comment },
    ]),
  );
}

/** @codemirror/lint underline() 同款 SVG data URL（色值参数化），源码模板见其 dist/index.js:642-647 */
function lintUnderline(color: string): string {
  return `url('data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" width="6" height="3">${encodeURIComponent(
    `<path d="m0 2.5 l2 -1.5 l1 0 l2 1.5 l1 0" stroke="${color}" fill="none" stroke-width=".7"/>`,
  )}</svg>')`;
}
