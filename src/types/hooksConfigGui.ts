// hooks 配置 GUI 模型 — 前端 GUI 模型,非后端 DTO(CP-024 单源化迁出)
//
// 迁自原 types/hooksConfig.ts GUI 段(阶段 3 配置面板类型定义):
// GUI 模型为面板展示/编辑用(EventTree / HandlerForm 数据源),
// Rust 无对应结构,不进 ts-rs 生成物——见 src/types/CLAUDE.md 残面清单。

// ═══════════════════════════════════════════════════════════════════
// GUI 模型（面板展示/编辑用）
// ═══════════════════════════════════════════════════════════════════

/** GUI 模型根：事件组列表 */
export interface HooksConfigGui {
  events: HookEventGroup[];
}

/** GUI 事件组：事件名 + matcher 组列表 */
export interface HookEventGroup {
  event: string;
  matcherGroups: HookMatcherGroup[];
}

/** GUI matcher 组：matcher 为 null 表示 JSON 中省略 matcher 键（全匹配，C13-5） */
export interface HookMatcherGroup {
  matcher: string | null;
  handlers: HookHandlerGui[];
}

/** GUI handler：5 种 handler 字段矩阵（C13-3 官方版，与 HookHandlerJson 同构） */
export interface HookHandlerGui {
  type: "command" | "http" | "mcp_tool" | "prompt" | "agent";
  // command 型：command* / args / async / asyncRewake / shell
  command?: string;
  args?: string[];
  async?: boolean;
  asyncRewake?: boolean;
  shell?: string;
  // http 型：url* / headers / allowedEnvVars（无 method/body）
  url?: string;
  headers?: Record<string, string>;
  allowedEnvVars?: string[];
  // mcp_tool 型：server* / tool* / input（字段名是 input 非 args）
  server?: string;
  tool?: string;
  input?: Record<string, unknown>;
  // prompt 型：prompt* / model / continueOnBlock
  prompt?: string;
  model?: string;
  continueOnBlock?: boolean;
  // agent 型：prompt* / model（无 description/subagent_type）
  // 通用字段：if / timeout / statusMessage；once 不展示（C13-3）
  if?: string;
  timeout?: number;
  statusMessage?: string;
}
