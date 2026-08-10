// cliProfiles/types.ts — CLI profile 类型定义（跨边界契约，spec 00 §3.1）
//
// 本 Stage 落地时 capabilities 签名引用现状类型名（ClaudeStatus / HookEventPayload /
// HistorySession），Stage 02/03/04 更名时随行同步。
// capabilities 为可选能力域：未声明 = 该域不可用，消费方优雅降级。

import type { ClaudeStatus } from "../../lib/claudeStatus";
import type { HookEventPayload } from "../../ipc/hooks";
import type { HistorySession } from "../../types/claudeHistory";

/** hooks 能力域（协议知识实现留在 profiles/<cli>/，本文件仅签名） */
export interface HooksCapability {
  /** hook 事件名 → 会话四态（F3 状态机，claude = eventToStatus 10 事件映射） */
  eventToStatus(event: string, notificationType?: string | null): ClaudeStatus;
  /** hook 事件负载 → 通知类别判定（claude = classifyEvent 五映射） */
  classifyNotification(
    payload: HookEventPayload,
  ): "permission" | "error" | "done" | null;
  /** 上下文窗口上限（用量口径分母，claude = 200_000） */
  contextLimit: number;
  /** 保存后提示文案（claude = "hooks 改动需重启 claude 会话生效"） */
  restartHint: string;
  /** 是否提供 hooks 配置编辑器（hub 面板选择行过滤条件，claude = true） */
  hasConfigEditor: boolean;
}

/** history 能力域（历史会话恢复策略） */
export interface HistoryCapability {
  /** 是否支持分支恢复（claude = true） */
  supportsFork: boolean;
  /** 恢复命令（右键菜单复制，claude = `claude --resume <id>`） */
  buildResumeCommand(session: HistorySession): string;
  /** 恢复注入内容（恢复编排第 4 步，claude = resume 命令 + fork 追加） */
  buildRestoreInput(
    session: HistorySession,
    opts: { fork: boolean },
  ): string;
}

/** CLI profile（跨边界契约，spec 00 §3.1） */
export interface CodingCliProfile {
  /** cliId 公共键，如 "claude" */
  id: string;
  /** 展示名，如 "claude" */
  displayName: string;
  /** 首 token 精确匹配键集（支持多首 token，如 ["claude","cc"]） */
  commands: string[];
  /** 品牌 logo 根绝对路径，如 "/cli-icons/claude.png" */
  iconSrc: string;
  /** OSC 133 C 命中页签标题 */
  tabTitle: string;
  /** 能力域（可选——未声明 = 该域不可用，消费方优雅降级） */
  capabilities: {
    hooks?: HooksCapability;
    history?: HistoryCapability;
  };
}
