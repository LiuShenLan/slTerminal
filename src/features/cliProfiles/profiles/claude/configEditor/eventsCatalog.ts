// eventsCatalog.ts — 事件元数据单点（P3-FE-26）
//
// 真值源：本文件事件元数据全表（30 事件 x 10 组，自官方 schema 提取，
// 见 src/features/cliProfiles/CLAUDE.md 文件表）。
// 本文件为纯数据 + 纯查询函数，零 DOM/React 依赖，供 EventTree / HandlerForm /
// JsonMode 导航 / MatcherTester 共用。

// handler 类型五档（C13-3 官方字段矩阵的 5 种类型）
export type HandlerType = "command" | "http" | "mcp_tool" | "prompt" | "agent";

export const HANDLER_TYPES: readonly HandlerType[] = [
  "command",
  "http",
  "mcp_tool",
  "prompt",
  "agent",
];

// handler 支持档（C13-4）：
//   A = 全 5 种（command/http/mcp_tool/prompt/agent）
//   B = command + http + mcp_tool
//   C = command + mcp_tool
export type HandlerSupportLevel = "A" | "B" | "C";

export const HANDLER_TYPES_BY_LEVEL: Record<HandlerSupportLevel, readonly HandlerType[]> = {
  A: HANDLER_TYPES,
  B: ["command", "http", "mcp_tool"],
  C: ["command", "mcp_tool"],
};

// 单事件元数据
export interface HookEventMeta {
  /** 事件名（settings.json hooks 键） */
  event: string;
  /** 所属分组 */
  group: string;
  /** 是否支持 matcher；false = GUI 省略 matcher 输入、guiToJson 省略 matcher 键 */
  supportsMatcher: boolean;
  /** matcher 匹配目标（工具名/notification_type/source/trigger 等）；不支持 matcher 时为 null */
  matcherTarget: string | null;
  /** handler 支持档（A/B/C） */
  handlerLevel: HandlerSupportLevel;
  /**
   * matcher 窄字符集受限（仅 FileChanged/StopFailure）：
   * 通常窄字符集 = 字母/数字/_/-/空格/|/,；受限时仅 字母/数字/_/|，
   * 连字符/空格/逗号强制走 JS 正则（C13-5）
   */
  restrictedMatcherCharset?: boolean;
}

// 10 个分组（顺序即「事件元数据目录」表格出现顺序）
export const EVENT_GROUPS: readonly string[] = [
  "会话生命周期",
  "用户交互",
  "工具调用",
  "通知与消息",
  "子代理与任务",
  "上下文管理",
  "停止与错误",
  "配置与文件变更",
  "工作树",
  "启发式交互",
];

// 30 事件 x 10 组完整映射（本文件为单一真值源，禁省略）
export const HOOK_EVENTS: readonly HookEventMeta[] = [
  // 会话生命周期（3）
  { event: "SessionStart", group: "会话生命周期", supportsMatcher: true, matcherTarget: "source（startup/resume/clear/compact）", handlerLevel: "C" },
  { event: "SessionEnd", group: "会话生命周期", supportsMatcher: true, matcherTarget: "reason", handlerLevel: "B" },
  { event: "Setup", group: "会话生命周期", supportsMatcher: true, matcherTarget: "触发标志", handlerLevel: "C" },
  // 用户交互（2）
  { event: "UserPromptSubmit", group: "用户交互", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  { event: "UserPromptExpansion", group: "用户交互", supportsMatcher: true, matcherTarget: "命令名称", handlerLevel: "A" },
  // 工具调用（6）
  { event: "PreToolUse", group: "工具调用", supportsMatcher: true, matcherTarget: "工具名", handlerLevel: "A" },
  { event: "PermissionRequest", group: "工具调用", supportsMatcher: true, matcherTarget: "工具名", handlerLevel: "A" },
  { event: "PermissionDenied", group: "工具调用", supportsMatcher: true, matcherTarget: "工具名", handlerLevel: "A" },
  { event: "PostToolUse", group: "工具调用", supportsMatcher: true, matcherTarget: "工具名", handlerLevel: "A" },
  { event: "PostToolUseFailure", group: "工具调用", supportsMatcher: true, matcherTarget: "工具名", handlerLevel: "A" },
  { event: "PostToolBatch", group: "工具调用", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  // 通知与消息（2）
  { event: "Notification", group: "通知与消息", supportsMatcher: true, matcherTarget: "notification_type", handlerLevel: "B" },
  // MessageDisplay：官方文档未明确其 handler 支持档，依据 D1 §6.7 默认超时表含
  // command/http/mcp_tool 保守推断为 B 档（prompt/agent 未核实，保守不展示）。
  // 执行期若官方文档明确，回改本表。
  { event: "MessageDisplay", group: "通知与消息", supportsMatcher: false, matcherTarget: null, handlerLevel: "B" },
  // 子代理与任务（5）
  { event: "SubagentStart", group: "子代理与任务", supportsMatcher: true, matcherTarget: "子代理类型名", handlerLevel: "B" },
  { event: "SubagentStop", group: "子代理与任务", supportsMatcher: true, matcherTarget: "子代理类型名", handlerLevel: "A" },
  { event: "TaskCreated", group: "子代理与任务", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  { event: "TaskCompleted", group: "子代理与任务", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  { event: "TeammateIdle", group: "子代理与任务", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  // 上下文管理（2）
  { event: "PreCompact", group: "上下文管理", supportsMatcher: true, matcherTarget: "manual/auto", handlerLevel: "B" },
  { event: "PostCompact", group: "上下文管理", supportsMatcher: true, matcherTarget: "manual/auto", handlerLevel: "B" },
  // 停止与错误（2）
  { event: "Stop", group: "停止与错误", supportsMatcher: false, matcherTarget: null, handlerLevel: "A" },
  { event: "StopFailure", group: "停止与错误", supportsMatcher: true, matcherTarget: "错误类型", handlerLevel: "B", restrictedMatcherCharset: true },
  // 配置与文件变更（4）
  { event: "ConfigChange", group: "配置与文件变更", supportsMatcher: true, matcherTarget: "配置来源", handlerLevel: "B" },
  { event: "CwdChanged", group: "配置与文件变更", supportsMatcher: false, matcherTarget: null, handlerLevel: "B" },
  { event: "FileChanged", group: "配置与文件变更", supportsMatcher: true, matcherTarget: "文件名模式（basename）", handlerLevel: "B", restrictedMatcherCharset: true },
  { event: "InstructionsLoaded", group: "配置与文件变更", supportsMatcher: true, matcherTarget: "加载原因", handlerLevel: "B" },
  // 工作树（2）
  { event: "WorktreeCreate", group: "工作树", supportsMatcher: false, matcherTarget: null, handlerLevel: "B" },
  { event: "WorktreeRemove", group: "工作树", supportsMatcher: false, matcherTarget: null, handlerLevel: "B" },
  // 启发式交互（2）
  { event: "Elicitation", group: "启发式交互", supportsMatcher: true, matcherTarget: "MCP 服务器名称", handlerLevel: "B" },
  { event: "ElicitationResult", group: "启发式交互", supportsMatcher: true, matcherTarget: "MCP 服务器名称", handlerLevel: "B" },
];

// 事件名索引（O(1) 查询）
const EVENT_INDEX = new Map<string, HookEventMeta>(
  HOOK_EVENTS.map((meta) => [meta.event, meta]),
);

// matcher 窄字符集受限事件（FileChanged / StopFailure，C13-5）
export const RESTRICTED_MATCHER_CHARSET_EVENTS: ReadonlySet<string> = new Set(
  HOOK_EVENTS.filter((m) => m.restrictedMatcherCharset).map((m) => m.event),
);

// ===== 纯查询函数 =====

/** 按事件名取元数据；未知事件返回 undefined */
export function getEventMeta(event: string): HookEventMeta | undefined {
  return EVENT_INDEX.get(event);
}

/** 事件是否支持 matcher（未知事件保守视为不支持） */
export function isMatcherSupported(event: string): boolean {
  return EVENT_INDEX.get(event)?.supportsMatcher ?? false;
}

/** 事件是否使用受限窄字符集（FileChanged/StopFailure，C13-5） */
export function hasRestrictedMatcherCharset(event: string | undefined): boolean {
  return event !== undefined && RESTRICTED_MATCHER_CHARSET_EVENTS.has(event);
}

/** 事件支持的 handler 类型列表（未知事件默认放行全 5 种） */
export function getSupportedHandlerTypes(event: string): readonly HandlerType[] {
  const level = EVENT_INDEX.get(event)?.handlerLevel ?? "A";
  return HANDLER_TYPES_BY_LEVEL[level];
}

/** 全部分组（EVENT_GROUPS 顺序） */
export function getGroups(): readonly string[] {
  return EVENT_GROUPS;
}

/** 某分组下的事件列表（按目录表顺序） */
export function getEventsByGroup(group: string): readonly HookEventMeta[] {
  return HOOK_EVENTS.filter((m) => m.group === group);
}

// ===== 5 种 handler 字段矩阵（C13-3 官方版） =====

export interface HandlerFieldDef {
  /** 字段名 */
  key: string;
  /** 是否必填（C13-3 标 * 的字段） */
  required: boolean;
  /** 字段值类型（供表单渲染） */
  kind: "string" | "stringArray" | "boolean" | "record" | "number";
}

// 各类型专有字段（C13-3：command=command*/args/async/asyncRewake/shell、
// http=url*/headers/allowedEnvVars、mcp_tool=server*/tool*/input、
// prompt=prompt*/model/continueOnBlock、agent=prompt*/model）
export const HANDLER_FIELD_MATRIX: Record<HandlerType, readonly HandlerFieldDef[]> = {
  command: [
    { key: "command", required: true, kind: "string" },
    { key: "args", required: false, kind: "stringArray" },
    { key: "async", required: false, kind: "boolean" },
    { key: "asyncRewake", required: false, kind: "boolean" },
    { key: "shell", required: false, kind: "string" },
  ],
  // http 无 method/body——固定 POST，body 恒为事件 JSON
  http: [
    { key: "url", required: true, kind: "string" },
    { key: "headers", required: false, kind: "record" },
    { key: "allowedEnvVars", required: false, kind: "stringArray" },
  ],
  // mcp_tool 字段名是 input（非 args）
  mcp_tool: [
    { key: "server", required: true, kind: "string" },
    { key: "tool", required: true, kind: "string" },
    { key: "input", required: false, kind: "record" },
  ],
  prompt: [
    { key: "prompt", required: true, kind: "string" },
    { key: "model", required: false, kind: "string" },
    { key: "continueOnBlock", required: false, kind: "boolean" },
  ],
  // agent 无 description/subagent_type——那是内置 Agent 工具的输入参数，非 hook handler 字段
  agent: [
    { key: "prompt", required: true, kind: "string" },
    { key: "model", required: false, kind: "string" },
  ],
};

// 通用字段（C13-3）：if 仅工具事件求值（PreToolUse/PostToolUse/PostToolUseFailure/
// PermissionRequest/PermissionDenied）；timeout 为超时秒数（数值）；
// once 不展示（settings.json 中无效）；asyncTimeout 是异步执行的返回值字段，
// 不是配置字段，均不收录
export const HANDLER_COMMON_FIELDS: readonly HandlerFieldDef[] = [
  { key: "if", required: false, kind: "string" },
  { key: "timeout", required: false, kind: "number" },
  { key: "statusMessage", required: false, kind: "string" },
];

/** 全部已知 handler 字段键（专有 + 通用，供 configModel 字段归类用） */
export const ALL_HANDLER_FIELD_KEYS: ReadonlySet<string> = new Set([
  ...HANDLER_TYPES.flatMap((t) => HANDLER_FIELD_MATRIX[t].map((f) => f.key)),
  ...HANDLER_COMMON_FIELDS.map((f) => f.key),
]);
