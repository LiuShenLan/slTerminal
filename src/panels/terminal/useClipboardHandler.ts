// useClipboardHandler — OSC 52 剪贴板拦截 hook
//
// 职责：
// - 注册 term.parser.registerOscHandler(52, ...) 拦截 Claude Code /copy 命令
// - base64 解码 + UTF-8 TextDecoder 支持 CJK 内容
// - 焦点门控：非可见面板忽略
// - Payload 上限 1MB 防 DoS
// - 安全策略：仅写入、仅系统剪贴板(c)、忽略 primary/secondary

import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { writeText } from "../../ipc/clipboard";

/** OSC 52 剪贴板 payload 上限（1MB），防止 DoS */
const MAX_OSC52_PAYLOAD = 1048576;

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
    const disposable = terminal.parser.registerOscHandler(52, (data: string) => {
      const semicolonIdx = data.indexOf(";");
      if (semicolonIdx === -1) return true;

      const selector = data.substring(0, semicolonIdx);
      const payload = data.substring(semicolonIdx + 1);

      // 仅系统剪贴板（c），忽略 p（primary）和 q（secondary）
      if (selector && selector !== "c") return true;
      // 禁止读请求
      if (payload === "?" || payload.length === 0) return true;
      // Payload 上限 MAX_OSC52_PAYLOAD
      if (payload.length > MAX_OSC52_PAYLOAD) return true;
      // 焦点门控：非可见面板忽略
      if (visibleRef.current === false) return true;

      try {
        // atob 返回二进制字符串（每字符一个字节），需经 UTF-8 解码
        const binary = atob(payload);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
          bytes[i] = binary.charCodeAt(i);
        }
        const text = new TextDecoder().decode(bytes);
        writeText(text).catch((e) => console.error("[OSC52] 写剪贴板失败:", e));
      } catch {
        // base64 解码失败，静默忽略
      }
      return true;
    });

    // 返回 cleanup：卸载时 dispose handler
    return () => {
      disposable.dispose();
    };
  }, [terminal]);
}
