// keyboard.ts — 编辑器快捷键命令工厂
//
// 职责：返回编辑器上下文的 Command[]，在 App.tsx 一次性注册（不随编辑器实例增删）。
// handler 经 getActiveEditor() 派发到**当前聚焦**编辑器——多编辑器下始终作用于聚焦实例。
// 元数据（默认键/优先级）来自 commandCatalog，本文件只提供 handler。
//
// 注意：Ctrl+F 搜索、撤销/重做等仍由 CodeMirror 内部 keymap 处理（注册表未注册 → 冒泡到 CM）。

import type { Command } from "../../features/shortcuts";
import { commandFromMeta } from "../../features/shortcuts";
import { getActiveEditor } from "./activeEditor";

/**
 * 创建编辑器面板的快捷键命令列表（一次性注册，handler 派发到聚焦编辑器）。
 * 无聚焦编辑器时 handler 返回 false（透传）。
 */
export function createEditorShortcuts(): Command[] {
  return [
    commandFromMeta("editor.save", () => {
      const e = getActiveEditor();
      if (!e) return false;
      e.save();
      return true;
    }),
    commandFromMeta("editor.toggleWordWrap", () => {
      const e = getActiveEditor();
      if (!e) return false;
      e.toggleWordWrap();
      return true;
    }),
  ];
}
