// useClipboardHandler — OSC 52 剪贴板拦截 hook
//
// 职责：
// - 拦截 OSC 52 剪贴板序列（Claude Code /copy 命令），注册经 oscHandlers.ts
// - base64 解码 + UTF-8 TextDecoder 支持 CJK 内容
// - 焦点门控：非可见面板忽略
// - Payload 上限 1MB 防 DoS
// - 安全策略：仅写入、仅系统剪贴板(c)、忽略 primary/secondary
// - TQ-E-01：注册体抽离至 oscHandlers.ts 纯注册层，本文件为薄包装（仅注入依赖）

import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { writeText } from "../../ipc/clipboard";
import { registerOsc52 } from "./oscHandlers";

/**
 * 注册 OSC 52 剪贴板拦截 handler。
 * 返回 cleanup disposer，调用方在终端卸载时 dispose。
 *
 * @param terminal  - xterm.js Terminal 实例
 * @param visible   - 面板是否可见（焦点门控）
 */
export function useClipboardHandler(
  terminal: Terminal | null,
  visible: boolean,
): void {
  // 用 ref 持有 visible，避免 handler 闭包依赖 visible 导致重复注册
  const visibleRef = useRef(visible);
  visibleRef.current = visible;

  useEffect(() => {
    if (!terminal) return;

    // OSC 52 剪贴板：拦截 Claude Code /copy 命令，写入系统剪贴板
    // 安全策略：仅写入、仅系统剪贴板(c)、仅可见面板、payload≤1MB、禁止读
    // TQ-E-01: 注册体在 oscHandlers.ts 纯注册层（L3 headless 与生产共用同一真值源）
    const disposable = registerOsc52(terminal, {
      isVisible: () => visibleRef.current,
      writeText,
    });

    // 返回 cleanup：卸载时 dispose handler
    return () => {
      disposable.dispose();
    };
  }, [terminal]);
}
