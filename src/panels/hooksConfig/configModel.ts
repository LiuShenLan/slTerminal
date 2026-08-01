// configModel.ts — 配置模型双向转换（P3-FE-10）
//
// 输入/输出为 settings.json 的 hooks 子树（Record<事件名, MatcherGroupJson[]>），
// 与 src/types/hooksConfig.ts（P3-FE-06）定义的 HooksConfigJson / MatcherGroupJson /
// HookHandlerJson / HooksConfigGui / HookEventGroup / HookMatcherGroup /
// HookHandlerGui 契约同名镜像，结构兼容。
// 纯函数零 DOM/React，供 useHooksConfig / GuiMode / JsonMode / HandlerForm 共用。

import {
  HANDLER_TYPES,
  ALL_HANDLER_FIELD_KEYS,
  isMatcherSupported,
  getEventMeta,
  type HandlerType,
} from "./eventsCatalog";

// ===== 原始 JSON 类型（settings.json hooks 子树） =====

export type HooksConfigJson = Record<string, MatcherGroupJson[]>;

export interface MatcherGroupJson {
  /** matcher 表达式；事件不支持 matcher 或为空时省略该键 */
  matcher?: string;
  /** handler 列表（数组包裹恒保留） */
  hooks: HookHandlerJson[];
}

export interface HookHandlerJson {
  type: string;
  [key: string]: unknown;
}

// ===== GUI 模型类型 =====

export interface HooksConfigGui {
  events: HookEventGui[];
}

export interface HookEventGui {
  /** 事件名 */
  event: string;
  /** 所属分组（来自 eventsCatalog；未知事件归 UNKNOWN_EVENT_GROUP） */
  group: string;
  matcherGroups: HookMatcherGroupGui[];
}

export interface HookMatcherGroupGui {
  /** matcher 表达式；"" = 无 matcher（不支持 matcher 的事件省略该输入） */
  matcher: string;
  handlers: HookHandlerGui[];
}

// 5 种 handler 字段矩阵（C13-3 官方版），未设置的字段为 undefined（序列化时省略）
export interface HookHandlerGui {
  type: HandlerType;
  // command 专有
  command?: string;
  args?: string[];
  async?: boolean;
  asyncRewake?: boolean;
  shell?: string;
  // http 专有（无 method/body——固定 POST）
  url?: string;
  headers?: Record<string, string>;
  allowedEnvVars?: string[];
  // mcp_tool 专有（字段名是 input，非 args）
  server?: string;
  tool?: string;
  input?: Record<string, unknown>;
  // prompt 专有
  prompt?: string;
  model?: string;
  continueOnBlock?: boolean;
  // 通用字段
  if?: string;
  timeout?: number;
  statusMessage?: string;
  /** jsonToGui 保留的未知字段（多余字段容错，round-trip 不丢数据） */
  extraFields?: Record<string, unknown>;
}

/** 未知事件归组（jsonToGui 容错，round-trip 不丢事件） */
export const UNKNOWN_EVENT_GROUP = "未知事件";

const HANDLER_TYPES_SET = new Set<string>(HANDLER_TYPES);

// ===== jsonToGui：原始 JSON → GUI 模型 =====

/**
 * hooks 子树 → GUI 模型。
 * 对非对象顶层输入降级为空模型，不抛错；事件值非数组 / matcher 组非对象 /
 * handler 非对象或 type 非法均容错跳过。
 */
export function jsonToGui(json: unknown): HooksConfigGui {
  if (json === null || typeof json !== "object" || Array.isArray(json)) {
    return { events: [] };
  }
  const events: HookEventGui[] = [];
  for (const [event, rawGroups] of Object.entries(json)) {
    // 事件值非数组 → 容错跳过该事件
    if (!Array.isArray(rawGroups)) continue;
    const matcherGroups: HookMatcherGroupGui[] = [];
    for (const rawGroup of rawGroups) {
      if (rawGroup === null || typeof rawGroup !== "object" || Array.isArray(rawGroup)) {
        continue;
      }
      const group = rawGroup as Record<string, unknown>;
      const matcher = typeof group.matcher === "string" ? group.matcher : "";
      const handlers: HookHandlerGui[] = [];
      if (Array.isArray(group.hooks)) {
        for (const rawHandler of group.hooks) {
          const handler = toHandlerGui(rawHandler);
          if (handler) handlers.push(handler);
        }
      }
      matcherGroups.push({ matcher, handlers });
    }
    const meta = getEventMeta(event);
    events.push({
      event,
      group: meta?.group ?? UNKNOWN_EVENT_GROUP,
      matcherGroups,
    });
  }
  return { events };
}

/** 单条 handler JSON → GUI（type 缺失/非法/非对象 → null 容错跳过） */
function toHandlerGui(raw: unknown): HookHandlerGui | null {
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return null;
  const r = raw as Record<string, unknown>;
  const type = r.type;
  if (typeof type !== "string" || !HANDLER_TYPES_SET.has(type)) return null;
  const gui: HookHandlerGui = { type: type as HandlerType };
  for (const [key, value] of Object.entries(r)) {
    if (key === "type") continue;
    if (ALL_HANDLER_FIELD_KEYS.has(key)) {
      (gui as unknown as Record<string, unknown>)[key] = value;
    } else {
      // 未知字段保留到 extraFields，round-trip 不丢数据
      gui.extraFields ??= {};
      gui.extraFields[key] = value;
    }
  }
  return gui;
}

// ===== guiToJson：GUI 模型 → 原始 JSON =====

/**
 * GUI 模型 → hooks 子树。
 * 不支持 matcher 的事件省略 matcher 键但保留数组包裹；支持 matcher 的事件
 * matcher 为空串时同样省略该键（等价全匹配，C13-5）。
 */
export function guiToJson(gui: HooksConfigGui): HooksConfigJson {
  const out: HooksConfigJson = {};
  for (const eventGui of gui.events) {
    const groups: MatcherGroupJson[] = [];
    for (const groupGui of eventGui.matcherGroups) {
      const group: MatcherGroupJson = {
        hooks: groupGui.handlers.map(toHandlerJson),
      };
      // 仅支持 matcher 且非空时写 matcher 键
      if (isMatcherSupported(eventGui.event) && groupGui.matcher !== "") {
        group.matcher = groupGui.matcher;
      }
      groups.push(group);
    }
    out[eventGui.event] = groups;
  }
  return out;
}

/** 单条 GUI handler → JSON：type + 已定义字段（undefined 省略）+ extraFields */
function toHandlerJson(handler: HookHandlerGui): HookHandlerJson {
  const out: HookHandlerJson = { type: handler.type };
  for (const key of ALL_HANDLER_FIELD_KEYS) {
    const value = (handler as unknown as Record<string, unknown>)[key];
    if (value !== undefined) out[key] = value;
  }
  if (handler.extraFields) {
    for (const [key, value] of Object.entries(handler.extraFields)) {
      out[key] = value;
    }
  }
  return out;
}

// ===== isSltermManaged：注入段识别（C9） =====

/**
 * command 含 slterm-hook-reporter 子串判定（照 C9 识别规则）。
 * 供 GUI 标记「slTerminal 托管」+ 禁删/禁禁用/表单只读（C13-8）。
 */
export function isSltermManaged(handler: unknown): boolean {
  if (handler === null || typeof handler !== "object") return false;
  const command = (handler as { command?: unknown }).command;
  return typeof command === "string" && command.includes("slterm-hook-reporter");
}

