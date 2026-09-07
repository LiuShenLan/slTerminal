// hooks-json-schema-cm.test.ts — 自绘 hooks schema lint/hover 层纯函数直测（CP-002）
//
// 直测 jsonSchemaCm.ts 纯函数（pointerToRange/pathAt/resolveSchemaPath/
// lintHooksSchemaDoc/resolveHoverDescription）——组件侧 sentinel 断言见
// hooks-config-jsonmode.test.tsx（本文件与它互补，无 jsdom 依赖）。
// schema 模块保持真实（validateHooksJson/hooksSubSchema 为真值源）。

import { describe, it, expect } from "vitest";
import {
  pointerToRange,
  pathAt,
  resolveSchemaPath,
  lintHooksSchemaDoc,
  resolveHoverDescription,
} from "../features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm";
import { hooksSubSchema } from "../features/cliProfiles/profiles/claude/configEditor/schema";

/** 多事件嵌套文档——pointer 用例共用（含跨事件区分，验证逐段定位锚定目标键） */
const NESTED_DOC = [
  "{",
  '  "PreToolUse": [',
  '    { "hooks": [{ "type": "command", "command": "echo a" }] }',
  "  ],",
  '  "PostToolUse": [',
  '    { "hooks": [{ "type": "command", "command": "echo b" }] }',
  "  ]",
  "}",
].join("\n");

/** 单事件单行文档——pathAt 键链用例（回溯窗口内无跨事件键，链确定性收敛） */
const SINGLE_DOC = '{"PreToolUse": [{"hooks": [{"type": "command", "command": "echo a"}]}]}';

describe("pointerToRange", () => {
  it("pointerToRange_顶层键_定位到值区间", () => {
    const doc = '{"PreToolUse": []}';
    const range = pointerToRange(doc, "/PreToolUse");
    // from = 键引号对闭合后（冒号处）；to = 值数组闭合符 ']' 前
    expect(range).toEqual({
      from: doc.indexOf('"PreToolUse"') + '"PreToolUse"'.length,
      to: doc.indexOf("]"),
    });
  });

  it("pointerToRange_嵌套数组索引_收敛到目标行", () => {
    const doc = NESTED_DOC;
    const range = pointerToRange(doc, "/PostToolUse/0/hooks/0/type");
    expect(range).not.toBeNull();
    // 目标 type 键（PostToolUse 段内）——从/to 均落在该行区间
    const postToolUseIdx = doc.indexOf('"PostToolUse"');
    const typeIdx = doc.indexOf('"type"', postToolUseIdx);
    const lineStart = doc.lastIndexOf("\n", typeIdx) + 1;
    const lineEndIdx = doc.indexOf("\n", typeIdx);
    const lineEnd = lineEndIdx < 0 ? doc.length : lineEndIdx;
    expect(typeIdx).toBeGreaterThanOrEqual(0);
    expect(range!.from).toBeGreaterThanOrEqual(typeIdx);
    expect(range!.to).toBeGreaterThan(range!.from);
    expect(range!.from).toBeLessThanOrEqual(lineEnd);
    expect(range!.to).toBeLessThanOrEqual(lineEnd);
    expect(range!.from).toBeGreaterThanOrEqual(lineStart);
  });

  it("pointerToRange_未知键_返回null", () => {
    expect(pointerToRange(NESTED_DOC, "/NoSuchEvent")).toBeNull();
  });

  it("pointerToRange_空pointer_返回null", () => {
    expect(pointerToRange(NESTED_DOC, "")).toBeNull();
  });
});

describe("pathAt", () => {
  it("pathAt_嵌套位置_返回键序列", () => {
    // pos 落在最内层 command 值内——向前回溯键链（数组数字段不入路径）
    const pos = SINGLE_DOC.indexOf('"echo a"');
    expect(pathAt(SINGLE_DOC, pos)).toEqual([
      "PreToolUse",
      "hooks",
      "type",
      "command",
    ]);
  });

  it("pathAt_文档头_返回空数组", () => {
    expect(pathAt(NESTED_DOC, 0)).toEqual([]);
    expect(pathAt(NESTED_DOC, 1)).toEqual([]);
  });
});

describe("resolveSchemaPath", () => {
  it("resolveSchemaPath_事件键_命中hooks子schema", () => {
    const node = resolveSchemaPath(["PreToolUse"]);
    expect(node).not.toBeNull();
    expect(node!.description).toBe("Hooks that run before tool calls");
  });

  it("resolveSchemaPath_hookCommand$ref_解析出description", () => {
    // hooks 数组元素 items = {$ref: #/$defs/hookCommand}——resolveRef 解引用到 defs
    const node = resolveSchemaPath(["PreToolUse", "0", "hooks", "0"]);
    const defs = (hooksSubSchema as unknown as {
      $defs: { hookCommand: { anyOf: Array<{ description?: string }> } };
    }).$defs;
    expect(node).toBe(defs.hookCommand);
    const anyOf = (node as unknown as { anyOf?: Array<{ description?: string }> }).anyOf;
    expect(anyOf?.[0]?.description).toBe("Bash command hook");
  });

  it("resolveSchemaPath_未知键_返回null", () => {
    expect(resolveSchemaPath(["NoSuchEvent"])).toBeNull();
  });
});

describe("lintHooksSchemaDoc", () => {
  it("lintHooksSchemaDoc_合法配置_零诊断", () => {
    const doc =
      '{"PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}]}';
    expect(lintHooksSchemaDoc(doc)).toEqual([]);
  });

  it("lintHooksSchemaDoc_未知事件_单条error级诊断且message含Additional property", () => {
    const diags = lintHooksSchemaDoc('{"NoSuchEvent": []}');
    expect(diags).toHaveLength(1);
    expect(diags[0].severity).toBe("error");
    expect(diags[0].message).toContain("Additional property");
  });

  it("lintHooksSchemaDoc_语法错误_退回整文档区间(from=0,to=doc.length)", () => {
    const doc = "{bad json";
    const diags = lintHooksSchemaDoc(doc);
    expect(diags).toHaveLength(1);
    expect(diags[0].severity).toBe("error");
    expect(diags[0].message).toContain("JSON 语法错误");
    expect(diags[0].from).toBe(0);
    expect(diags[0].to).toBe(doc.length);
  });
});

describe("resolveHoverDescription", () => {
  it("resolveHoverDescription_事件键_返回description", () => {
    const doc = '{"PreToolUse": []}';
    // 悬停在 PreToolUse 值区间内 → 键链 [PreToolUse] → 事件 description
    const pos = doc.indexOf("]");
    expect(resolveHoverDescription(doc, pos)).toBe(
      "Hooks that run before tool calls",
    );
  });

  it("resolveHoverDescription_未知键路径_返回null", () => {
    const doc = '{"PreToolUse": [], "Surprise": 1}';
    // 悬停在未知键值内 → resolveSchemaPath 键链断（properties 未命中）→ null
    const pos = doc.indexOf("1");
    expect(resolveHoverDescription(doc, pos)).toBeNull();
  });
});
