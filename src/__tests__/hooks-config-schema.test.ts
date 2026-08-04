// hooks-config-schema.test.ts — validateHooksJson 直接边界测试（HKC-08，11 Y5）
//
// 直测 schema/index.ts 的 validateHooksJson（JSON.parse 语法 + hooks 子 schema 校验）：
// 合法 / 缺 hooks 键 / 非法 matcher / 未知事件告警边界（handler type 不在枚举、
// command 缺必填 command、http 缺必填 url）/ 顶层数组拒绝 / 空对象合法通过 / 语法错误。
// 与 JsonMode/保存路径的间接覆盖互补——子 schema 提取错误（additionalProperties 放宽、
// required 丢失）在此精确定位。
//
// 纯函数测试：无 mock、无 jsdom、无 React，直接调用断言（照 path.test.ts 模式）。

import { describe, it, expect } from "vitest";
import { validateHooksJson } from "../features/hooksConfig/schema";

/** 基线合法 hooks 子树（command handler） */
const VALID_CONFIG = {
  PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
};

describe("validateHooksJson 直接边界（HKC-08）", () => {
  it("合法 hooks 子树通过（isValid=true 且零诊断）", () => {
    const r = validateHooksJson(JSON.stringify(VALID_CONFIG));
    expect(r.isValid).toBe(true);
    expect(r.diagnostics).toEqual([]);
  });

  it("空对象合法通过（无 hooks 配置 = 合法空子树）", () => {
    const r = validateHooksJson("{}");
    expect(r.isValid).toBe(true);
    expect(r.diagnostics).toEqual([]);
  });

  it("matcher 组缺必填 hooks 键拒绝", () => {
    const r = validateHooksJson(JSON.stringify({ PreToolUse: [{ matcher: "Edit" }] }));
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
    // 诊断含缺失字段名（hookMatcher required: ["hooks"]）
    expect(r.diagnostics.some((d) => d.message.includes("hooks"))).toBe(true);
  });

  it("非法 matcher（非字符串）拒绝", () => {
    const r = validateHooksJson(
      JSON.stringify({
        PreToolUse: [{ matcher: 123, hooks: [{ type: "command", command: "x" }] }],
      }),
    );
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it("未知事件键拒绝（additionalProperties: false 拦未知事件）", () => {
    const r = validateHooksJson(
      JSON.stringify({ NotARealEvent: [{ hooks: [{ type: "command", command: "x" }] }] }),
    );
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.some((d) => d.message.includes("Additional property"))).toBe(true);
  });

  it("handler type 不在枚举（anyOf 五种全不匹配）拒绝", () => {
    const r = validateHooksJson(
      JSON.stringify({ PreToolUse: [{ hooks: [{ type: "watcher", command: "x" }] }] }),
    );
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it("command handler 缺必填 command 拒绝", () => {
    const r = validateHooksJson(
      JSON.stringify({ PreToolUse: [{ hooks: [{ type: "command" }] }] }),
    );
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it("http handler 缺必填 url 拒绝", () => {
    const r = validateHooksJson(JSON.stringify({ PreToolUse: [{ hooks: [{ type: "http" }] }] }));
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it("顶层数组拒绝（hooks 子树必须是对象）", () => {
    const r = validateHooksJson(JSON.stringify([{ PreToolUse: VALID_CONFIG.PreToolUse }]));
    expect(r.isValid).toBe(false);
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it("非法 JSON 文本 → 语法错误诊断（parse 分支，pointer 为空串）", () => {
    const r = validateHooksJson('{ "PreToolUse": ');
    expect(r.isValid).toBe(false);
    expect(r.diagnostics[0].message).toContain("JSON 语法错误");
    expect(r.diagnostics[0].pointer).toBe("");
  });
});
