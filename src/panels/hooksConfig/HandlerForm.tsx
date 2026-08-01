// HandlerForm.tsx — handler 编辑表单（P3-FE-14）
//
// 根据 type 渲染 5 种 handler 专用表单，字段矩阵照 C13-3 官方版：
// command=command*/args/async/asyncRewake/shell、http=url*/headers/allowedEnvVars
// （无 method/body——固定 POST）、mcp_tool=server*/tool*/input（字段名是 input 非 args）、
// prompt=prompt*/model/continueOnBlock、agent=prompt*/model（无 description/subagent_type），
// + 通用字段 if/timeout/statusMessage；once 不展示（settings.json 中忽略）。
// 字段清单来自 eventsCatalog 的 HANDLER_FIELD_MATRIX / HANDLER_COMMON_FIELDS（单一真值源，
// 不各自推断）。
//
// - 事件→handler 支持矩阵约束类型选择（eventsCatalog getSupportedHandlerTypes 驱动：
//   B 档事件禁用 prompt/agent；SessionStart/Setup 仅 command/mcp_tool）
// - 切换 type 保留通用字段（if/timeout/statusMessage）与适用于新类型的字段，清除不适用字段
// - 注入段（isSltermManaged 命中）handler 表单只读 + 禁删 + 禁禁用（C13-8）
// - matcher 输入框属 matcher 组级（GuiMode 渲染），本组件不渲染——不支持 matcher 的事件
//   由 GuiMode 按 eventsCatalog 标记省略 matcher 输入

import React, { useEffect, useState } from "react";
import {
  HANDLER_FIELD_MATRIX,
  HANDLER_COMMON_FIELDS,
  getSupportedHandlerTypes,
  type HandlerType,
  type HandlerFieldDef,
} from "./eventsCatalog";
import { isSltermManaged, type HookHandlerGui } from "./configModel";
import {
  INPUT_BG,
  INPUT_BORDER,
  BUTTON_FG,
  PLACEHOLDER_FG,
  ERROR_FG,
  SEPARATOR_BG,
  DIM_FG,
} from "../../theme";

/** HandlerForm props——handler + 所属事件 + 变更回调 */
export interface HandlerFormProps {
  /** 当前 handler（GUI 模型，未设置字段为 undefined，序列化时省略） */
  handler: HookHandlerGui;
  /** 所属事件名（驱动类型支持矩阵过滤，eventsCatalog） */
  event: string;
  /** 字段变更回调（传入完整新 handler 对象） */
  onChange: (next: HookHandlerGui) => void;
}

/** 类型中文标签（type selector 选项） */
const TYPE_LABELS: Record<HandlerType, string> = {
  command: "command（命令）",
  http: "http（HTTP 请求）",
  mcp_tool: "mcp_tool（MCP 工具）",
  prompt: "prompt（提示词）",
  agent: "agent（子代理）",
};

/** 字段中文标签 */
const FIELD_LABELS: Record<string, string> = {
  command: "命令",
  args: "参数（JSON 数组）",
  async: "异步模式",
  asyncRewake: "异步重新唤醒",
  shell: "Shell 解释器",
  url: "URL",
  headers: "请求头（JSON 对象）",
  allowedEnvVars: "允许的环境变量（JSON 数组）",
  server: "MCP Server",
  tool: "工具名",
  input: "输入参数（JSON 对象）",
  prompt: "提示词",
  model: "模型",
  continueOnBlock: "阻塞时继续",
  if: "条件过滤器（if）",
  timeout: "超时（秒）",
  statusMessage: "状态消息",
};

/** 字段占位提示 */
const FIELD_PLACEHOLDERS: Record<string, string> = {
  command: "要执行的命令，如 claude",
  args: '如 ["-p", "hello"]',
  shell: "如 bash / cmd（缺省走系统 shell）",
  url: "https://api.example.com",
  headers: '如 {"Authorization": "Bearer ..."}',
  allowedEnvVars: '如 ["HOME", "PATH"]',
  server: "MCP 服务器名称",
  tool: "工具名",
  input: '如 {"key": "value"}',
  prompt: "提示词内容",
  model: "模型名（缺省跟随 claude 配置）",
  if: "权限规则过滤器（仅工具事件求值）",
  timeout: "超时秒数",
  statusMessage: "运行期间自定义 spinner 文本",
};

// ===== 纯函数 =====

/**
 * 切换 handler type：保留通用字段（if/timeout/statusMessage）与适用于新类型的字段
 * （如 prompt↔agent 共有的 prompt/model），清除不适用字段（C13-3）；
 * extraFields（未知字段容错，round-trip 不丢数据）保留。
 */
export function switchHandlerType(
  prev: HookHandlerGui,
  newType: HandlerType,
): HookHandlerGui {
  const next: HookHandlerGui = { type: newType };
  const keep = new Set<string>([
    ...HANDLER_FIELD_MATRIX[newType].map((f) => f.key),
    ...HANDLER_COMMON_FIELDS.map((f) => f.key),
  ]);
  for (const key of keep) {
    const value = (prev as unknown as Record<string, unknown>)[key];
    if (value !== undefined) (next as unknown as Record<string, unknown>)[key] = value;
  }
  if (prev.extraFields) next.extraFields = { ...prev.extraFields };
  return next;
}

/** 值 → JSON 文本（textarea 显示用；undefined/null → 空串） */
function toJsonText(value: unknown): string {
  if (value === undefined || value === null) return "";
  return typeof value === "string" ? value : JSON.stringify(value);
}

/** 解析 JSON 数组文本；非法或非数组返回 undefined（不向父级上报） */
function parseJsonArray(text: string): unknown {
  const r = JSON.parse(text);
  return Array.isArray(r) ? r : undefined;
}

/** 解析 JSON 对象文本；非法或非对象返回 undefined（不向父级上报） */
function parseJsonRecord(text: string): unknown {
  const r = JSON.parse(text);
  return r !== null && typeof r === "object" && !Array.isArray(r) ? r : undefined;
}

/** 必填字段缺失判定（undefined/null/空串视为缺失） */
function isMissingRequired(handler: HookHandlerGui, key: string): boolean {
  const value = (handler as unknown as Record<string, unknown>)[key];
  if (value === undefined || value === null) return true;
  if (typeof value === "string" && value.trim() === "") return true;
  return false;
}

// ===== 样式（配色全部 theme/colors.ts token，硬约束 #6） =====

const formStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  padding: "8px 0",
};

const fieldRowStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: PLACEHOLDER_FG,
  userSelect: "none",
};

const labelErrorStyle: React.CSSProperties = {
  ...labelStyle,
  color: ERROR_FG,
};

const requiredStar: React.CSSProperties = {
  color: ERROR_FG,
  marginLeft: 2,
};

const inputBaseStyle: React.CSSProperties = {
  padding: "3px 6px",
  fontSize: 12,
  fontFamily: "inherit",
  background: INPUT_BG,
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 3,
  color: BUTTON_FG,
  outline: "none",
};

const typeSelectStyle: React.CSSProperties = {
  ...inputBaseStyle,
  width: "100%",
  cursor: "pointer",
};

const textareaStyle: React.CSSProperties = {
  ...inputBaseStyle,
  resize: "vertical",
  minHeight: 48,
  fontFamily: "Consolas, monospace",
};

const checkboxRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "2px 0",
};

const checkboxLabelStyle: React.CSSProperties = {
  fontSize: 12,
  color: BUTTON_FG,
  cursor: "pointer",
  userSelect: "none",
};

const sectionStyle: React.CSSProperties = {
  height: 1,
  background: SEPARATOR_BG,
  margin: "4px 0",
};

const errorTextStyle: React.CSSProperties = {
  fontSize: 10,
  color: ERROR_FG,
  marginTop: 1,
};

/** 「slTerminal 托管」标记样式 */
const managedBadgeStyle: React.CSSProperties = {
  fontSize: 10,
  color: BUTTON_FG,
  background: INPUT_BORDER,
  padding: "1px 6px",
  borderRadius: 3,
  alignSelf: "flex-start",
  userSelect: "none",
};

/** 注入段锁定行样式（禁删 + 禁禁用，C13-8） */
const lockRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "4px 0",
};

const lockButtonStyle: React.CSSProperties = {
  padding: "2px 10px",
  fontSize: 11,
  color: DIM_FG,
  background: "transparent",
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 3,
  cursor: "not-allowed",
};

// ===== 组件 =====

export const HandlerForm: React.FC<HandlerFormProps> = ({ handler, event, onChange }) => {
  // 注入段识别（C9 规则，configModel 单点判定）
  const managed = isSltermManaged(handler);

  // 事件支持矩阵过滤可选类型（eventsCatalog 驱动）；当前 type 不在支持列表时
  // 追加为选项——JSON 中与事件档位不匹配的 handler 不静默丢类型（round-trip 容错）
  const allowedTypes = getSupportedHandlerTypes(event);
  const typeOptions = allowedTypes.includes(handler.type)
    ? [...allowedTypes]
    : [...allowedTypes, handler.type];

  // 专有字段（C13-3 官方矩阵，声明顺序即渲染顺序）
  const fields = HANDLER_FIELD_MATRIX[handler.type];

  // record/stringArray 字段的编辑草稿——非法 JSON 输入不向父级上报（草稿保留待修，
  // 避免受控输入回弹）；type/event 变化（字段集合变化）时丢弃旧草稿
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  useEffect(() => {
    setDrafts({});
  }, [handler.type, event]);

  /** 更新单个字段；空值（空串/undefined）从对象中删除该键 */
  const handleFieldChange = (key: string, value: unknown) => {
    // 注入段锁定守卫（C13-8）：托管 handler 不产生任何变更（disabled 之外的逻辑层兜底）
    if (managed) return;
    const next: HookHandlerGui = { ...handler, [key]: value };
    if (value === "" || value === undefined) {
      delete (next as unknown as Record<string, unknown>)[key];
    }
    onChange(next);
  };

  /** type 切换——保留通用字段、清除不适用字段（switchHandlerType 纯函数） */
  const handleTypeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    if (managed) return;
    onChange(switchHandlerType(handler, e.target.value as HandlerType));
  };

  /** 渲染单个字段（按矩阵 kind 分派） */
  const renderField = (def: HandlerFieldDef) => {
    const key = def.key;
    const id = `hf-${key}`;
    const rawValue = (handler as unknown as Record<string, unknown>)[key];
    const label = FIELD_LABELS[key] ?? key;
    const missing = def.required && isMissingRequired(handler, key);
    const placeholder = FIELD_PLACEHOLDERS[key] ?? label;

    if (def.kind === "boolean") {
      return (
        <div key={key} style={checkboxRowStyle}>
          <input
            id={id}
            type="checkbox"
            checked={rawValue === true}
            disabled={managed}
            onChange={(e) => handleFieldChange(key, e.target.checked)}
            data-e2e={`handler-field-${key}`}
          />
          <label htmlFor={id} style={checkboxLabelStyle}>
            {label}
          </label>
        </div>
      );
    }

    if (def.kind === "record" || def.kind === "stringArray") {
      // 草稿优先（非法 JSON 输入保留待修），否则由 handler 值序列化
      const draft = drafts[key];
      const text = draft !== undefined ? draft : toJsonText(rawValue);
      const parse = def.kind === "record" ? parseJsonRecord : parseJsonArray;
      return (
        <div key={key} style={fieldRowStyle}>
          <label htmlFor={id} style={labelStyle}>
            {label}
          </label>
          <textarea
            id={id}
            disabled={managed}
            value={text}
            placeholder={placeholder}
            onChange={(e) => {
              const t = e.target.value;
              setDrafts((d) => ({ ...d, [key]: t }));
              if (t.trim() === "") {
                // 清空 → 删除该键
                handleFieldChange(key, undefined);
              } else {
                try {
                  const parsed = parse(t);
                  // 非法 JSON / 类型不符：不上报，草稿保留待修
                  if (parsed !== undefined) handleFieldChange(key, parsed);
                } catch {
                  // 非法 JSON：不上报，草稿保留待修
                }
              }
            }}
            style={textareaStyle}
            data-e2e={`handler-field-${key}`}
          />
        </div>
      );
    }

    if (def.kind === "number") {
      return (
        <div key={key} style={fieldRowStyle}>
          <label htmlFor={id} style={labelStyle}>
            {label}
          </label>
          <input
            id={id}
            type="number"
            disabled={managed}
            value={typeof rawValue === "number" ? rawValue : ""}
            placeholder={placeholder}
            onChange={(e) => {
              const t = e.target.value;
              if (t === "") {
                handleFieldChange(key, undefined);
              } else {
                const n = Number(t);
                if (!Number.isNaN(n)) handleFieldChange(key, n);
              }
            }}
            style={inputBaseStyle}
            data-e2e={`handler-field-${key}`}
          />
        </div>
      );
    }

    // string（含必填字段）
    return (
      <div key={key} style={fieldRowStyle}>
        <label htmlFor={id} style={missing ? labelErrorStyle : labelStyle}>
          {label}
          {def.required && <span style={requiredStar}>*</span>}
        </label>
        <input
          id={id}
          type="text"
          disabled={managed}
          value={typeof rawValue === "string" ? rawValue : ""}
          placeholder={placeholder}
          onChange={(e) => handleFieldChange(key, e.target.value)}
          style={inputBaseStyle}
          data-e2e={`handler-field-${key}`}
        />
        {missing && <span style={errorTextStyle}>此字段为必填</span>}
      </div>
    );
  };

  return (
    <div style={formStyle} data-e2e="handler-form">
      {/* 注入段标记（C13-8）：isSltermManaged 命中的 handler 显示「slTerminal 托管」 */}
      {managed && (
        <span data-e2e="handler-managed-badge" style={managedBadgeStyle}>
          slTerminal 托管
        </span>
      )}
      {/* type 选择器（事件支持矩阵过滤） */}
      <div style={fieldRowStyle}>
        <label htmlFor="hf-type" style={labelStyle}>
          类型
        </label>
        <select
          id="hf-type"
          data-e2e="handler-type-select"
          value={handler.type}
          disabled={managed}
          onChange={handleTypeChange}
          style={typeSelectStyle}
        >
          {typeOptions.map((t) => (
            <option key={t} value={t}>
              {TYPE_LABELS[t]}
            </option>
          ))}
        </select>
      </div>
      {/* 专有字段（C13-3 官方矩阵） */}
      {fields.map(renderField)}
      {/* 通用字段分隔线 + 通用字段（if/timeout/statusMessage） */}
      <div style={sectionStyle} />
      {HANDLER_COMMON_FIELDS.map(renderField)}
      {/* 注入段锁定行（C13-8）：托管 handler 禁删 + 禁禁用——不渲染禁用 checkbox
          （启停开关在事件树，P3-FE-19），仅渲染锁定态删除按钮；动作接线由 GuiMode 提供 */}
      {managed && (
        <div style={lockRowStyle}>
          <button
            type="button"
            data-e2e="handler-delete"
            disabled
            onClick={() => {
              // 托管条目删除恒为锁定态（动作接线在 GuiMode）
            }}
            style={lockButtonStyle}
          >
            删除
          </button>
        </div>
      )}
    </div>
  );
};
