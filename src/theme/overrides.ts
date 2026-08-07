// overrides — 组件库配色注入（active 方案 → 库专用 CSS 变量 / CM6 扩展）
//
// 四导出签名以 Stage 01 脚本头注释 C2 为准（spec §4.8）：
//   dockviewVarStyle(): Record<string, string>    active 方案 libraries.dockview 20 条
//     CSS 变量，键为变量名原样（如 --dv-group-view-background-color），供 React style 内联注入
//   allotmentVarStyle(): Record<string, string>   2 键（--separator-border / --focus-border，
//     --sash-size 等尺寸变量不动）
//   editorTheme: Extension                        = schemeRegistry.getActive().editor.theme
//     （darcula 为 oneDark 直 import 透出）
//   editorColorOverrides(): Extension             active 方案 editor.overrides → CM6
//     EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background）
//
// 函数形导出每次调用取当前 active 方案（支持 D2 热切换）；editorTheme 为模块级常量
// （求值时机由 main.tsx 启动序列保证，见 spec §5）。

import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
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

/** CM 主题扩展透出（= active 方案 editor.theme，darcula 为 oneDark） */
export const editorTheme: Extension = schemeRegistry.getActive().editor.theme;

/** active 方案 editor.overrides → CM6 EditorView.theme 扩展（lint 7 键 / searchMatch 4 键 / background） */
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
      // 搜索匹配高亮——与 oneDark 同选择器同属性形态（match + outline 两键）
      ".cm-searchMatch": {
        backgroundColor: overrides.searchMatch.match,
        outline: `1px solid ${overrides.searchMatch.matchOutline}`,
      },
      ".cm-searchMatch.cm-searchMatch-selected": {
        backgroundColor: overrides.searchMatch.selected,
      },
      ".cm-selectionMatch": {
        backgroundColor: overrides.searchMatch.selectionMatch,
      },
      // 编辑器背景——对齐 ui.editorBg
      "&": { backgroundColor: overrides.background },
    },
    { dark: true },
  );
}

/** @codemirror/lint underline() 同款 SVG data URL（色值参数化），源码模板见其 dist/index.js:642-647 */
function lintUnderline(color: string): string {
  return `url('data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" width="6" height="3">${encodeURIComponent(
    `<path d="m0 2.5 l2 -1.5 l1 0 l2 1.5 l1 0" stroke="${color}" fill="none" stroke-width=".7"/>`,
  )}</svg>')`;
}
