// keystroke.ts — KeyStroke 与可读字符串（"Ctrl+Shift+KeyC"）互转
//
// 字符串格式即快捷键指纹格式：修饰键固定序 Ctrl→Shift→Alt→Meta，末接 event.code。
// 用作 settings.json 中用户覆盖层的键位表示，也是注册表指纹索引的 key。
//
// 之所以安全用 "+" 分隔：KeyboardEvent.code 值（KeyC / Enter / Digit1 / F5 …）从不含 "+"。

import type { KeyStroke } from "./types";

/** 合法修饰键 token（顺序即规范序） */
const MODIFIER_ORDER = ["Ctrl", "Shift", "Alt", "Meta"] as const;
const ALLOWED_MODIFIERS = new Set<string>(MODIFIER_ORDER);

/**
 * KeyStroke → 规范字符串。
 * 有修饰键时形如 "Ctrl+Shift+KeyC"；无修饰键时直接是 code（如 "F1"）。
 */
export function formatKeystroke(ks: KeyStroke): string {
  const mods: string[] = [];
  if (ks.ctrlKey) mods.push("Ctrl");
  if (ks.shiftKey) mods.push("Shift");
  if (ks.altKey) mods.push("Alt");
  if (ks.metaKey) mods.push("Meta");
  return mods.length > 0 ? `${mods.join("+")}+${ks.code}` : ks.code;
}

/**
 * 字符串 → KeyStroke，非法输入返回 null。
 * 非法情形：空串、尾随/前导 "+"、未知修饰键、重复修饰键、大小写不符。
 */
export function parseKeystroke(s: string): KeyStroke | null {
  if (typeof s !== "string" || s.length === 0) return null;

  const parts = s.split("+");
  const code = parts.pop();
  if (!code) return null; // 空 code（尾随 "+" 或空串）

  // 剩余部分必须全是合法修饰键，且不重复
  for (const part of parts) {
    if (!ALLOWED_MODIFIERS.has(part)) return null; // 未知修饰键 / 前导 "+" 产生的空串
  }
  if (new Set(parts).size !== parts.length) return null; // 重复修饰键

  return {
    ctrlKey: parts.includes("Ctrl"),
    shiftKey: parts.includes("Shift"),
    altKey: parts.includes("Alt"),
    metaKey: parts.includes("Meta"),
    code,
  };
}

/** 判断字符串是否为合法 keystroke 表示 */
export function isValidKeystrokeString(s: string): boolean {
  return parseKeystroke(s) !== null;
}
