// hooks-config-panel.test.tsx — hooks 配置面板 L2 测试（P3-TE-08）
//
// 覆盖：PANEL_TYPES 包含 hooksConfig / isValidPanelType 识别 /
// HooksConfigPanel 三态渲染（loading / content / 损坏错误态）/
// 层级切换器存在与禁用逻辑（rootPath 为空 project/local 禁用）/
// 保存按钮初始禁用 / 面板聚焦（focusin）轻量重读。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockReadHooksConfig, mockWriteHooksConfig, mockAsk } = vi.hoisted(() => ({
  mockReadHooksConfig: vi.fn(),
  mockWriteHooksConfig: vi.fn(),
  mockAsk: vi.fn(async () => true),
}));

// mock IPC hooksConfig —— 三层 hooks 子树读写
vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: mockReadHooksConfig,
  writeHooksConfig: mockWriteHooksConfig,
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
import { useHooksConfig as useHooksConfigStore } from "../stores/hooksConfig";

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
  useHooksConfigStore.setState({ disabledHooks: [], loaded: false });
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
    resetStores();
  });

  afterEach(() => {
    cleanup();
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
    // 模式容器 Stage 03 占位文案
    expect(getByText("配置编辑区将在后续阶段实现（GUI / JSON 模式）")).toBeTruthy();
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

  it("面板聚焦（focusin）触发轻量重读（外部修改检测）", async () => {
    seedProject("C:/proj");
    mockReadHooksConfig.mockResolvedValue({});
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBe(1));
    fireEvent.focus(container.firstElementChild as HTMLElement);
    await waitFor(() => expect(mockReadHooksConfig.mock.calls.length).toBeGreaterThanOrEqual(2));
  });
});
