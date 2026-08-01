// hooks-config-jsonmode.test.tsx — JsonMode / MatcherTester L2 测试（P3-TE-09 / P3-TE-10）
//
// TE-09（JSON 模式渲染与 Schema 校验）：CM6 EditorView 创建、schema 扩展注册
// （jsonSchemaHover/jsonSchemaLinter + hooks 子 schema + height theme）、
// 非法 JSON 触发 onValidationChange(false)、外部 value 同步、MatcherTester 试测。
// TE-10（事件导航）：十大分组事件名渲染、点击后选区跳到对应事件键位置。
//
// mock 策略：照 gitshow-panel.test.tsx 先例——jsdom 无布局引擎，mock CM6 模块，
// 捕获 EditorState.create 的 extensions 与 updateListener 回调手动驱动。
// schema 模块保持真实（validateHooksJson 与 hooksSubSchema 是测试目标）。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockEditorViewDestroy,
  mockScrollIntoView,
  mockUpdateListenerOf,
  mockEditorViewTheme,
  mockJsonSchemaHover,
  mockJsonSchemaLinter,
  mockStateExtensions,
  mockHandleRefresh,
  mockLinter,
  mockHoverTooltip,
  mockJsonParseLinter,
  mockJson,
  capturedEditorStateConfig,
} = vi.hoisted(() => {
  const mockScrollIntoView = vi.fn(() => ({ __scrollIntoView: true }));
  const capturedEditorStateConfig: { doc?: string; extensions?: unknown[] }[] = [];
  return {
    mockEditorViewDestroy: vi.fn(),
    mockScrollIntoView,
    mockUpdateListenerOf: vi.fn((cb: unknown) => ({ __listener: cb })),
    mockEditorViewTheme: vi.fn(() => []),
    mockJsonSchemaHover: vi.fn(() => [{ __schemaHover: true }]),
    mockJsonSchemaLinter: vi.fn((opts: unknown) => [{ __schemaLinter: opts }]),
    mockStateExtensions: vi.fn((schema: unknown) => [{ __stateExt: schema }]),
    mockHandleRefresh: vi.fn(),
    mockLinter: vi.fn((x: unknown) => x),
    mockHoverTooltip: vi.fn((x: unknown) => x),
    mockJsonParseLinter: vi.fn(() => ({ __parseLinter: true })),
    mockJson: vi.fn(() => [{ __jsonLang: true }]),
    capturedEditorStateConfig,
  };
});

// @codemirror/view mock —— EditorView 无法在 jsdom 真实工作（无布局引擎）
vi.mock("@codemirror/view", () => {
  const MockEditorView = class {
    static theme = mockEditorViewTheme;
    static scrollIntoView = mockScrollIntoView;
    static updateListener = { of: mockUpdateListenerOf };
    state: { doc: string };
    // 实例级 mock——经 parent._cmView 访问断言 dispatch/focus 调用
    dispatch = vi.fn();
    focus = vi.fn();
    constructor(config: { state: { doc: string }; parent: HTMLElement }) {
      this.state = config.state;
      (config.parent as HTMLElement & Record<string, unknown>)._cmView = this;
    }
    destroy() {
      mockEditorViewDestroy();
    }
  };
  return {
    EditorView: MockEditorView,
    hoverTooltip: mockHoverTooltip,
  };
});

// @codemirror/state mock —— 捕获 EditorState.create 的 doc/extensions
vi.mock("@codemirror/state", () => ({
  EditorState: {
    create(config: { doc?: string; extensions?: unknown[] }) {
      capturedEditorStateConfig.push({ doc: config.doc, extensions: config.extensions });
      return { doc: config.doc ?? "" };
    },
  },
}));

vi.mock("codemirror", () => ({ basicSetup: [] }));
vi.mock("@codemirror/theme-one-dark", () => ({ oneDark: [] }));

// @codemirror/lang-json mock —— json 语言 + 语法 linter
vi.mock("@codemirror/lang-json", () => ({
  json: mockJson,
  jsonParseLinter: mockJsonParseLinter,
}));

// @codemirror/lint mock
vi.mock("@codemirror/lint", () => ({ linter: mockLinter }));

// codemirror-json-schema mock —— schema 扩展注册 spy（schema 参数 = hooks 子 schema）
vi.mock("codemirror-json-schema", () => ({
  jsonSchemaHover: mockJsonSchemaHover,
  jsonSchemaLinter: mockJsonSchemaLinter,
  stateExtensions: mockStateExtensions,
  handleRefresh: mockHandleRefresh,
}));

import React from "react";
import { render, cleanup, fireEvent, act } from "@testing-library/react";
import JsonMode, { findEventPosition } from "../panels/hooksConfig/JsonMode";
import MatcherTester from "../panels/hooksConfig/MatcherTester";
import { HOOK_EVENTS, EVENT_GROUPS, getGroups, getEventsByGroup } from "../panels/hooksConfig/eventsCatalog";
import { hooksSubSchema } from "../features/hooksConfig/schema";

/** mock EditorView 挂载形态（经 parent._cmView 访问） */
interface MockEditorViewInstance {
  dispatch: ReturnType<typeof vi.fn>;
  focus: ReturnType<typeof vi.fn>;
  state: { doc: string };
}

/** 渲染 JsonMode 并返回容器 + mock EditorView 实例 */
function renderJsonMode(value: string, onChange = vi.fn(), onValidationChange = vi.fn()) {
  const utils = render(
    React.createElement(JsonMode, { value, onChange, onValidationChange }),
  );
  const container = utils.container.querySelector(
    '[data-e2e="hooks-json-editor"]',
  ) as HTMLElement & { _cmView?: MockEditorViewInstance };
  if (!container?._cmView) {
    throw new Error("EditorView mock 未挂载（容器 ref 未绑定）");
  }
  return { ...utils, view: container._cmView, onChange, onValidationChange };
}

describe("P3-TE-09 JSON 模式渲染与 Schema 校验", () => {
  beforeEach(() => {
    capturedEditorStateConfig.length = 0;
    mockJsonSchemaHover.mockClear();
    mockJsonSchemaLinter.mockClear();
    mockStateExtensions.mockClear();
    mockLinter.mockClear();
    mockHoverTooltip.mockClear();
    mockJsonParseLinter.mockClear();
    mockJson.mockClear();
    mockUpdateListenerOf.mockClear();
    mockEditorViewTheme.mockClear();
    mockScrollIntoView.mockClear();
    mockEditorViewDestroy.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("渲染创建 CM6 EditorView（doc = value 外部驱动）", () => {
    const { view } = renderJsonMode('{"PreToolUse": []}');
    expect(view).toBeTruthy();
    expect(capturedEditorStateConfig.length).toBe(1);
    expect(capturedEditorStateConfig[0].doc).toBe('{"PreToolUse": []}');
  });

  it("schema 扩展注册：jsonSchemaHover / jsonSchemaLinter + hooks 子 schema + height theme", () => {
    renderJsonMode("{}");
    // schema 扩展均被调用
    expect(mockJsonSchemaHover).toHaveBeenCalledTimes(1);
    expect(mockJsonSchemaLinter).toHaveBeenCalledTimes(1);
    // stateExtensions 收到 hooks 子 schema（对齐 hooks 子树编辑范围）
    expect(mockStateExtensions).toHaveBeenCalledTimes(1);
    expect(mockStateExtensions.mock.calls[0][0]).toBe(hooksSubSchema);
    // jsonSchemaLinter 无参注册，needsRefresh 配置在外层 linter() 包装上
    expect(mockJsonSchemaLinter).toHaveBeenCalledTimes(1);
    expect(mockLinter).toHaveBeenCalledTimes(2);
    const linterCalls = mockLinter.mock.calls as unknown as [
      [unknown, unknown],
      [unknown, unknown],
    ];
    expect(linterCalls[0][1]).toEqual({ delay: 300 });
    expect(linterCalls[1][1]).toEqual({ needsRefresh: mockHandleRefresh });
    // 语言 + 语法 linter 注册
    expect(mockJson).toHaveBeenCalledTimes(1);
    expect(mockJsonParseLinter).toHaveBeenCalledTimes(1);
    expect(mockHoverTooltip).toHaveBeenCalled();
    // height:100% theme（.cm-editor 确定高度 → 竖向滚动条，验收 1.3）
    expect(mockEditorViewTheme).toHaveBeenCalledWith({ "&": { height: "100%" } });
  });

  it("非法 JSON 触发 onValidationChange(false) + onChange 透传", () => {
    const onChange = vi.fn();
    const onValidationChange = vi.fn();
    renderJsonMode("{}", onChange, onValidationChange);
    // 手动驱动 updateListener 回调（mock 捕获的组件真实回调）
    const listener = mockUpdateListenerOf.mock.calls[0][0] as (
      update: { docChanged: boolean; state: { doc: string } },
    ) => void;
    act(() => {
      listener({ docChanged: true, state: { doc: "{bad json" } });
    });
    expect(onChange).toHaveBeenCalledWith("{bad json");
    expect(onValidationChange).toHaveBeenCalledWith(
      false,
      expect.arrayContaining([
        expect.objectContaining({ message: expect.stringContaining("JSON 语法错误") }),
      ]),
    );
  });

  it("合法 JSON 但 schema 违规（未知事件键）触发 onValidationChange(false)", () => {
    const onValidationChange = vi.fn();
    renderJsonMode("{}", vi.fn(), onValidationChange);
    const listener = mockUpdateListenerOf.mock.calls[0][0] as (
      update: { docChanged: boolean; state: { doc: string } },
    ) => void;
    act(() => {
      // NoSuchEvent 不在 hooks 子 schema properties（additionalProperties: false）
      listener({ docChanged: true, state: { doc: '{"NoSuchEvent": []}' } });
    });
    expect(onValidationChange).toHaveBeenCalledWith(false, expect.any(Array));
  });

  it("合法 JSON 通过校验触发 onValidationChange(true)", () => {
    const onValidationChange = vi.fn();
    renderJsonMode("{}", vi.fn(), onValidationChange);
    const listener = mockUpdateListenerOf.mock.calls[0][0] as (
      update: { docChanged: boolean; state: { doc: string } },
    ) => void;
    act(() => {
      listener({
        docChanged: true,
        state: { doc: '{"PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}]}' },
      });
    });
    expect(onValidationChange).toHaveBeenCalledWith(true, []);
  });

  it("外部 value 变化同步进编辑器（dispatch 全量替换）；相同 value 不 dispatch", () => {
    const { rerender, view, container } = renderJsonMode('{"PreToolUse": []}');
    // 相同 value 重渲染：不 dispatch
    rerender(
      React.createElement(JsonMode, {
        value: '{"PreToolUse": []}',
        onChange: vi.fn(),
        onValidationChange: vi.fn(),
      }),
    );
    expect(view.dispatch).not.toHaveBeenCalled();
    // 新 value（如保存/重载/切层后）：dispatch 全量替换
    const next = '{"PreToolUse": [{"hooks": [{"type": "command", "command": "x"}]}]}';
    rerender(
      React.createElement(JsonMode, {
        value: next,
        onChange: vi.fn(),
        onValidationChange: vi.fn(),
      }),
    );
    const latest = (
      container.querySelector('[data-e2e="hooks-json-editor"]') as HTMLElement & {
        _cmView?: MockEditorViewInstance;
      }
    )._cmView;
    expect(latest?.dispatch).toHaveBeenCalledWith({
      changes: { from: 0, to: '{"PreToolUse": []}'.length, insert: next },
    });
  });

  it("卸载时 destroy EditorView", () => {
    const { unmount } = renderJsonMode("{}");
    unmount();
    expect(mockEditorViewDestroy).toHaveBeenCalledTimes(1);
  });

  it("MatcherTester 集成渲染：默认空 matcher 全匹配命中", () => {
    const { getByText, container } = render(React.createElement(MatcherTester));
    expect(container.querySelector('[data-e2e="hooks-matcher-tester"]')).toBeTruthy();
    // 空 matcher → 全匹配（mode: all）
    expect(getByText("命中 ✓")).toBeTruthy();
    expect(container.textContent).toContain("全匹配");
  });
});

describe("P3-TE-09 MatcherTester 试测", () => {
  afterEach(() => {
    cleanup();
  });

  it("精确匹配 OR：matcher=Edit|Write 命中 Edit", () => {
    const { container, getByText } = render(React.createElement(MatcherTester));
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-input"]') as HTMLInputElement,
      { target: { value: "Edit|Write" } },
    );
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-tool"]') as HTMLInputElement,
      { target: { value: "Edit" } },
    );
    expect(getByText("命中 ✓")).toBeTruthy();
    expect(container.textContent).toContain("精确匹配 OR");
  });

  it("精确匹配 OR：toolName 不在 OR 列表 → 未命中", () => {
    const { container, getByText } = render(React.createElement(MatcherTester));
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-input"]') as HTMLInputElement,
      { target: { value: "Edit|Write" } },
    );
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-tool"]') as HTMLInputElement,
      { target: { value: "Bash" } },
    );
    expect(getByText("未命中 ✗")).toBeTruthy();
  });

  it("JS 正则模式：非窄字符集 matcher 走正则", () => {
    const { container, getByText } = render(React.createElement(MatcherTester));
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-input"]') as HTMLInputElement,
      { target: { value: "^Ed.*t$" } },
    );
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-tool"]') as HTMLInputElement,
      { target: { value: "Edit" } },
    );
    expect(getByText("命中 ✓")).toBeTruthy();
    expect(container.textContent).toContain("JS 正则（非锚定）");
  });

  it("事件感知窄字符集：FileChanged 受限提示 + 连字符强制走正则", () => {
    const { container } = render(React.createElement(MatcherTester));
    // 切到 FileChanged（受限窄字符集）
    fireEvent.change(container.querySelector('[data-e2e="matcher-event"]') as HTMLSelectElement, {
      target: { value: "FileChanged" },
    });
    expect(container.textContent).toContain("受限窄字符集");
    // matcher 含连字符（受限字符集外字符）→ 走 JS 正则
    fireEvent.change(
      container.querySelector('[data-e2e="matcher-input"]') as HTMLInputElement,
      { target: { value: "*.ts" } },
    );
    expect(container.textContent).toContain("JS 正则（非锚定）");
  });
});

describe("P3-TE-10 事件导航", () => {
  beforeEach(() => {
    mockUpdateListenerOf.mockClear();
    mockScrollIntoView.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("渲染十大分组标题 + 30 事件按钮（eventsCatalog 驱动）", () => {
    const { container } = renderJsonMode("{}");
    // 十大分组（非旧九组）
    expect(getGroups()).toEqual(EVENT_GROUPS);
    expect(EVENT_GROUPS.length).toBe(10);
    for (const group of EVENT_GROUPS) {
      expect(container.textContent).toContain(group);
    }
    // 30 事件按钮齐全
    expect(HOOK_EVENTS.length).toBe(30);
    for (const meta of HOOK_EVENTS) {
      expect(container.querySelector(`[data-e2e="hooks-nav-${meta.event}"]`)).toBeTruthy();
    }
    // 分组内事件归属正确（eventsCatalog 分组查询）
    for (const group of EVENT_GROUPS) {
      const events = getEventsByGroup(group);
      expect(events.length).toBeGreaterThan(0);
    }
  });

  it("findEventPosition 纯函数：正常定位 / 未找到 -1", () => {
    const doc = '{\n  "PreToolUse": [],\n  "PostToolUse": []\n}';
    expect(findEventPosition(doc, "PreToolUse")).toBe(doc.indexOf('"PreToolUse"'));
    expect(findEventPosition(doc, "SessionStart")).toBe(-1);
  });

  it("点击事件按钮 → dispatch setSelection 跳到对应事件键位置（scrollIntoView）", () => {
    const doc = '{\n  "PreToolUse": [],\n  "PostToolUse": []\n}';
    const { container, view } = renderJsonMode(doc);
    const pos = doc.indexOf('"PreToolUse"');
    fireEvent.click(
      container.querySelector('[data-e2e="hooks-nav-PreToolUse"]') as HTMLElement,
    );
    expect(view.dispatch).toHaveBeenCalledWith({
      selection: { anchor: pos, head: pos + "PreToolUse".length + 2 },
      effects: { __scrollIntoView: true },
    });
    expect(view.focus).toHaveBeenCalledTimes(1);
  });

  it("事件不在文档中 → 点击不产生 dispatch（无副作用）", () => {
    const { container, view } = renderJsonMode('{"PreToolUse": []}');
    fireEvent.click(container.querySelector('[data-e2e="hooks-nav-SessionEnd"]') as HTMLElement);
    expect(view.dispatch).not.toHaveBeenCalled();
  });

  it("导航 + 编辑联动：点击后 dispatch 修改不影响后续编辑校验", () => {
    const onChange = vi.fn();
    const onValidationChange = vi.fn();
    const doc = '{\n  "PreToolUse": [],\n  "PostToolUse": []\n}';
    const { container, view } = renderJsonMode(doc, onChange, onValidationChange);
    fireEvent.click(container.querySelector('[data-e2e="hooks-nav-PostToolUse"]') as HTMLElement);
    expect(view.dispatch).toHaveBeenCalledTimes(1);
    // 编辑链路不受导航影响
    const listener = mockUpdateListenerOf.mock.calls[0][0] as (
      update: { docChanged: boolean; state: { doc: string } },
    ) => void;
    act(() => {
      listener({ docChanged: true, state: { doc: '{"PreToolUse": [1]}' } });
    });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onValidationChange).toHaveBeenCalledWith(false, expect.any(Array));
  });
});
