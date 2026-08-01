// hooks-config-panel.test.tsx — hooks 配置面板 L2 测试（P3-TE-08 + P3-FE-21/22）
//
// 覆盖：PANEL_TYPES 包含 hooksConfig / isValidPanelType 识别 /
// HooksConfigPanel 三态渲染（loading / content / 损坏错误态）/
// 层级切换器存在与禁用逻辑（rootPath 为空 project/local 禁用）/
// 保存按钮初始禁用 / 页面重新可见（visibilitychange）轻量重读 / JsonMode 接入（P3-FE-11）/
// F2 注入/卸载按钮与注入状态条（P3-FE-21/22：三态显示、注入/卸载后刷新状态 + 重读 user 层）。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockReadHooksConfig, mockWriteHooksConfig, mockAsk, mockJsonMode } = vi.hoisted(() => ({
  mockReadHooksConfig: vi.fn(),
  mockWriteHooksConfig: vi.fn(),
  mockAsk: vi.fn(async () => true),
  // JsonMode mock 组件：渲染 null，测试经 mockJsonMode.mock.calls 断言 props 传递
  mockJsonMode: vi.fn(() => null),
}));

// mock IPC hooks —— F2 注入/卸载/状态查询（P3-FE-21/22；本地覆盖 setup.ts 全局 mock 以便断言调用）
const { mockInject, mockUninstall, mockGetInjectionStatus } = vi.hoisted(() => ({
  mockInject: vi.fn(),
  mockUninstall: vi.fn(),
  mockGetInjectionStatus: vi.fn(),
}));

// mock IPC hooksConfig —— 三层 hooks 子树读写
vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: mockReadHooksConfig,
  writeHooksConfig: mockWriteHooksConfig,
}));

// mock IPC hooks —— F2 注入/卸载/状态查询（P3-FE-21/22；本地覆盖 setup.ts 全局 mock 以便断言调用）
vi.mock("../ipc/hooks", () => ({
  inject: mockInject,
  uninstall: mockUninstall,
  getInjectionStatus: mockGetInjectionStatus,
}));

// mock JsonMode —— 隔离 CM6/schema（JsonMode 自身测试见 hooks-config-jsonmode.test.tsx）
vi.mock("../panels/hooksConfig/JsonMode", () => ({
  default: mockJsonMode,
}));

// mock IPC dialog —— dirty 确认弹窗（不弹真实对话框）
vi.mock("../ipc/dialog", () => ({
  ask: mockAsk,
}));

// mock IPC settings —— hooksConfig store loadFromDisk 的后端读
vi.mock("../ipc/settings", () => ({
  loadSettings: vi.fn(async () => null),
  saveSettings: vi.fn(async () => {}),
}));

import React from "react";
import { render, cleanup, fireEvent, waitFor, act } from "@testing-library/react";
import { HooksConfigPanel } from "../panels/hooksConfig";
import { PANEL_TYPES, isValidPanelType } from "../panelRegistry";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ── 辅助函数：种子 stores（照 commit-view 模式）──
function seedProject(rootPath: string) {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: undefined,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { "proj-1": true },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
}

// ── visibilitychange 辅助：jsdom 中派发事件不会自动改 visibilityState，
//    需先 defineProperty 覆盖（configurable 允许 afterEach 还原）；默认 "visible"。
function setVisibilityState(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { value: state, configurable: true });
}

/** 派发 visibilitychange（调用前按需 setVisibilityState） */
function dispatchVisibilityChange() {
  fireEvent(document, new Event("visibilitychange"));
}

describe("PANEL_TYPES / isValidPanelType", () => {
  it('PANEL_TYPES 包含 "hooksConfig"（末尾追加）', () => {
    expect(PANEL_TYPES).toContain("hooksConfig");
    expect(PANEL_TYPES[PANEL_TYPES.length - 1]).toBe("hooksConfig");
  });

  it('isValidPanelType("hooksConfig") 返回 true', () => {
    expect(isValidPanelType("hooksConfig")).toBe(true);
  });
});

describe("HooksConfigPanel 渲染", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockJsonMode.mockClear();
    // F2 mock 默认值：挂载 effect 会查询注入状态（不设默认则返回 undefined 使状态条显示崩溃）
    mockInject.mockReset();
    mockUninstall.mockReset();
    mockGetInjectionStatus.mockReset();
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    resetStores();
  });

  afterEach(() => {
    cleanup();
    Reflect.deleteProperty(document, "visibilityState"); // 还原 jsdom 默认可见（无自有属性时 no-op）
  });

  it("loading 态：显示加载中...", async () => {
    let resolveRead!: (v: unknown) => void;
    mockReadHooksConfig.mockReturnValueOnce(
      new Promise((r) => {
        resolveRead = r;
      }),
    );
    const { getByText } = render(React.createElement(HooksConfigPanel));
    // 首帧即 loading（初始 loading=true，read 未完成）
    expect(getByText("加载中...")).toBeTruthy();
    await act(async () => {
      resolveRead({});
    });
  });

  it("content 态：层级切换器 + 优先级标注 + 保存按钮 + 模式占位文案", async () => {
    mockReadHooksConfig.mockResolvedValueOnce({
      PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
    });
    const { getByRole, getByText } = render(React.createElement(HooksConfigPanel));
    // 三态完成后渲染工具栏
    await waitFor(() => expect(getByRole("button", { name: "User" })).toBeTruthy());
    expect(getByRole("button", { name: "Project" })).toBeTruthy();
    expect(getByRole("button", { name: "Local" })).toBeTruthy();
    expect(getByText("优先级：Local > Project > User")).toBeTruthy();
    // 保存按钮：初始无未保存修改 → 禁用
    const saveBtn = getByRole("button", { name: "保存" }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
    // 模式容器：JsonMode 已接入（P3-FE-11）——value 为 hooks 子树序列化 JSON
    expect(mockJsonMode).toHaveBeenCalled();
    const lastCall = mockJsonMode.mock.calls[mockJsonMode.mock.calls.length - 1] as unknown as [
      { value: string },
    ];
    expect(JSON.parse(lastCall[0].value)).toEqual({
      PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
    });
  });

  it("损坏错误态：read 返回 Err 显示「配置文件损坏，请先修复」（与无配置 null 区分）", async () => {
    mockReadHooksConfig.mockRejectedValueOnce(new Error("settings.json 损坏"));
    const { getByText } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(getByText("配置文件损坏，请先修复")).toBeTruthy());
  });

  it("无 rootPath 时 Project/Local 层禁用，仅 User 可用", async () => {
    mockReadHooksConfig.mockResolvedValueOnce({});
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    const userBtn = (await waitFor(() => getByRole("button", { name: "User" }))) as HTMLButtonElement;
    expect(userBtn.disabled).toBe(false);
    expect((getByRole("button", { name: "Project" }) as HTMLButtonElement).disabled).toBe(true);
    expect((getByRole("button", { name: "Local" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("有 rootPath 时 Project/Local 层可用", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValueOnce({});
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => getByRole("button", { name: "User" }));
    expect((getByRole("button", { name: "Project" }) as HTMLButtonElement).disabled).toBe(false);
    expect((getByRole("button", { name: "Local" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("点击层级按钮切换到 project 层并重读（携 rootPath）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    const projectBtn = await waitFor(() => getByRole("button", { name: "Project" }));
    fireEvent.click(projectBtn);
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      const last = calls[calls.length - 1];
      expect(last[0]).toBe("project");
      expect(last[1]).toBe("C:/proj");
    });
  });

  it("页面重新可见（visibilitychange）触发轻量重读（外部修改检测）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 模拟切回前台：置可见态再派发 visibilitychange（jsdom 不会自动翻状态）
    setVisibilityState("visible");
    dispatchVisibilityChange();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("visibilitychange 时面板不可见（display:none）不触发重读", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // Dockview 页面显隐为 CSS display:none——祖先或自身不可见时跳过（后台页面不重读）
    (container.firstElementChild as HTMLElement).style.display = "none";
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
  });

  it("visibilityState 为 hidden（切到后台）时不触发重读", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 切后台的 visibilitychange：先置 hidden 再派发——handler 开头状态过滤，不重读
    setVisibilityState("hidden");
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
    // 恢复可见后再派发 → 正常触发（证明过滤的是状态而非监听器缺失）
    setVisibilityState("visible");
    dispatchVisibilityChange();
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("面板内点击不触发重读（无 visibilitychange，验收 #1 语义延续）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    const innerButton = await waitFor(
      () => container.querySelector('[data-e2e="hooks-layer-user"]') as HTMLElement,
    );
    fireEvent.click(innerButton);
    await new Promise((r) => setTimeout(r, 20));
    expect(mockReadHooksConfig.mock.calls.length).toBe(1);
  });

  it("JSON 错误提示单行截断（长消息不换行撑高工具栏，验收 1.2）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    const longMsg =
      "JSON 语法错误：Expected ',' or '}' after property value in JSON at position 123 (line 4 column 7) #/PreToolUse/0/hooks/0";
    act(() => {
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [{ onValidationChange: (v: boolean, d: { message: string }[]) => void }];
      props[0].onValidationChange(false, [{ message: longMsg }]);
    });
    const err = container.querySelector('[data-e2e="hooks-json-error"]') as HTMLElement;
    expect(err).toBeTruthy();
    // 单行截断：nowrap + ellipsis + 宽度上限；完整消息挂 title
    expect(err.style.whiteSpace).toBe("nowrap");
    expect(err.style.textOverflow).toBe("ellipsis");
    expect(err.style.maxWidth).toBe("240px");
    expect(err.title).toBe(longMsg);
  });

  it("ask 弹窗打开期间 visibilitychange 回归不二次弹窗（验收 2.1 防循环）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    mockAsk.mockReturnValue(new Promise(() => {})); // 弹窗挂起（模拟确认框打开中）
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 置 dirty（合法 JSON 编辑 → updateConfigJson）
    act(() => {
      // calls[i] 为参数数组，props 在 calls[i][0]（照「content 态」用例 lastCall[0] 模式）
      const props = mockJsonMode.mock.calls[
        mockJsonMode.mock.calls.length - 1
      ] as unknown as [{ onChange: (t: string) => void }];
      props[0].onChange(
        JSON.stringify({ PreToolUse: [{ hooks: [{ type: "command", command: "x" }] }] }),
      );
    });
    // 第一次 visibilitychange（切回前台）→ reload → confirmDiscard → ask（挂起）
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 0));
    expect(mockAsk).toHaveBeenCalledTimes(1);
    // 弹窗关闭的回归（visibilitychange）→ askGuard 抑制，不二次弹窗（点否循环根治）
    dispatchVisibilityChange();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockAsk).toHaveBeenCalledTimes(1);
    expect(container.firstElementChild).toBeTruthy();
  });
});

describe("F2 注入/卸载与注入状态条（P3-FE-21/22）", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockInject.mockReset();
    mockUninstall.mockReset();
    mockGetInjectionStatus.mockReset();
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    resetStores();
  });

  afterEach(() => {
    cleanup();
  });

  it("挂载后显示注入状态三态（已注入 / 未注入 / 版本过旧）——挂载 effect 查询一次", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    // 已注入
    mockGetInjectionStatus.mockResolvedValue({ status: "injected", version: 1 });
    const first = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(first.getByText("注入状态：已注入")).toBeTruthy());
    first.unmount();
    // 版本过旧（重新挂载触发新查询）
    mockGetInjectionStatus.mockResolvedValue({ status: "outdated", version: 0 });
    const second = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(second.getByText("注入状态：版本过旧")).toBeTruthy());
    second.unmount();
    // 未注入（重新挂载触发新查询）
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    const third = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(third.getByText("注入状态：未注入")).toBeTruthy());
  });

  it("点击「注入 Hooks」→ inject 调用 → 状态刷新为已注入 → 重读 user 层配置", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockResolvedValue({ status: "injected", version: 1 });
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    await waitFor(() => expect(mockInject).toHaveBeenCalled());
    // 注入后重读 user 层（当前层即 user → reload）
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore + 1));
    expect(mockReadHooksConfig.mock.calls[mockReadHooksConfig.mock.calls.length - 1][0]).toBe("user");
  });

  it("project 层点注入 → 自动切到 user 层重读（最后一次 read 为 user 层）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockResolvedValue({ status: "injected", version: 1 });
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    const projectBtn = await waitFor(() => getByRole("button", { name: "Project" }));
    fireEvent.click(projectBtn);
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      expect(calls[calls.length - 1][0]).toBe("project");
    });
    fireEvent.click(getByRole("button", { name: "注入 Hooks" }));
    await waitFor(() => expect(mockInject).toHaveBeenCalled());
    await waitFor(() => {
      const calls = mockReadHooksConfig.mock.calls;
      expect(calls[calls.length - 1][0]).toBe("user");
    });
  });

  it("点击「卸载 Hooks」→ uninstall 调用 → 重新查询状态 → 重读 user 层配置", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockGetInjectionStatus.mockResolvedValue({ status: "notInjected", version: null });
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    // 初始挂载查询一次（P3-FE-22 挂载刷新）
    await waitFor(() => expect(mockGetInjectionStatus.mock.calls.length).toBe(1));
    const uninstallBtn = await waitFor(() => getByRole("button", { name: "卸载 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(uninstallBtn);
    await waitFor(() => expect(mockUninstall).toHaveBeenCalled());
    // 卸载后重新查询状态（uninstall 返回 void，状态由二次查询刷新）
    await waitFor(() => expect(mockGetInjectionStatus.mock.calls.length).toBe(2));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore + 1));
  });

  it("注入失败 → 显示错误提示（不刷新状态、不重读配置）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    mockInject.mockRejectedValue(new Error("settings.json 非法 JSON"));
    const { getByRole, getByText, queryByText } = render(React.createElement(HooksConfigPanel));
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    const callsBefore = mockReadHooksConfig.mock.calls.length;
    fireEvent.click(injectBtn);
    await waitFor(() => expect(getByText("注入失败，请检查 ~/.claude/settings.json")).toBeTruthy());
    expect(mockReadHooksConfig.mock.calls.length).toBe(callsBefore);
    expect(queryByText("注入状态：已注入")).toBeNull();
  });

  it("注入/卸载操作期间按钮禁用（防重复点击）", async () => {
    mockReadHooksConfig.mockResolvedValue({});
    let resolveInject!: (v: { status: string }) => void;
    mockInject.mockReturnValue(
      new Promise((r) => {
        resolveInject = r;
      }),
    );
    const { getByRole } = render(React.createElement(HooksConfigPanel));
    const injectBtn = await waitFor(() => getByRole("button", { name: "注入 Hooks" }));
    fireEvent.click(injectBtn);
    await waitFor(() => expect((injectBtn as HTMLButtonElement).disabled).toBe(true));
    expect((getByRole("button", { name: "卸载 Hooks" }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => {
      resolveInject({ status: "injected" });
    });
  });
});
