// hooks-config-sync.test.tsx — 双模式同步（P3-TE-13）+ 保存拒绝与提示（P3-TE-14）L2 测试
//
// TE-13（双模式同步）：GUI 新增事件 → JSON 文本含该事件；JSON 合法修改 → GUI 树更新；
//                      JSON 非法 → 切 GUI 被阻止（按钮禁用 + 工具栏错误提示）。
// TE-14（保存拒绝与提示）：语法错误保存被拒（对象形态校验弹窗 + UI 禁用门控）、
//                         schema 错误保存被拒（json-schema-library 校验弹窗）、
//                         合法保存成功显示重启提示、writeHooksConfig 调用 payload 为
//                         hooks 子树（键集合精确匹配 { layer, hooks, projectPath? }）。
//
// 测试模式照 hooks-config-panel.test.tsx：mock JsonMode/GuiMode 捕获 props 驱动双向同步；
// useHooksConfig 保存路径用 renderHook 直测（绕过 UI 禁用门控，直达校验拒绝逻辑）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ── props 捕获类型：mock 组件每次渲染调用一次，末次调用 = 当前 props ──
interface JsonModePropsLike {
  value: string;
  onChange: (text: string) => void;
  onValidationChange: (isValid: boolean, diagnostics: { message: string; pointer: string }[]) => void;
}
interface GuiEventLike {
  event: string;
  group?: string;
  matcherGroups: unknown[];
}
interface GuiModePropsLike {
  gui: { events: GuiEventLike[] };
  onChange: (gui: { events: GuiEventLike[] }) => void;
}

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockReadHooksConfig, mockWriteHooksConfig, mockAsk, mockJsonMode, mockGuiMode } = vi.hoisted(
  () => ({
    mockReadHooksConfig: vi.fn(),
    mockWriteHooksConfig: vi.fn().mockResolvedValue(undefined),
    mockAsk: vi.fn().mockResolvedValue(true),
    // JsonMode/GuiMode mock 组件：渲染 null，测试经 mock 调用参数断言 props 传递与回调
    mockJsonMode: vi.fn(() => null),
    mockGuiMode: vi.fn(() => null),
  }),
);

// mock IPC hooksConfig —— 三层 hooks 子树读写
vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: mockReadHooksConfig,
  writeHooksConfig: mockWriteHooksConfig,
}));

// mock IPC dialog —— 保存失败弹窗 + dirty 确认（不弹真实对话框）
vi.mock("../ipc/dialog", () => ({
  ask: mockAsk,
}));

// mock JsonMode/GuiMode —— 隔离 CM6/schema 与表单树（各自测试见对应文件）
vi.mock("../panels/hooksConfig/JsonMode", () => ({ default: mockJsonMode }));
vi.mock("../panels/hooksConfig/GuiMode", () => ({ default: mockGuiMode }));

import React from "react";
import { render, fireEvent, waitFor, act, cleanup, renderHook } from "@testing-library/react";
import { HooksConfigPanel } from "../panels/hooksConfig";
import { useHooksConfig } from "../panels/hooksConfig/useHooksConfig";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { HooksConfigJson } from "../types/hooksConfig";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";

/** 基线合法 hooks 子树（通过 schema 校验） */
const VALID_BASE: HooksConfigJson = {
  PreToolUse: [{ hooks: [{ type: "command", command: "echo hi" }] }],
};

// ── 辅助：种子 stores（照 hooks-config-panel.test.tsx）──
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

// ── props 捕获：末次调用 = 当前 props（mock 无参签名，calls 整体强转） ──
function jsonProps(): JsonModePropsLike {
  const calls = mockJsonMode.mock.calls as unknown as JsonModePropsLike[][];
  return calls[calls.length - 1][0];
}

function guiProps(): GuiModePropsLike {
  const calls = mockGuiMode.mock.calls as unknown as GuiModePropsLike[][];
  return calls[calls.length - 1][0];
}

/** 渲染面板（调用方随后 await waitFor(mockJsonMode 已调用) 等待加载完成） */
function renderLoadedPanel() {
  return render(React.createElement(HooksConfigPanel));
}

const byE2e = (container: HTMLElement, selector: string): HTMLElement =>
  container.querySelector(`[data-e2e="${selector}"]`) as HTMLElement;

// ═══════════════════════════════════════════════════════════════════
// P3-TE-13 双模式同步
// ═══════════════════════════════════════════════════════════════════
describe("P3-TE-13 双模式同步", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockJsonMode.mockClear();
    mockGuiMode.mockClear();
    resetStores();
  });

  afterEach(() => {
    cleanup();
  });

  it("GUI 新增事件 → JSON 文本含该事件（guiToJson → configJson 同步）", async () => {
    mockReadHooksConfig.mockResolvedValueOnce(VALID_BASE);
    const { container, getByRole } = renderLoadedPanel();
    await waitFor(() => expect(mockJsonMode).toHaveBeenCalled());

    // 切到 GUI 模式 → 捕获 GuiMode props（onChange = useHooksConfig.updateGui）
    fireEvent.click(byE2e(container, "hooks-mode-gui"));
    expect(mockGuiMode).toHaveBeenCalled();

    // GUI 新增事件：构造新模型上抛 onChange（等价真实 GuiMode「添加事件」后的行为）
    act(() => {
      guiProps().onChange({
        events: [
          ...guiProps().gui.events,
          {
            event: "Stop",
            group: "Session",
            matcherGroups: [{ matcher: "", handlers: [{ type: "command", command: "echo stop" }] }],
          },
        ],
      });
    });

    // 切回 JSON 模式 → JsonMode value（configJson 序列化）含新事件，原事件保留
    fireEvent.click(byE2e(container, "hooks-mode-json"));
    await waitFor(() => {
      const parsed = JSON.parse(jsonProps().value) as HooksConfigJson;
      expect(parsed.Stop).toBeDefined();
      expect(parsed.Stop?.[0].hooks[0].command).toBe("echo stop");
      expect(parsed.PreToolUse?.[0].hooks[0].command).toBe("echo hi");
    });
    // 两模式共享 dirty：GUI 编辑后保存按钮可用
    const saveBtn = getByRole("button", { name: "保存" }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(false);
  });

  it("JSON 合法修改 → GUI 树更新（jsonToGui → guiModel 同步）", async () => {
    mockReadHooksConfig.mockResolvedValueOnce(VALID_BASE);
    const { container } = renderLoadedPanel();
    await waitFor(() => expect(mockJsonMode).toHaveBeenCalled());

    // 合法 JSON 修改：含新增 Stop 事件（parse 通过 → updateConfigJson → jsonToGui）
    const modified = JSON.stringify({
      ...VALID_BASE,
      Stop: [{ hooks: [{ type: "command", command: "echo stop" }] }],
    });
    act(() => {
      jsonProps().onChange(modified);
      jsonProps().onValidationChange(true, []);
    });

    // 切到 GUI 模式（JSON 合法 → 允许）→ 事件树含新事件
    fireEvent.click(byE2e(container, "hooks-mode-gui"));
    await waitFor(() => expect(mockGuiMode).toHaveBeenCalled());
    const events = guiProps().gui.events;
    const stop = events.find((e) => e.event === "Stop");
    expect(stop).toBeDefined();
    expect(stop?.matcherGroups[0]).toMatchObject({ matcher: "", handlers: [{ type: "command", command: "echo stop" }] });
  });

  it("JSON 非法 → 切 GUI 被阻止（按钮禁用 + 工具栏错误提示 + 保存禁用）", async () => {
    mockReadHooksConfig.mockResolvedValueOnce(VALID_BASE);
    const { container, getByText } = renderLoadedPanel();
    await waitFor(() => expect(mockJsonMode).toHaveBeenCalled());

    // 模拟非法 JSON：onChange 不更新 configJson（保留最后合法快照），onValidationChange 上报 false
    act(() => {
      jsonProps().onChange('{ "PreToolUse": ');
      jsonProps().onValidationChange(false, [
        { message: "JSON 语法错误：Unexpected token", pointer: "" },
      ]);
    });

    // GUI 按钮禁用 + 工具栏错误提示
    const guiBtn = byE2e(container, "hooks-mode-gui") as HTMLButtonElement;
    expect(guiBtn.disabled).toBe(true);
    expect(getByText(/JSON 存在错误，无法切换 GUI/)).toBeTruthy();

    // 点击被禁按钮 → 仍停留 JSON 模式（GuiMode 未渲染）
    fireEvent.click(guiBtn);
    expect(mockGuiMode).not.toHaveBeenCalled();

    // 非法 JSON 同步禁用保存入口（语法错误拒绝保存）
    const saveBtn = byE2e(container, "hooks-save") as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
    expect(mockWriteHooksConfig).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════
// P3-TE-14 保存拒绝与提示
// ═══════════════════════════════════════════════════════════════════
describe("P3-TE-14 保存拒绝与提示", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockJsonMode.mockClear();
    mockGuiMode.mockClear();
    resetStores();
  });

  afterEach(() => {
    cleanup();
  });

  it("语法错误（非对象）保存被拒：弹窗提示 + 拒绝 writeHooksConfig + dirty 保留", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.updateConfigJson(null as unknown as HooksConfigJson);
    });
    await act(async () => {
      await result.current.save();
    });

    // JSON.parse 语法校验失败 → 弹窗提示、不调用 writeHooksConfig
    expect(mockAsk).toHaveBeenCalledWith(
      "hooks 配置必须是 JSON 对象",
      expect.objectContaining({ title: "保存失败" }),
    );
    expect(mockWriteHooksConfig).not.toHaveBeenCalled();
    expect(result.current.dirty).toBe(true);
    expect(result.current.saved).toBe(false);
  });

  it("schema 错误保存被拒：弹窗提示诊断 + 拒绝 writeHooksConfig", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      // 未知事件 → hooks 子 schema additionalProperties:false 拒绝（语法合法但 schema 违规）
      result.current.updateConfigJson({
        NotARealEvent: [{ hooks: [{ type: "command", command: "x" }] }],
      });
    });
    await act(async () => {
      await result.current.save();
    });

    expect(mockAsk).toHaveBeenCalledWith(
      expect.stringContaining("Additional property"),
      expect.objectContaining({ title: "保存失败" }),
    );
    expect(mockWriteHooksConfig).not.toHaveBeenCalled();
    expect(result.current.dirty).toBe(true);
  });

  it("合法保存成功（user 层）：payload 为 hooks 子树，键集合 { layer, hooks } + saved 置位", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.updateConfigJson(VALID_BASE);
    });
    await act(async () => {
      await result.current.save();
    });

    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    // 键集合精确匹配：cliId 首参（中间态 = CLAUDE_CLI_ID）+ user 层无 projectPath → 第 4 参 undefined，
    // wrapper 层（ipc/hooksConfig.ts）按 undefined 省略 projectPath 键 → invoke payload 为 { cliId, layer, hooks }
    //（wrapper→invoke 键集合契约由 ipc-hooks-config-contract.test.ts 守卫）
    const callArgs = mockWriteHooksConfig.mock.calls[0];
    expect(callArgs[0]).toBe(CLAUDE_CLI_ID);
    expect(callArgs[1]).toBe("user");
    expect(callArgs[3]).toBeUndefined();
    // hooks 参数为纯 hooks 子树：仅事件键，无其他 settings 字段
    expect(callArgs[2]).toEqual(VALID_BASE);
    expect(Object.keys(callArgs[2])).toEqual(["PreToolUse"]);
    expect(result.current.dirty).toBe(false);
    expect(result.current.saved).toBe(true);
  });

  it("合法保存成功（project 层）：payload 键集合 { layer, hooks, projectPath }", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    // 切到 project 层（dirty=false 无需确认弹窗）
    act(() => {
      result.current.setLayer("project");
    });
    await waitFor(() => expect(result.current.layer).toBe("project"));

    act(() => {
      result.current.updateConfigJson({
        ...VALID_BASE,
        Stop: [{ hooks: [{ type: "command", command: "echo stop" }] }],
      });
    });
    await act(async () => {
      await result.current.save();
    });

    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    // 键集合精确匹配：cliId 首参 + project 层含 projectPath → invoke payload 为 { cliId, layer, hooks, projectPath }
    const callArgs = mockWriteHooksConfig.mock.calls[0];
    expect(callArgs[0]).toBe(CLAUDE_CLI_ID);
    expect(callArgs[1]).toBe("project");
    expect(callArgs[2]).toEqual({
      ...VALID_BASE,
      Stop: [{ hooks: [{ type: "command", command: "echo stop" }] }],
    });
    expect(callArgs[3]).toBe("C:/proj");
    expect(result.current.saved).toBe(true);
  });

  it("合法保存成功 → 面板状态条显示「hooks 改动需重启 claude 会话生效」；再次编辑后隐藏", async () => {
    mockReadHooksConfig.mockResolvedValueOnce(VALID_BASE);
    const { container } = renderLoadedPanel();
    await waitFor(() => expect(mockJsonMode).toHaveBeenCalled());

    // 模拟一次合法编辑（dirty=true，JSON 合法）
    act(() => {
      jsonProps().onChange(
        JSON.stringify({
          ...VALID_BASE,
          Stop: [{ hooks: [{ type: "command", command: "echo stop" }] }],
        }),
      );
      jsonProps().onValidationChange(true, []);
    });
    const saveBtn = byE2e(container, "hooks-save") as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(false);

    fireEvent.click(saveBtn);
    // 保存成功 → 重启提示条显示
    await waitFor(() =>
      expect(container.querySelector('[data-e2e="hooks-restart-hint"]')).toBeTruthy(),
    );
    expect(container.querySelector('[data-e2e="hooks-restart-hint"]')?.textContent).toContain(
      "hooks 改动需重启 claude 会话生效",
    );
    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);

    // 再次编辑 → 提示隐藏（saved 状态随编辑清除）
    act(() => {
      jsonProps().onChange(JSON.stringify(VALID_BASE));
      jsonProps().onValidationChange(true, []);
    });
    expect(container.querySelector('[data-e2e="hooks-restart-hint"]')).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════
// HKC-02 load() generation 竞态取消
// ═══════════════════════════════════════════════════════════════════
describe("HKC-02 load() generation 竞态取消", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    resetStores();
  });

  afterEach(() => {
    cleanup();
  });

  it("切层竞态：旧层 read 延迟 resolve 被丢弃，最终 configJson 为目标层数据", async () => {
    seedProject("C:/proj");
    const USER_CONFIG: HooksConfigJson = {
      PreToolUse: [{ hooks: [{ type: "command", command: "echo user" }] }],
    };
    const PROJECT_CONFIG: HooksConfigJson = {
      PostToolUse: [{ hooks: [{ type: "command", command: "echo project" }] }],
    };
    // 第一次 read（挂载 user 层）挂起——模拟慢请求
    let resolveUser!: (v: unknown) => void;
    mockReadHooksConfig.mockReturnValueOnce(
      new Promise((r) => {
        resolveUser = r;
      }),
    );
    // 第二次 read（project 层）直接 resolve
    mockReadHooksConfig.mockResolvedValueOnce(PROJECT_CONFIG);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    // 切到 project 层（dirty=false 无需确认弹窗）→ 新请求发出
    act(() => {
      result.current.setLayer("project");
    });
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(2));
    // 新层 resolve → configJson 为目标层数据
    await waitFor(() => expect(result.current.configJson).toEqual(PROJECT_CONFIG));
    expect(result.current.layer).toBe("project");
    // 旧层延迟 resolve → generation 检查丢弃，configJson 不被覆盖
    await act(async () => {
      resolveUser(USER_CONFIG);
    });
    expect(result.current.configJson).toEqual(PROJECT_CONFIG);
  });
});
