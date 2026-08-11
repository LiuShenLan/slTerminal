// cliProfiles/types.ts — CLI profile 类型定义（跨边界契约，spec 00 §3.1）
//
// capabilities 为可选能力域：未声明 = 该域不可用，消费方优雅降级。
// 类型引用随全局更名同步：ClaudeStatus → AgentStatus（Stage 02 随行），
// AgentEventPayload / HistorySession 随 Stage 03/04 更名时同步。

import type { AgentStatus } from "../../lib/agentStatus";
import type { AgentEventPayload } from "../../types/agent";
import type { AgentHistorySession } from "../../types/agentHistory";
// React 仅类型 import（KZ-1：类型引用 ComponentType/MutableRefObject，运行期擦除——
// 不构成 features/cliProfiles → panels 的运行期依赖，防循环依赖）
import type React from "react";

/** hub 配置编辑器组件 props（泛化自 ClaudeHooksConfigEditorProps，KZ-1） */
export interface HooksConfigEditorProps {
  /** 目标 CLI profile（hub 选中态）——cliId = profile.id，ipc 实参唯一来源 */
  profile: CodingCliProfile;
  /** dirty 变化上报（hub 切换 CLI 的 dirty 守卫用；undefined = 不上报） */
  onDirtyChange?: (dirty: boolean) => void;
  /** hub 切换确认弹窗守卫 ref——守卫期间编辑器回归触发不重读（防循环） */
  askGuardRef?: React.MutableRefObject<boolean>;
}

/** hooks 能力域（协议知识实现留在 profiles/<cli>/，本文件仅签名） */
export interface HooksCapability {
  /** hook 事件名 → 会话四态（F3 状态机，claude = eventToStatus 10 事件映射） */
  eventToStatus(event: string, notificationType?: string | null): AgentStatus;
  /** hook 事件负载 → 通知类别判定（claude = classifyEvent 五映射） */
  classifyNotification(
    payload: AgentEventPayload,
  ): "permission" | "error" | "done" | null;
  /** 上下文窗口上限（用量口径分母，claude = 200_000） */
  contextLimit: number;
  /** 保存后提示文案（claude = "hooks 改动需重启 claude 会话生效"） */
  restartHint: string;
  /** 是否提供 hooks 配置编辑器（hub 面板选择行过滤条件，claude = true） */
  hasConfigEditor: boolean;
  /** hub 配置编辑器组件（hasConfigEditor=true 时必填；缺失 → hub 空态防御，
      不渲染任何 CLI 编辑器——KZ-1；claude = ClaudeHooksConfigEditor，挂载于
      profiles/claude/，编辑器实现属 claude 合法领地） */
  configEditor?: React.ComponentType<HooksConfigEditorProps>;
  /** hooks 配置分层声明（hasConfigEditor=true 时必填——KZ-4；编辑器层切换器
      数据源。claude = user/project/local 三层现值（含 label/hint 文案，迁自
      ClaudeHooksConfigEditor 退役 LAYERS 常量）——值集由各 CLI profile 自声明） */
  configLayers?: { id: string; label: string; hint: string }[];
}

/** history 能力域（历史会话恢复策略） */
export interface HistoryCapability {
  /** 是否支持分支恢复（claude = true） */
  supportsFork: boolean;
  /** 恢复命令（右键菜单复制，claude = `claude --resume <id>`） */
  buildResumeCommand(session: AgentHistorySession): string;
  /** 恢复注入内容（恢复编排第 4 步，claude = resume 命令 + fork 追加） */
  buildRestoreInput(
    session: AgentHistorySession,
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
