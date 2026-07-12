// useCommandDetection — OSC 133 命令边界检测 hook
//
// 解析 shell-integration.ps1 注入的 OSC 133 C/D 序列，
// 匹配注册的命令（如 claude）并通过 onTabStateChange 回调通知页签标题/图标切换。
// 仅限于 pwsh/powershell——cmd.exe 无 shell integration 能力。

import { useCallback, useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import { tabTitleRegistry } from "./TabTitleRegistry";
import type { TabState } from "./TabTitleRegistry";

/** OSC 133 命令边界检测 hook
 *
 * @param terminal            xterm.js Terminal 实例（可为 null，null 时不注册 handler）
 * @param onTabStateChange    页签状态变更回调
 * @param sharedCmdRunningRef 共享的命令运行状态 ref（由 useXterm 创建，供 usePtyOutput 读取）
 */
export function useCommandDetection(
  terminal: Terminal | null,
  onTabStateChange?: (state: TabState) => void,
  sharedCmdRunningRef?: MutableRefObject<boolean>,
): {
  /** 重置命令运行状态（PTY spawn 成功后 / 进程退出时调用） */
  resetCommandState: () => void;
} {
  /** 当前是否有注册的命令在运行（如 claude）——使用外部共享 ref 或内部创建 */
  const _internalCmdRef = useRef(false);
  const isCommandRunningRef = sharedCmdRunningRef ?? _internalCmdRef;
  /** onTabStateChange 回调 ref（避免 handler 闭包捕获旧回调） */
  const onTabStateChangeRef = useRef(onTabStateChange);
  onTabStateChangeRef.current = onTabStateChange;

  useEffect(() => {
    if (!terminal) return;

    // OSC 133;D（命令退出），xterm.js 解析器剥离 OSC number 前缀（133），
    // handler 收到的 data 为 "C;claude" 或 "D;0"
    const disposable = terminal.parser.registerOscHandler(133, (data: string) => {
      const semicolonIndex = data.indexOf(";");
      const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;

      if (type === "C") {
        // OSC 133 C — 命令即将执行
        const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : "";
        const rule = tabTitleRegistry.match(command);
        if (rule) {
          isCommandRunningRef.current = true;
          onTabStateChangeRef.current?.({ active: true, title: rule.title, icon: rule.icon });
        }
      } else if (type === "D" && isCommandRunningRef.current) {
        // OSC 133 D — 命令执行完毕
        isCommandRunningRef.current = false;
        onTabStateChangeRef.current?.({ active: false });
      }

      // 返回 false 不消费序列，xterm.js 仍渲染提示符
      return false;
    });

    return () => {
      disposable.dispose();
    };
  }, [terminal]);

  /** PTY spawn 成功后 / 进程退出时重置命令运行状态（稳定引用） */
  const resetCommandState = useCallback(() => {
    isCommandRunningRef.current = false;
    onTabStateChangeRef.current?.({ active: false });
  }, []);

  return { resetCommandState };
}
