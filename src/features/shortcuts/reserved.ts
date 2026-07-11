// reserved.ts — 保留键判定
//
// 两类键永不可被用户覆盖绑定（决策 A2 锁定键）：
//   (a) 终端控制字符透传——xterm 将其编码为控制字节发给 PTY，绑定会截断 SIGINT 等语义
//   (b) CodeMirror 内部编辑键——归 CM 自身 keymap 管理，本期不纳入自定义
//
// 校验发生在构建绑定表时（ShortcutRegistry.effectiveKeystroke）：用户覆盖命中保留键 →
// console.warn + 回退默认键。代码定义的默认键不过此校验（可信）。

import type { KeyStroke, ShortcutContext } from "./types";
import { formatKeystroke } from "./keystroke";

/** 终端保留键：控制字符透传（保护 Ctrl+C=SIGINT 等） */
const TERMINAL_RESERVED = new Set<string>([
  "Ctrl+KeyC", // \x03 SIGINT
  "Ctrl+KeyV", // \x16
  "Ctrl+KeyX", // \x18
  "Ctrl+KeyZ", // \x1a
  "Ctrl+KeyA", // \x01 行首
]);

/** 编辑器保留键：CodeMirror 内部编辑键（搜索/撤销/重做/缩进） */
const EDITOR_RESERVED = new Set<string>([
  "Ctrl+KeyF", // 搜索
  "Ctrl+KeyZ", // 撤销
  "Ctrl+KeyY", // 重做
  "Ctrl+Shift+KeyZ", // 重做
  "Tab", // 缩进
  "Shift+Tab", // 反缩进
]);

/**
 * 判断某 keystroke 在给定 context 下是否为保留键（不可被用户绑定）。
 * global 命令在终端和编辑器都会触发，故须同时避开两套保留集。
 */
export function isReserved(ks: KeyStroke, context: ShortcutContext): boolean {
  const fp = formatKeystroke(ks);
  if (context === "terminal") return TERMINAL_RESERVED.has(fp);
  if (context === "editor") return EDITOR_RESERVED.has(fp);
  if (context === "global") return TERMINAL_RESERVED.has(fp) || EDITOR_RESERVED.has(fp);
  return false;
}
