// agentStatus.ts — 编码 CLI 会话四态类型（CLI 中立，MC-401 迁移）
//
// 事件→状态映射单点已随 MC-401 迁入 CLI profile hooks 能力：消费方按
// profile.hooks.eventToStatus 分发（claude 实现在
// src/features/cliProfiles/profiles/claude/strategies.ts）。lib 层不再含
// 任何 claude 事件名字面量（AC-5 守卫兼容）。
//
// UI 重设计（IC-03）：STATUS_EMOJI 常量与 getStatusIcon 已删除——状态渲染层
// 全部改 StatusDot 组件（src/lib/StatusDot.tsx，working→绿/attention→黄/
// done→灰/error→红），本文件仅保留四态类型契约。
//
// 已知行为假设（无法自动化验证）：
// - Ctrl+C 用户主动中断不发射任何 hook 事件（完成/错误事件为预期语义），
//   working 无中断出边为预期行为，依赖下一事件覆盖或空闲提示(~60s) 衰减转 attention

/** 编码 CLI 会话状态（四态 + null 表示无状态无图标） */
export type AgentStatus = "working" | "attention" | "done" | "error" | null;
