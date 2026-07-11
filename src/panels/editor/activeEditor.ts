// activeEditor.ts — 模块级"当前聚焦编辑器"指针
//
// 解决多编辑器共享命令 id（editor.save）时的派发问题：
// 命令在注册表只注册一次，handler 经此指针派发到**当前聚焦**的编辑器实例。
// 编辑器 focusin 时 setActiveEditor(自身 actions)，focusout 时 clearActiveEditor（仅在匹配时）。

/** 聚焦编辑器对外暴露的动作（供快捷键命令 handler 调用） */
export interface EditorActions {
  save: () => void;
}

let active: EditorActions | null = null;

/** 设置当前聚焦编辑器 */
export function setActiveEditor(actions: EditorActions): void {
  active = actions;
}

/** 清除当前聚焦编辑器——仅当传入的 actions 正是当前 active 时才清（防竞态） */
export function clearActiveEditor(actions: EditorActions): void {
  if (active === actions) active = null;
}

/** 获取当前聚焦编辑器动作（无聚焦编辑器时为 null） */
export function getActiveEditor(): EditorActions | null {
  return active;
}
