// useCommandDetection — OSC 133 命令边界检测 hook
//
// 解析 shell-integration.ps1 注入的 OSC 133 C/D 序列，
// 经 CliProfileRegistry.matchByCommand 匹配 CLI profile（首 token 精确匹配，
// 覆盖 claude --resume / claude -p 等带参变体）并通过 onTabStateChange
// 回调通知页签标题/状态切换。
// 仅限于 pwsh/powershell——cmd.exe 无 shell integration 能力。
// TQ-E-01：注册体抽离至 oscHandlers.ts 纯注册层，本文件为薄包装（仅注入依赖）。

import { useCallback, useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import { TerminalRegistry } from "./TerminalRegistry";
import { cliProfileRegistry } from "../../features/cliProfiles";
import type { AgentStatus } from "../../lib/agentStatus";
import { registerOsc133 } from "./oscHandlers";

/** 页签状态变化事件（原 TabTitleRegistry.ts 定义，注册表退役后迁入本文件顶部导出） */
export interface TabState {
  /** 命令是否启动（true=启动, false=退出） */
  active: boolean;
  /** 命令运行时标题（active=true 时有效） */
  title?: string;
  /** 命令运行时状态（active=true 时有效，null=无状态；渲染层映射 StatusDot 圆点，IC-03） */
  status?: AgentStatus | null;
  /** active=false 时是否恢复原标题（缺省 true；false = 仅清图标不恢复——
   *  B13：SessionEnd/EXIT hook 事件与 spawn 初始化重置不恢复标题，恢复只由
   *  真退出信号（OSC 133 D / PTY EXIT）承担） */
  restoreTitle?: boolean;
}

/** OSC 133 命令边界检测 hook
 *
 * @param terminal            xterm.js Terminal 实例（可为 null，null 时不注册 handler）
 * @param panelId             面板 ID（供 TerminalRegistry 写入会话状态）
 * @param onTabStateChange    页签状态变更回调
 * @param sharedCmdRunningRef 共享的命令运行状态 ref（由 useXterm 创建，供 usePtyOutput 读取）
 */
export function useCommandDetection(
  terminal: Terminal | null,
  panelId: string,
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
    // TQ-E-01: 注册体在 oscHandlers.ts 纯注册层——本 hook 仅注入依赖：
    // matchByCommand/setAgentSession/onTabStateChange 参数化，语义与原内联体一致
    // （B12 先写会话再发回调的顺序在注册层内保持）
    const disposable = registerOsc133(terminal, {
      isCommandRunning: isCommandRunningRef,
      matchByCommand: (cmd) => cliProfileRegistry.matchByCommand(cmd),
      setAgentSession: (cliId) =>
        TerminalRegistry.setAgentSession(
          panelId,
          cliId ? { cliId, matchedCommand: cliId } : null,
        ),
      onTabStateChange: (s) => onTabStateChangeRef.current?.(s as TabState),
    });

    return () => {
      disposable.dispose();
    };
  }, [terminal]);

  /** PTY spawn 成功后 / 进程退出时重置命令运行状态（稳定引用） */
  const resetCommandState = useCallback(() => {
    isCommandRunningRef.current = false;
    // B13: 初始化重置仅清图标不恢复标题——重启恢复时标题刚经 B12 重算，
    // 恢复会抹掉重算结果（B12/B13 共同守卫）
    onTabStateChangeRef.current?.({ active: false, restoreTitle: false });
  }, []);

  return { resetCommandState };
}
