// activeTerminal.ts — 模块级"当前聚焦终端"指针
//
// 解决多终端共享命令 id（terminal.copy/paste/newline）时的派发问题：
// 命令在注册表只注册一次，handler 经此指针派发到**当前聚焦**的终端实例。
// 终端 focusin 时 setActiveTerminal(自身 actions)，focusout 时 clearActiveTerminal（仅在匹配时）。

/** 聚焦终端对外暴露的动作（供快捷键命令 handler 调用） */
export interface TerminalActions {
  getSelection: () => string | undefined;
  paste: (text: string) => void;
  writeToPty: (data: Uint8Array) => void;
}

let active: TerminalActions | null = null;

/** 设置当前聚焦终端 */
export function setActiveTerminal(actions: TerminalActions): void {
  active = actions;
}

/** 清除当前聚焦终端——仅当传入的 actions 正是当前 active 时才清（防竞态：A blur → B focus 后 A 的 blur 不清 B） */
export function clearActiveTerminal(actions: TerminalActions): void {
  if (active === actions) active = null;
}

/** 获取当前聚焦终端动作（无聚焦终端时为 null） */
export function getActiveTerminal(): TerminalActions | null {
  return active;
}
