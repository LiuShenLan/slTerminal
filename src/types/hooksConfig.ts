// hooks 配置 DTO — 阶段 3 配置面板类型定义
//
// 原始 JSON 类型对应 settings.json 的 hooks 子树（契约 C13-1 编辑范围），
// GUI 模型为面板展示/编辑用（EventTree / HandlerForm 数据源）。
// handler 字段矩阵照 contract.md C13-3（2026-07-31 官方文档核实版）。

/** hooks 配置层级（FE-14 收窄：当前仅 claude 三层 user/project/local，后端 parse_layer
    只认这三值；层切换器数据源仍为 profile.capabilities.hooks.configLayers（KZ-4），
    未来 CLI 加层时再泛化本联合——types/CLAUDE.md 登记在 S19） */
export type HooksLayer = "user" | "project" | "local";

// ═══════════════════════════════════════════════════════════════════
// 原始 JSON 类型（settings.json 的 hooks 子树）
// ═══════════════════════════════════════════════════════════════════

/** settings.json 的 hooks 子树：事件名 → matcher 组数组 */
export type HooksConfigJson = Record<string, MatcherGroupJson[]>;

/** matcher 组：matcher 匹配串（省略 = 全匹配，C13-5）+ handler 数组 */
export interface MatcherGroupJson {
  matcher?: string;
  hooks: HookHandlerJson[];
}

/** 原始 hook handler（官方 settings.json 字段，字段矩阵照 C13-3） */
export interface HookHandlerJson {
  type: "command" | "http" | "mcp_tool" | "prompt" | "agent";
  // command 型：command* / args / async / asyncRewake / shell
  command?: string;
  args?: string[];
  async?: boolean;
  asyncRewake?: boolean;
  shell?: string;
  // http 型：url* / headers / allowedEnvVars（无 method/body——固定 POST，body 恒为事件 JSON）
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
  // agent 型：prompt* / model（无 description/subagent_type——那是内置 Agent 工具的输入参数）
  // 通用字段：if（仅工具事件求值）/ timeout / statusMessage；once 不展示（C13-3）
  if?: string;
  timeout?: number;
  statusMessage?: string;
}

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

