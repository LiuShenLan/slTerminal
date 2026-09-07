// editorThemeSlot.ts — CM 编辑器主题热切换槽（CP-039）
//
// 一个 EditorView 一个槽（Compartment 不可跨 view 共享——editor/CLAUDE.md 红线）。
// extension 入扩展数组，替换原 [editorSyntaxHighlight(), editorTheme, editorColorOverrides()]
// 三项（槽内数组顺序固化不变：syntax 必须先于 theme——ACC-05 mountStyles reverse 层叠）。
// view 创建后 bind(view)：订阅 schemeRegistry.onDidChange，方案一切换即 Compartment
// 重配置——文档/光标/undo 全保留，编辑器不重建。

import { Compartment } from "@codemirror/state";
import type { Extension } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import {
  getEditorTheme,
  editorColorOverrides,
  editorSyntaxHighlight,
} from "./overrides";
import { schemeRegistry } from "./schemeRegistry";
// 内置方案注册副作用（照 colors.ts 内部 import "./schemes" 同一保护模式）——
// bundle 在调用期取 active 方案，须保证 linear 恒已注册；重复注册同 id 覆盖无副作用
import "./schemes";

/** 主题包：顺序固化 [syntax, theme, overrides]（ACC-05——syntax 只能靠数组顺序决胜）。
 *  export 仅供本模块 createEditorThemeSlot 调用——生产消费方经槽 extension 间接获得，
 *  无外部直 import，按合法导出登记 knip.json ignoreIssues（2026-09-08）。 */
export function editorThemeBundle(): Extension[] {
  return [editorSyntaxHighlight(), getEditorTheme(), editorColorOverrides()];
}

export interface EditorThemeSlot {
  /** 入扩展数组的单项（Compartment 包装） */
  extension: Extension;
  /** view 创建后调用：订阅方案变更，返回取消函数（组件卸载时调用） */
  bind(view: EditorView): () => void;
}

/** 每 EditorView 创建一个槽 */
export function createEditorThemeSlot(): EditorThemeSlot {
  const compartment = new Compartment();
  return {
    extension: compartment.of(editorThemeBundle()),
    bind(view) {
      return schemeRegistry.onDidChange(() => {
        view.dispatch({
          effects: compartment.reconfigure(editorThemeBundle()),
        });
      });
    },
  };
}
