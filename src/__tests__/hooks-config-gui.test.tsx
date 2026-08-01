// hooks-config-gui.test.tsx — GuiMode / EventTree L2 测试（P3-TE-12）
//
// 覆盖：十大分组渲染（含折叠）、hook 计数、选中回调（树选中 → 详情联动 +
// 高亮 token）、添加/删除事件、注入段标记与禁删（C13-8 三层禁删）、
// 不支持 matcher 事件无 matcher 输入、matcher 组 / handler 增删与 matcher 值更新。
//
// 纯 React 组件测试：GuiMode 无 IPC/store/CM6 依赖，直接渲染真实组件，
// 用带状态 Harness 模拟父层受控（onChange → setGui 重渲染，对齐生产链路）。

import { describe, it, expect, afterEach, vi } from "vitest";
import React, { useState } from "react";
import { render, cleanup, fireEvent } from "@testing-library/react";
import GuiMode from "../panels/hooksConfig/GuiMode";
import type { HooksConfigGui, HookEventGui, HookHandlerGui } from "../panels/hooksConfig/configModel";
import { UNKNOWN_EVENT_GROUP } from "../panels/hooksConfig/configModel";
import { EVENT_GROUPS, getEventMeta, getSupportedHandlerTypes } from "../panels/hooksConfig/eventsCatalog";
import { ACTIVE_SELECTION_BG, FOCUS_BORDER, INPUT_BORDER } from "../theme";

/** hex 色值 → jsdom rgb 形态（照 activityBar.test.tsx 先例，jsdom 统一归一化） */
function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

/** 构造单事件 GUI 条目（group 从 eventsCatalog 推导，未知事件归未知组） */
function makeEvent(
  event: string,
  groups: Array<{ matcher?: string; handlers: Array<Partial<HookHandlerGui> & { type: HookHandlerGui["type"] }> }>,
): HookEventGui {
  return {
    event,
    group: getEventMeta(event)?.group ?? UNKNOWN_EVENT_GROUP,
    matcherGroups: groups.map((g) => ({
      matcher: g.matcher ?? "",
      handlers: g.handlers as HookHandlerGui[],
    })),
  };
}

/** 注入段 handler（command 含 slterm-hook-reporter，C9 识别规则） */
const managedHandler: HookHandlerGui = {
  type: "command",
  command: 'node "C:\\Users\\me\\.slterminal\\hooks\\slterm-hook-reporter.js"',
  timeout: 5,
};

/** 常用测试配置：PreToolUse（2 组 3 handler）+ SessionStart（1 组 1 handler） */
function makeDefaultGui(): HooksConfigGui {
  return {
    events: [
      makeEvent("PreToolUse", [
        {
          matcher: "Bash",
          handlers: [
            { type: "command", command: "echo hi" },
            { type: "http", url: "https://example.com/hook" },
          ],
        },
        { matcher: "", handlers: [{ type: "prompt", prompt: "confirm?" }] },
      ]),
      makeEvent("SessionStart", [{ handlers: [{ type: "mcp_tool", server: "srv", tool: "tool" }] }]),
    ],
  };
}

/** 受控 Harness：onChange 后 setGui 重渲染（模拟父层 updateConfigJson → jsonToGui 回灌） */
function Harness({ initialGui, onChange }: { initialGui: HooksConfigGui; onChange: (g: HooksConfigGui) => void }) {
  const [gui, setGui] = useState(initialGui);
  return React.createElement(GuiMode, {
    gui,
    onChange: (g) => {
      onChange(g);
      setGui(g);
    },
  });
}

function renderGui(initialGui: HooksConfigGui, onChange = () => {}) {
  return render(React.createElement(Harness, { initialGui, onChange }));
}

/** 事件行文本（含 hook 计数） */
function eventRowText(container: HTMLElement, event: string): string {
  const row = container.querySelector(`[data-e2e="gui-event-${event}"]`);
  if (!row) return "";
  return row.textContent ?? "";
}

/** 选中态断言辅助：事件行背景 = ACTIVE_SELECTION_BG token（jsdom rgb 形态） */
function selectedBg(container: HTMLElement, selector: string): string | null {
  const el = container.querySelector(selector) as HTMLElement | null;
  return el ? el.style.background : null;
}

describe("P3-TE-12 事件树结构（EventTree）", () => {
  afterEach(() => {
    cleanup();
  });

  it("渲染十大分组标题（EVENT_GROUPS 驱动，空配置也全量显示）", () => {
    // 空配置：全部 10 组标题 + 无配置提示
    const { container } = renderGui({ events: [] });
    for (const group of EVENT_GROUPS) {
      expect(container.querySelector(`[data-e2e="gui-group-head-${group}"]`)).toBeTruthy();
      expect(container.textContent).toContain(group);
    }
    expect(EVENT_GROUPS.length).toBe(10);
    expect(container.textContent).toContain("(无配置)");
  });

  it("三级树渲染：分组 → 事件名 → matcher 组 → handler 摘要", () => {
    const { container } = renderGui(makeDefaultGui());
    // 事件行存在
    expect(container.querySelector('[data-e2e="gui-event-PreToolUse"]')).toBeTruthy();
    // matcher 组行：空 matcher → 全匹配；非空 → matcher: 表达式
    expect(container.textContent).toContain("matcher: Bash");
    expect(container.textContent).toContain("全匹配");
    // handler 摘要（type + 主字段）
    expect(container.textContent).toContain("command: echo hi");
    expect(container.textContent).toContain("http: https://example.com/hook");
    expect(container.textContent).toContain("mcp_tool: srv/tool");
  });

  it("hook 计数：事件行显示跨 matcher 组求和的 handler 总数", () => {
    const { container } = renderGui(makeDefaultGui());
    // PreToolUse = 组0(2) + 组1(1) = 3；SessionStart = 1
    expect(eventRowText(container, "PreToolUse")).toContain("(3)");
    expect(eventRowText(container, "SessionStart")).toContain("(1)");
  });

  it("分组可折叠：点击分组标题隐藏/恢复该组事件", () => {
    const { container } = renderGui(makeDefaultGui());
    const head = container.querySelector('[data-e2e="gui-group-head-工具调用"]') as HTMLElement;
    expect(container.querySelector('[data-e2e="gui-event-PreToolUse"]')).toBeTruthy();
    fireEvent.click(head);
    expect(container.querySelector('[data-e2e="gui-event-PreToolUse"]')).toBeNull();
    fireEvent.click(head);
    expect(container.querySelector('[data-e2e="gui-event-PreToolUse"]')).toBeTruthy();
  });
});

describe("P3-TE-12 选中回调与详情联动", () => {
  afterEach(() => {
    cleanup();
  });

  it("点击事件行 → 详情区显示该事件 + 选中态高亮（token）", () => {
    const { container } = renderGui(makeDefaultGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    // 详情区标题
    expect(container.querySelector('[data-e2e="gui-detail-event"]')?.textContent).toBe("PreToolUse");
    // 选中行背景 = ACTIVE_SELECTION_BG，未选中行透明
    expect(selectedBg(container, '[data-e2e="gui-event-PreToolUse"]')).toBe(hexToRgb(ACTIVE_SELECTION_BG));
    expect(selectedBg(container, '[data-e2e="gui-event-SessionStart"]')).toBe("transparent");
  });

  it("点击 matcher 组头行 → 组框选中高亮（FOCUS_BORDER token）；点击 handler 行 → 行高亮", () => {
    const { container } = renderGui(makeDefaultGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    const groupBox = container.querySelector('[data-e2e="gui-group-PreToolUse-0"]') as HTMLElement;
    expect(groupBox.style.borderColor).toBe(hexToRgb(INPUT_BORDER));
    fireEvent.click(container.querySelector('[data-e2e="gui-matcher-PreToolUse-0"]')?.parentElement as HTMLElement);
    expect(groupBox.style.borderColor).toBe(hexToRgb(FOCUS_BORDER));
    // handler 行选中 → 背景高亮
    fireEvent.click(container.querySelector('[data-e2e="gui-handler-PreToolUse-0-0"]') as HTMLElement);
    expect(selectedBg(container, '[data-e2e="gui-handler-PreToolUse-0-0"]')).toBe(hexToRgb(ACTIVE_SELECTION_BG));
  });

  it("未选中事件时详情区显示引导文案", () => {
    const { container } = renderGui(makeDefaultGui());
    expect(container.textContent).toContain("在左侧选择已配置事件");
  });
});

describe("P3-TE-12 添加/删除事件", () => {
  afterEach(() => {
    cleanup();
  });

  it("添加事件：下拉选择未配置事件 → 回调含新事件（空 matcher 组）+ 自动选中", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.change(container.querySelector('[data-e2e="gui-add-event-select"]') as HTMLSelectElement, {
      target: { value: "UserPromptSubmit" },
    });
    fireEvent.click(container.querySelector('[data-e2e="gui-add-event"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    const added = last.events.find((e) => e.event === "UserPromptSubmit");
    expect(added).toBeTruthy();
    expect(added?.matcherGroups).toEqual([]);
    expect(added?.group).toBe(getEventMeta("UserPromptSubmit")?.group);
    // 自动选中：详情区标题变为新事件
    expect(container.querySelector('[data-e2e="gui-detail-event"]')?.textContent).toBe("UserPromptSubmit");
    // 树中新增行
    expect(container.querySelector('[data-e2e="gui-event-UserPromptSubmit"]')).toBeTruthy();
  });

  it("添加事件：已配置事件不出现在候选下拉", () => {
    const { container } = renderGui(makeDefaultGui());
    const select = container.querySelector('[data-e2e="gui-add-event-select"]') as HTMLSelectElement;
    const options = Array.from(select.options).map((o) => o.value);
    expect(options).toContain("UserPromptSubmit");
    expect(options).not.toContain("PreToolUse");
    expect(options).not.toContain("SessionStart");
  });

  it("删除事件：回调不含该事件 + 详情区回退空态", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-del"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events.find((e) => e.event === "PreToolUse")).toBeUndefined();
    expect(last.events.some((e) => e.event === "SessionStart")).toBe(true);
    // 选中事件被删 → 空态
    expect(container.textContent).toContain("在左侧选择已配置事件");
  });
});

describe("P3-TE-12 注入段标记与禁删（C13-8）", () => {
  afterEach(() => {
    cleanup();
  });

  const managedGui = (): HooksConfigGui => ({
    events: [
      makeEvent("PreToolUse", [
        {
          matcher: "",
          handlers: [managedHandler, { type: "command", command: "echo user-hook" }],
        },
      ]),
    ],
  });

  it("注入段 handler 显示「slTerminal 托管」标记（树摘要 + 详情行）", () => {
    const { container } = renderGui(managedGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    expect(container.querySelectorAll("[data-e2e^='gui-tree-handler-']").length).toBe(2);
    // 树中托管标记
    const tree = container.querySelector('[data-e2e="hooks-event-tree"]') as HTMLElement;
    expect(tree.textContent).toContain("slTerminal 托管");
    // 详情中托管标记
    expect(container.textContent).toContain("slTerminal 托管");
  });

  it("托管 handler 删除按钮禁用；普通 handler 可删", () => {
    const onChange = vi.fn();
    const { container } = renderGui(managedGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    const managedDel = container.querySelector('[data-e2e="gui-handler-del-PreToolUse-0-0"]') as HTMLButtonElement;
    const normalDel = container.querySelector('[data-e2e="gui-handler-del-PreToolUse-0-1"]') as HTMLButtonElement;
    expect(managedDel.disabled).toBe(true);
    expect(normalDel.disabled).toBe(false);
    // 删除普通 handler → 回调保留托管条目
    fireEvent.click(normalDel);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    const group = last.events[0].matcherGroups[0];
    expect(group.handlers).toHaveLength(1);
    expect(group.handlers[0].command).toContain("slterm-hook-reporter");
  });

  it("含托管 handler 的 matcher 组删除禁用；事件删除禁用", () => {
    const { container } = renderGui(managedGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    expect(
      (container.querySelector('[data-e2e="gui-group-del-PreToolUse-0"]') as HTMLButtonElement).disabled,
    ).toBe(true);
    expect((container.querySelector('[data-e2e="gui-event-del"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it("无托管条目的事件删除按钮可用", () => {
    const { container } = renderGui(makeDefaultGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    expect((container.querySelector('[data-e2e="gui-event-del"]') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("P3-TE-12 matcher 输入与 matcher 组操作", () => {
  afterEach(() => {
    cleanup();
  });

  it("不支持 matcher 事件（UserPromptSubmit）无 matcher 输入框", () => {
    const { container } = renderGui({
      events: [makeEvent("UserPromptSubmit", [{ handlers: [{ type: "command", command: "x" }] }])],
    });
    fireEvent.click(container.querySelector('[data-e2e="gui-event-UserPromptSubmit"]') as HTMLElement);
    expect(container.querySelector('[data-e2e^="gui-matcher-"]')).toBeNull();
    expect(container.textContent).toContain("全匹配（该事件不支持 matcher）");
  });

  it("支持 matcher 事件（PreToolUse）渲染 matcher 输入框；输入更新回调 matcher 值", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    const input = container.querySelector('[data-e2e="gui-matcher-PreToolUse-0"]') as HTMLInputElement;
    expect(input).toBeTruthy();
    expect(input.value).toBe("Bash");
    fireEvent.change(input, { target: { value: "Bash|Write" } });
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events[0].matcherGroups[0].matcher).toBe("Bash|Write");
  });

  it("添加 matcher 组：回调追加空 matcher 组", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    fireEvent.click(container.querySelector('[data-e2e="gui-group-add"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events[0].matcherGroups).toHaveLength(3);
    expect(last.events[0].matcherGroups[2]).toEqual({ matcher: "", handlers: [] });
  });

  it("删除 matcher 组：回调移除该组", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    fireEvent.click(container.querySelector('[data-e2e="gui-group-del-PreToolUse-0"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events[0].matcherGroups).toHaveLength(1);
    expect(last.events[0].matcherGroups[0].matcher).toBe("");
  });
});

describe("P3-TE-12 handler 增删与支持矩阵", () => {
  afterEach(() => {
    cleanup();
  });

  it("添加 handler：type 下拉 = 事件支持矩阵（eventsCatalog 驱动），回调追加该 type", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    // A 档事件：5 种全支持
    const typeSelect = container.querySelector('[data-e2e="gui-handler-type-PreToolUse-0"]') as HTMLSelectElement;
    expect(Array.from(typeSelect.options).map((o) => o.value)).toEqual(
      Array.from(getSupportedHandlerTypes("PreToolUse")),
    );
    fireEvent.change(typeSelect, { target: { value: "http" } });
    fireEvent.click(container.querySelector('[data-e2e="gui-handler-add-PreToolUse-0"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    const group0 = last.events[0].matcherGroups[0];
    expect(group0.handlers).toHaveLength(3);
    expect(group0.handlers[2]).toEqual({ type: "http" });
  });

  it("C 档事件（SessionStart）type 下拉仅 command/mcp_tool", () => {
    const { container } = renderGui(makeDefaultGui());
    fireEvent.click(container.querySelector('[data-e2e="gui-event-SessionStart"]') as HTMLElement);
    const typeSelect = container.querySelector('[data-e2e="gui-handler-type-SessionStart-0"]') as HTMLSelectElement;
    expect(Array.from(typeSelect.options).map((o) => o.value)).toEqual(["command", "mcp_tool"]);
  });

  it("删除 handler：回调移除该 handler；空组显示「暂无 handler」", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    fireEvent.click(container.querySelector('[data-e2e="gui-event-SessionStart"]') as HTMLElement);
    fireEvent.click(container.querySelector('[data-e2e="gui-handler-del-SessionStart-0-0"]') as HTMLElement);
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events[1].matcherGroups[0].handlers).toEqual([]);
    expect(container.textContent).toContain("暂无 handler");
  });
});

describe("P3-FE-14 HandlerForm 接入 GuiMode", () => {
  afterEach(() => {
    cleanup();
  });

  /** 选中 PreToolUse 组 0 的 handler index（事件 → matcher 组头 → handler 行） */
  function selectHandler(container: HTMLElement, hi: number) {
    fireEvent.click(container.querySelector('[data-e2e="gui-event-PreToolUse"]') as HTMLElement);
    fireEvent.click(
      container.querySelector('[data-e2e="gui-matcher-PreToolUse-0"]')?.parentElement as HTMLElement,
    );
    fireEvent.click(
      container.querySelector(`[data-e2e="gui-handler-PreToolUse-0-${hi}"]`) as HTMLElement,
    );
  }

  it("选中 handler 渲染 HandlerForm（type 选择器 + 字段表单）", () => {
    const { container } = renderGui(makeDefaultGui());
    // 未选中时无表单
    expect(container.querySelector('[data-e2e="handler-form"]')).toBeNull();
    selectHandler(container, 0);
    // 选中后表单展开：type 选择器值 = 该 handler type；command 字段值为既有字段
    expect(container.querySelector('[data-e2e="handler-form"]')).toBeTruthy();
    expect((container.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement).value).toBe("command");
    expect((container.querySelector('[data-e2e="handler-field-command"]') as HTMLInputElement).value).toBe("echo hi");
  });

  it("HandlerForm 编辑字段 → updateHandler 上抛新模型（双模式同步入口）", () => {
    const onChange = vi.fn();
    const { container } = renderGui(makeDefaultGui(), onChange);
    selectHandler(container, 0);
    const field = container.querySelector('[data-e2e="handler-field-command"]') as HTMLInputElement;
    fireEvent.change(field, { target: { value: "echo updated" } });
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as HooksConfigGui;
    expect(last.events[0].matcherGroups[0].handlers[0].command).toBe("echo updated");
    // 同组其他 handler 不受影响
    expect(last.events[0].matcherGroups[0].handlers[1].type).toBe("http");
  });

  it("切换选中另一个 handler → 表单跟随（值替换）", () => {
    const { container } = renderGui(makeDefaultGui());
    selectHandler(container, 0);
    expect((container.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement).value).toBe("command");
    selectHandler(container, 1);
    // http handler：type 选择器切为 http，url 字段出现
    expect((container.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement).value).toBe("http");
    expect(container.querySelector('[data-e2e="handler-field-url"]')).toBeTruthy();
  });

  it("托管 handler 表单只读（C13-8：badge + 字段 disabled + type disabled）", () => {
    const gui: HooksConfigGui = {
      events: [
        makeEvent("PreToolUse", [{ matcher: "Bash", handlers: [managedHandler] }]),
      ],
    };
    const { container } = renderGui(gui);
    selectHandler(container, 0);
    expect(container.querySelector('[data-e2e="handler-managed-badge"]')).toBeTruthy();
    expect((container.querySelector('[data-e2e="handler-type-select"]') as HTMLSelectElement).disabled).toBe(true);
    expect((container.querySelector('[data-e2e="handler-field-command"]') as HTMLInputElement).disabled).toBe(true);
  });
});
