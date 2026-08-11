// schema/index.ts — SchemaStore 官方 schema 内嵌 + hooks 子 schema 提取（P3-FE-07）
//
// 数据源：SchemaStore https://json.schemastore.org/claude-code-settings.json
// （2026-08-01 下载，版本随 slTerminal 发布更新——升级时整文件替换本目录
// claude-code-settings.json 即可，离线可用、无网络请求）。
//
// 【自包含性核实结论（2026-08-01）】
// 全 schema 无远程 $ref：35 个本地 $ref 全部指向 `#/$defs/*`
// （$defs 仅 permissionRule / hookCommand / hookMatcher 三键，hookMatcher → hookCommand）。
// codemirror-json-schema / json-schema-library 仅支持本地 $ref，无需预打包展开远程引用。
//
// hooks 子 schema 提取策略：`properties.hooks` + 打包其依赖的 $defs 子集
// （hookMatcher + hookCommand，不含 permissions 专用的 permissionRule），
// 保证 `#/$defs/hookMatcher` 本地引用在独立 schema 中可解析。
// 供 JsonMode（悬停/波浪线）与 Stage 06 保存校验共用——对齐 hooks 子树编辑范围。

import { Draft07 } from "json-schema-library";
import type { JsonError, JsonSchema } from "json-schema-library";
import claudeCodeSettingsSchema from "./claude-code-settings.json";

/** 主 schema（完整 settings.json schema，供未来扩展/其他属性校验用） */
export const claudeCodeSettings = claudeCodeSettingsSchema;

/** 主 schema 版本指纹：$id 中含日期/版本（随 SchemaStore 更新） */
export const SCHEMA_ID: string = claudeCodeSettingsSchema.$id;

/** hooks 子 schema：properties.hooks + 依赖的 $defs 子集（自包含，本地 $ref 可解析） */
export const hooksSubSchema = {
  ...claudeCodeSettingsSchema.properties.hooks,
  $defs: {
    hookCommand: claudeCodeSettingsSchema.$defs.hookCommand,
    hookMatcher: claudeCodeSettingsSchema.$defs.hookMatcher,
  },
};

/** 校验用 Draft07 单例（schema 固定不变，复用避免重复编译开销） */
const hooksDraft = new Draft07(hooksSubSchema as JsonSchema);

// ===== 保存前校验（JsonMode 波浪线 / Stage 06 保存路径共用） =====

/** 单条诊断信息（供 onValidationChange 与保存提示展示） */
export interface JsonDiagnostic {
  /** 错误信息 */
  message: string;
  /** JSON Pointer 定位（如 /PostToolUse/0/hooks/0/type）；语法错误时为空串 */
  pointer: string;
}

/** JSON 文本校验结果 */
export interface JsonValidationResult {
  /** 是否合法：JSON 可解析 且 schema 校验通过 */
  isValid: boolean;
  /** 诊断列表（非法 JSON 时含解析错误） */
  diagnostics: JsonDiagnostic[];
}

/**
 * 校验 hooks 子树 JSON 文本（非 ajv，json-schema-library Draft07）：
 * ① JSON.parse 语法校验 → ② hooks 子 schema 校验（additionalProperties: false 拦未知事件）。
 */
export function validateHooksJson(text: string): JsonValidationResult {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch (err) {
    return {
      isValid: false,
      diagnostics: [{ message: `JSON 语法错误：${(err as Error).message}`, pointer: "" }],
    };
  }
  const errors = hooksDraft.validate(data) as unknown as JsonError[];
  return {
    isValid: errors.length === 0,
    diagnostics: errors.map((e) => ({
      message: e.message,
      pointer: String(e.dataPath ?? e.pointer ?? ""),
    })),
  };
}
