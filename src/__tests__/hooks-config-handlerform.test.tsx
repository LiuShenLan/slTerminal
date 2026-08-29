// hooks-config-handlerform.test.tsx — HandlerForm L2 测试（P3-TE-11）
//
// 覆盖：5 种 type 必填字段渲染（官方版字段名断言：mcp_tool 为 input、http 无
// method/body、agent 无 description/subagent_type）、事件支持矩阵过滤（eventsCatalog
// 驱动：B 档无 prompt/agent、SessionStart/Setup 仅 command/mcp_tool）、type 切换
// 清理字段（保留通用字段）、注入段禁改（只读+禁删，C13-8）。

import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { HandlerForm, switchHandlerType } from "../features/cliProfiles/profiles/claude/configEditor/HandlerForm";
import { HANDLER_TYPES } from "../features/cliProfiles/profiles/claude/configEditor/eventsCatalog";
import type { HookHandlerGui } from "../features/cliProfiles/profiles/claude/configEditor/configModel";

// ── 测试辅助 ──────────────────────────────────────────────

interface RenderFormOptions {
  type?: HookHandlerGui["type"];
  event?: string;
  overrides?: Partial<HookHandlerGui>;
}

/** 渲染 HandlerForm + 返回 onChange spy */
function renderForm(opts: RenderFormOptions = {}) {
  const { type = "command", event = "PreToolUse", overrides = {} } = opts;
  const handler: HookHandlerGui = { type, ...overrides };
  const onChange = vi.fn();
  const result = render(React.createElement(HandlerForm, { handler, event, onChange }));
  return { onChange, result, handler };
}

/** 取字段控件（data-e2e="handler-field-{key}"） */
function getField(key: string): HTMLElement {
  return document.querySelector(`[data-e2e="handler-field-${key}"]`) as HTMLElement;
}

/** 字段控件是否存在 */
function hasField(key: string): boolean {
  return document.querySelector(`[data-e2e="handler-field-${key}"]`) !== null;
}

/** type 选择器选项值列表 */
function getTypeOptions(): string[] {
  const select = document.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement;
  return [...select.options].map((o) => o.value);
}

/** 通用字段键（C13-3） */
const COMMON_KEYS = ["if", "timeout", "statusMessage"];

/** 注入段 handler（command 含 slterm-hook-reporter 子串，C9 识别规则） */
const MANAGED_HANDLER: HookHandlerGui = {
  type: "command",
  command: 'node "C:\\Users\\tester\\.slterminal\\hooks\\slterm-hook-reporter.js"',
  timeout: 5,
};

// ═══════════════════════════════════════════════════════════════
// switchHandlerType 纯函数——type 切换字段清理
// ═══════════════════════════════════════════════════════════════

describe("switchHandlerType——切换 type 保留通用字段、清除不适用字段", () => {
  it("command → http：清除 command/args/async/asyncRewake/shell，保留通用字段", () => {
    const next = switchHandlerType(
      { type: "command", command: "claude", args: ["-p", "hi"], async: true, asyncRewake: true, shell: "bash", timeout: 30, statusMessage: "运行中" },
      "http",
    );
    expect(next.type).toBe("http");
    expect(next.command).toBeUndefined();
    expect(next.args).toBeUndefined();
    expect(next.async).toBeUndefined();
    expect(next.asyncRewake).toBeUndefined();
    expect(next.shell).toBeUndefined();
    expect(next.timeout).toBe(30);
    expect(next.statusMessage).toBe("运行中");
  });

  it("http → mcp_tool：清除 url/headers/allowedEnvVars，保留通用字段", () => {
    const next = switchHandlerType(
      { type: "http", url: "https://api.example.com", headers: { Authorization: "Bearer x" }, allowedEnvVars: ["HOME"], timeout: 30 },
      "mcp_tool",
    );
    expect(next.type).toBe("mcp_tool");
    expect(next.url).toBeUndefined();
    expect(next.headers).toBeUndefined();
    expect(next.allowedEnvVars).toBeUndefined();
    expect(next.timeout).toBe(30);
  });

  it("agent → prompt：prompt/model 共有字段保留", () => {
    const next = switchHandlerType(
      { type: "agent", prompt: "帮我检查代码", model: "sonnet", timeout: 30 },
      "prompt",
    );
    expect(next.type).toBe("prompt");
    expect(next.prompt).toBe("帮我检查代码");
    expect(next.model).toBe("sonnet");
    expect(next.timeout).toBe(30);
  });

  it("command → agent：全部专有字段清除，仅剩通用字段", () => {
    const next = switchHandlerType(
      { type: "command", command: "claude", args: ["-p", "hi"], async: true, timeout: 30, if: "$hookEvent.exitCode == 0" },
      "agent",
    );
    expect(next.type).toBe("agent");
    expect(next.command).toBeUndefined();
    expect(next.args).toBeUndefined();
    expect(next.async).toBeUndefined();
    expect(next.timeout).toBe(30);
    expect(next.if).toBe("$hookEvent.exitCode == 0");
  });

  it("if/timeout/statusMessage 三个通用字段跨任意切换保留", () => {
    for (const newType of HANDLER_TYPES) {
      const next = switchHandlerType(
        { type: "command", command: "claude", if: "x", timeout: 5, statusMessage: "msg" },
        newType,
      );
      expect(next.if).toBe("x");
      expect(next.timeout).toBe(5);
      expect(next.statusMessage).toBe("msg");
    }
  });

  it("未设置字段不写入新对象（undefined 省略，序列化不污染）", () => {
    const next = switchHandlerType({ type: "command", command: "claude", timeout: 30 }, "http");
    const keys = Object.keys(next);
    expect(keys).not.toContain("command");
    expect(keys).not.toContain("async");
    expect(keys).toContain("timeout");
  });

  it("extraFields（未知字段容错）保留，round-trip 不丢数据", () => {
    const next = switchHandlerType(
      { type: "command", command: "claude", timeout: 30, extraFields: { customField: 1 } },
      "http",
    );
    expect(next.extraFields).toEqual({ customField: 1 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 5 种 type 字段渲染（C13-3 官方版字段名断言）
// ═══════════════════════════════════════════════════════════════

describe("5 种 type 字段渲染（C13-3 官方版字段名）", () => {
  afterEach(() => {
    cleanup();
  });

  it("command：command/args/async/asyncRewake/shell + 通用字段，无其他类型字段", () => {
    renderForm({ type: "command" });
    for (const key of ["command", "args", "async", "asyncRewake", "shell", ...COMMON_KEYS]) {
      expect(hasField(key)).toBe(true);
    }
    // 官方版：command 无 allowedEnvVars/url/server/tool/input/prompt/model/continueOnBlock
    for (const key of ["url", "headers", "allowedEnvVars", "server", "tool", "input", "prompt", "model", "continueOnBlock"]) {
      expect(hasField(key)).toBe(false);
    }
  });

  it("http：url/headers/allowedEnvVars + 通用字段，无 method/body（固定 POST，body 恒为事件 JSON）", () => {
    renderForm({ type: "http" });
    for (const key of ["url", "headers", "allowedEnvVars", ...COMMON_KEYS]) {
      expect(hasField(key)).toBe(true);
    }
    expect(hasField("method")).toBe(false);
    expect(hasField("body")).toBe(false);
    expect(hasField("command")).toBe(false);
  });

  it("mcp_tool：server/tool/input + 通用字段，字段名是 input 非 args", () => {
    renderForm({ type: "mcp_tool" });
    for (const key of ["server", "tool", "input", ...COMMON_KEYS]) {
      expect(hasField(key)).toBe(true);
    }
    expect(hasField("args")).toBe(false);
  });

  it("prompt：prompt/model/continueOnBlock + 通用字段", () => {
    renderForm({ type: "prompt" });
    for (const key of ["prompt", "model", "continueOnBlock", ...COMMON_KEYS]) {
      expect(hasField(key)).toBe(true);
    }
    expect(hasField("command")).toBe(false);
  });

  it("agent：prompt/model + 通用字段，无 description/subagent_type（那是内置 Agent 工具的输入参数）", () => {
    renderForm({ type: "agent" });
    for (const key of ["prompt", "model", ...COMMON_KEYS]) {
      expect(hasField(key)).toBe(true);
    }
    expect(hasField("description")).toBe(false);
    expect(hasField("subagent_type")).toBe(false);
    expect(hasField("continueOnBlock")).toBe(false);
  });

  it("5 种 type 均无 once 字段（settings.json 中忽略，C13-3）", () => {
    for (const type of HANDLER_TYPES) {
      renderForm({ type });
      expect(hasField("once")).toBe(false);
      cleanup();
    }
  });

  it("必填字段缺失显示「此字段为必填」提示（command 为空）", () => {
    renderForm({ type: "command", overrides: { command: "" } });
    expect(document.querySelector('[data-e2e="handler-field-command"]')).toBeTruthy();
    // 空串视为缺失（isMissingRequired 空串判定）
    const formText = document.querySelector('[data-e2e="handler-form"]')?.textContent ?? "";
    expect(formText).toContain("此字段为必填");
  });

  it("必填标记：http url 缺失同样提示（必填矩阵逐类型生效）", () => {
    renderForm({ type: "http" });
    const formText = document.querySelector('[data-e2e="handler-form"]')?.textContent ?? "";
    expect(formText).toContain("此字段为必填");
  });

  it("值回填：handler 值正确显示在控件上（command 文本 + timeout 数值）", () => {
    renderForm({ type: "command", overrides: { command: "claude", timeout: 30 } });
    expect((getField("command") as HTMLInputElement).value).toBe("claude");
    expect((getField("timeout") as HTMLInputElement).value).toBe("30");
  });

  it("record/stringArray 字段值回填为 JSON 文本", () => {
    renderForm({
      type: "command",
      overrides: { args: ["-p", "hello"], async: true },
    });
    expect((getField("args") as HTMLTextAreaElement).value).toBe('["-p","hello"]');
    expect((getField("async") as HTMLInputElement).checked).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════
// 事件支持矩阵过滤（eventsCatalog 驱动）
// ═══════════════════════════════════════════════════════════════

describe("事件 → handler 支持矩阵过滤（eventsCatalog 驱动）", () => {
  afterEach(() => {
    cleanup();
  });

  it("A 档事件（PreToolUse）type 选项为全部 5 种", () => {
    renderForm({ type: "command", event: "PreToolUse" });
    expect(getTypeOptions()).toEqual([...HANDLER_TYPES]);
  });

  it("B 档事件（SessionEnd）无 prompt/agent", () => {
    renderForm({ type: "command", event: "SessionEnd" });
    expect(getTypeOptions()).not.toContain("prompt");
    expect(getTypeOptions()).not.toContain("agent");
    expect(getTypeOptions()).toContain("command");
    expect(getTypeOptions()).toContain("http");
    expect(getTypeOptions()).toContain("mcp_tool");
  });

  it("B 档事件（Notification）无 prompt/agent", () => {
    renderForm({ type: "command", event: "Notification" });
    expect(getTypeOptions()).not.toContain("prompt");
    expect(getTypeOptions()).not.toContain("agent");
  });

  it("C 档事件（SessionStart）仅 command/mcp_tool", () => {
    renderForm({ type: "command", event: "SessionStart" });
    expect(getTypeOptions()).toEqual(["command", "mcp_tool"]);
  });

  it("C 档事件（Setup）仅 command/mcp_tool", () => {
    renderForm({ type: "command", event: "Setup" });
    expect(getTypeOptions()).toEqual(["command", "mcp_tool"]);
  });

  it("未知事件兜底 A 档（全部 5 种）", () => {
    renderForm({ type: "command", event: "UnknownEvent" });
    expect(getTypeOptions()).toEqual([...HANDLER_TYPES]);
  });

  it("当前 type 不在事件支持列表时仍显示为选项（round-trip 不静默丢类型）", () => {
    // JSON 中 SessionEnd（B 档）下存在 prompt handler 的数据不一致场景
    renderForm({ type: "prompt", event: "SessionEnd" });
    expect(getTypeOptions()).toContain("prompt");
  });
});

// ═══════════════════════════════════════════════════════════════
// type 切换（组件交互）
// ═══════════════════════════════════════════════════════════════

describe("type 切换交互", () => {
  afterEach(() => {
    cleanup();
  });

  it("切换 type → onChange 收到 switchHandlerType 结果（timeout 保留、command 清除）", () => {
    const { onChange } = renderForm({
      type: "command",
      overrides: { command: "claude", timeout: 30 },
    });
    const select = document.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "http" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.type).toBe("http");
    expect(next.timeout).toBe(30);
    expect(next.command).toBeUndefined();
  });
});

// ═══════════════════════════════════════════════════════════════
// 字段编辑
// ═══════════════════════════════════════════════════════════════

describe("字段编辑", () => {
  afterEach(() => {
    cleanup();
  });

  it("修改 command → onChange 收到新值", () => {
    const { onChange } = renderForm({ type: "command", overrides: { command: "claude" } });
    fireEvent.change(getField("command"), { target: { value: "echo hi" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.command).toBe("echo hi");
    expect(next.type).toBe("command");
  });

  it("清空 command → 键被删除（空串不写入）", () => {
    const { onChange } = renderForm({ type: "command", overrides: { command: "claude" } });
    fireEvent.change(getField("command"), { target: { value: "" } });
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect("command" in next).toBe(false);
  });

  it("args 合法 JSON 数组 → onChange 收到数组", () => {
    const { onChange } = renderForm({ type: "command" });
    fireEvent.change(getField("args"), { target: { value: '["-p", "hello"]' } });
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.args).toEqual(["-p", "hello"]);
  });

  it("args 清空 → 键被删除（非置空，HKC-04）", () => {
    const { onChange } = renderForm({
      type: "command",
      overrides: { args: ["-p", "hello"] },
    });
    fireEvent.change(getField("args"), { target: { value: "" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect("args" in next).toBe(false);
  });

  it("args 非法 JSON → 不触发 onChange（草稿保留待修，无回弹）", () => {
    const { onChange } = renderForm({ type: "command" });
    fireEvent.change(getField("args"), { target: { value: '["-p", ' } });
    expect(onChange).not.toHaveBeenCalled();
    expect((getField("args") as HTMLTextAreaElement).value).toBe('["-p", ');
  });

  it("headers record 合法 JSON → onChange 收到对象（http 型）", () => {
    const { onChange } = renderForm({ type: "http" });
    fireEvent.change(getField("headers"), {
      target: { value: '{"Authorization": "Bearer x"}' },
    });
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.headers).toEqual({ Authorization: "Bearer x" });
  });

  it("headers 清空 → 键被删除（非置空，HKC-04）", () => {
    const { onChange } = renderForm({
      type: "http",
      overrides: { headers: { Authorization: "Bearer x" } },
    });
    fireEvent.change(getField("headers"), { target: { value: "" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect("headers" in next).toBe(false);
  });

  it("timeout 数字输入 → onChange 收到 number", () => {
    const { onChange } = renderForm({ type: "command" });
    fireEvent.change(getField("timeout"), { target: { value: "30" } });
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.timeout).toBe(30);
  });

  it("timeout 清空 → 键被删除", () => {
    const { onChange } = renderForm({ type: "command", overrides: { timeout: 30 } });
    fireEvent.change(getField("timeout"), { target: { value: "" } });
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect("timeout" in next).toBe(false);
  });

  it("async checkbox 勾选 → onChange 收到 true", () => {
    const { onChange } = renderForm({ type: "command" });
    fireEvent.click(getField("async"));
    const next = onChange.mock.calls[0][0] as HookHandlerGui;
    expect(next.async).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════
// 注入段禁改（C13-8）
// ═══════════════════════════════════════════════════════════════

describe("注入段 handler 禁改（isSltermManaged，C13-8）", () => {
  afterEach(() => {
    cleanup();
  });

  it("托管 handler 显示「slTerminal 托管」标记", () => {
    renderForm({ type: "command", overrides: { command: MANAGED_HANDLER.command as string } });
    expect(document.querySelector('[data-e2e="handler-managed-badge"]')?.textContent).toBe(
      "slTerminal 托管",
    );
  });

  it("托管 handler 表单只读：type 选择器 + 文本字段 + checkbox 全部 disabled", () => {
    renderForm({
      type: "command",
      overrides: { command: MANAGED_HANDLER.command as string, async: true, timeout: 5 },
    });
    expect((document.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement).disabled).toBe(true);
    expect((getField("command") as HTMLInputElement).disabled).toBe(true);
    expect((getField("timeout") as HTMLInputElement).disabled).toBe(true);
    expect((getField("async") as HTMLInputElement).disabled).toBe(true);
  });

  it("托管 handler 禁删：删除按钮 disabled（C13-8）", () => {
    renderForm({
      type: "command",
      overrides: { command: MANAGED_HANDLER.command as string },
    });
    expect((document.querySelector('[data-e2e="handler-delete"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it("托管 handler 编辑不触发 onChange（表单完全锁定）", () => {
    const { onChange } = renderForm({
      type: "command",
      overrides: { command: MANAGED_HANDLER.command as string },
    });
    fireEvent.change(getField("command"), { target: { value: "echo hacked" } });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("非托管 handler：无托管标记、输入可用、无锁定行", () => {
    renderForm({ type: "command", overrides: { command: "claude" } });
    expect(document.querySelector('[data-e2e="handler-managed-badge"]')).toBeNull();
    expect((getField("command") as HTMLInputElement).disabled).toBe(false);
    expect(document.querySelector('[data-e2e="handler-delete"]')).toBeNull();
  });
});
