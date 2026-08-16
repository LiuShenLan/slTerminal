// useCommandDetection — OSC 133 命令边界检测 hook
//
// 解析 shell-integration.ps1 注入的 OSC 133 C/D 序列，
// 经 CliProfileRegistry.matchByCommand 匹配 CLI profile（首 token 精确匹配，
// 覆盖 claude --resume / claude -p 等带参变体）并通过 onTabStateChange
// 回调通知页签标题/状态切换。
// 仅限于 pwsh/powershell——cmd.exe 无 shell integration 能力。

import { useCallback, useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import { TerminalRegistry } from "./TerminalRegistry";
import { cliProfileRegistry } from "../../features/cliProfiles";
import type { AgentStatus } from "../../lib/agentStatus";

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
    const disposable = terminal.parser.registerOscHandler(133, (data: string) => {
      const semicolonIndex = data.indexOf(";");
      const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;

      if (type === "C") {
        // OSC 133 C — 命令即将执行
        const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : "";
        const profile = cliProfileRegistry.matchByCommand(command);
        if (profile) {
          isCommandRunningRef.current = true;
          // MC-107: 写入会话状态（未注入 hooks 时无 usageSourcePath，用量条不可用）——
          // cliId 取匹配 profile 的 id（hook 事件三级解析反查键，MC-205）
          // B12: 先写会话再发回调——TerminalPanel 的 originalTitleRef 捕获守卫
          // 检查 agentSession 非空即跳过，回调触发 onDidTitleChange 时会话必须已置位
          TerminalRegistry.setAgentSession(panelId, {
            cliId: profile.id,
            matchedCommand: profile.id,
          });
          // 标题取自匹配 profile（tabTitle）；未命中零副作用（不触发回调）。
          // F9 行为修订：logo 不再经此路径直传——页签 logo 由 TerminalPanel
          // 订阅 TerminalRegistry 的 sessionChange 按 agentSession.cliId 查 profile
          // （会话绑定，见 TerminalPanel.tsx）
          onTabStateChangeRef.current?.({
            active: true,
            title: profile.tabTitle,
            status: "attention",
          });
        }
      } else if (type === "D" && isCommandRunningRef.current) {
        // OSC 133 D — 命令执行完毕
        isCommandRunningRef.current = false;
        onTabStateChangeRef.current?.({ active: false });
        // 注册命令退出 → 清除会话行
        TerminalRegistry.setAgentSession(panelId, null);
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
    // B13: 初始化重置仅清图标不恢复标题——重启恢复时标题刚经 B12 重算，
    // 恢复会抹掉重算结果（B12/B13 共同守卫）
    onTabStateChangeRef.current?.({ active: false, restoreTitle: false });
  }, []);

  return { resetCommandState };
}
