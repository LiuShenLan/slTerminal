// settings-panel-autoclose.test.tsx — 设置中心壳切项目自动关闭 L2 测试（F11，SC-FE-08）
//
// 覆盖：切项目 → api.close 调用（非 dirty 直关）/ 同项目切页 → 不关 /
// 初始不一致（布局恢复场景）→ 挂载即静默关 / activePageId null → 不关（启动与删除末页瞬态，
// 防连锁误关）/ dirty 切项目 confirmDialog 取消 → 不关（面板暂留非活跃项目）/
// dirty 切项目 confirmDialog 确认 → 关。
//
// mock 边界：pages.ts 注册 side-effect 为空（防真实配置页副作用）；注册表真实 + 桩页；
// saveLayout / ConfirmDialog / ipc settings mock（照 settings-panel-dirty.test.tsx 先例）；
// useLayout / useProjects 为真实 store，beforeEach setState 种子（Zustand 测试模式）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, fireEvent, waitFor, act } from "@testing-library/react";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockConfirmDialog, mockSaveLayout } = vi.hoisted(() => ({
  mockConfirmDialog: vi.fn(async () => true),
  mockSaveLayout: vi.fn(() => ({ mockLayout: true })),
}));

// 注册 side-effect mock 为空：SettingsPanel import 本模块触发注册，但测试自管桩页
// （防真实 planBalance/hooks 页副作用）
vi.mock("../features/settingsCenter/pages", () => ({}));

// mock ConfirmDialog（src/lib barrel 再导出）——切项目关闭守卫确认弹窗（SC-FE-08）
vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: mockConfirmDialog,
}));

// mock IPC settings —— corrupted 警示条 loadSettings
vi.mock("../ipc/settings", () => ({
  loadSettings: vi.fn(async () => ({ data: null, corrupted: false })),
  saveSettings: vi.fn(async () => {}),
}));

// mock saveLayout —— persistParams 显式保存链终点
vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: mockSaveLayout,
}));

import SettingsPanel from "../panels/settings/SettingsPanel";
import { getSettingsPageRegistry } from "../features/settingsCenter";
import type { SettingsPageProps } from "../features/settingsCenter/types";
import {
  clearSettingsDirty,
  isSettingsDirty,
} from "../features/settingsCenter/dirtyRegistry";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

/** 桩页 P1：可经按钮上报 dirty（模拟真实配置页编辑） */
const P1Page: React.FC<SettingsPageProps> = ({ onDirtyChange }) => (
  <div data-e2e="page-p1">
    <button data-e2e="p1-dirty" onClick={() => onDirtyChange?.(true)}>
      置 dirty
    </button>
  </div>
);

// ── 壳 props mock（SettingsPanel 为 Dockview content component）──
const mockApi = {
  updateParameters: vi.fn(),
  onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
  getParameters: vi.fn(() => ({})),
  close: vi.fn(),
};
const mockContainerApi = { toJSON: vi.fn(() => ({ mockLayout: true })) };

// 面板 panelId = settings-page-a → ownPageId = "page-a"，属 projA（跨用例稳定）
const PANEL_ID = "settings-page-a";

// ── 项目/页面种子（两级：projA 两页 + projB 一页）──
const seedProjects = () => {
  useProjects.setState({
    projects: {
      projA: {
        projectId: "projA",
        name: "项目A",
        rootPath: "C:\\a",
        pages: [
          { pageId: "page-a", name: "页A", layout: {}, createdAt: 1, lastAccessedAt: 1 },
          { pageId: "page-a2", name: "页A2", layout: {}, createdAt: 2, lastAccessedAt: 2 },
        ],
        activePageId: "page-a",
        version: 1,
      },
      projB: {
        projectId: "projB",
        name: "项目B",
        rootPath: "C:\\b",
        pages: [
          { pageId: "page-b", name: "页B", layout: {}, createdAt: 1, lastAccessedAt: 1 },
        ],
        activePageId: "page-b",
        version: 1,
      },
    },
  });
};

function renderPanel(params?: Record<string, unknown>) {
  return render(
    React.createElement(SettingsPanel, {
      api: mockApi,
      containerApi: mockContainerApi,
      params: { panelId: PANEL_ID, ...params },
    } as unknown as React.ComponentProps<typeof SettingsPanel>),
  );
}

const byE2e = (container: HTMLElement, selector: string): HTMLElement | null =>
  container.querySelector(`[data-e2e="${selector}"]`);

describe("SettingsPanel 切项目自动关闭（SC-FE-08）", () => {
  beforeEach(() => {
    mockConfirmDialog.mockReset();
    mockConfirmDialog.mockResolvedValue(true);
    mockSaveLayout.mockClear();
    mockApi.updateParameters.mockReset();
    mockApi.close.mockReset();
    mockApi.getParameters.mockReset();
    mockApi.getParameters.mockReturnValue({});
    // 默认：活跃页 = page-a（面板自身项目），种子两项目
    useLayout.setState({ activePageId: "page-a" });
    seedProjects();
    // 注册表自管基线：单桩页（global 组）
    getSettingsPageRegistry()._reset();
    getSettingsPageRegistry().register({
      id: "p1",
      title: "页一",
      group: "global",
      component: P1Page,
      order: 1,
    });
    clearSettingsDirty(PANEL_ID);
  });

  afterEach(() => {
    cleanup();
    getSettingsPageRegistry()._reset();
    clearSettingsDirty(PANEL_ID);
  });

  it("切项目（page-a → page-b）→ api.close 调用（非 dirty 直关，confirmDialog 不弹）", async () => {
    renderPanel();
    expect(mockApi.close).not.toHaveBeenCalled();
    act(() => useLayout.setState({ activePageId: "page-b" }));
    await waitFor(() => expect(mockApi.close).toHaveBeenCalledTimes(1));
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("同项目切页（page-a → page-a2）→ 不关", async () => {
    renderPanel();
    act(() => useLayout.setState({ activePageId: "page-a2" }));
    await new Promise((r) => setTimeout(r, 20));
    expect(mockApi.close).not.toHaveBeenCalled();
  });

  it("初始不一致（布局恢复：挂载前 activePageId 已属他项目）→ 挂载即静默关", async () => {
    useLayout.setState({ activePageId: "page-b" });
    renderPanel();
    await waitFor(() => expect(mockApi.close).toHaveBeenCalledTimes(1));
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("activePageId 初始为 null（启动瞬态）→ 不关", async () => {
    useLayout.setState({ activePageId: null });
    renderPanel();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockApi.close).not.toHaveBeenCalled();
  });

  it("activePageId 变为 null（删除末页瞬态）→ 不关，防连锁误关", async () => {
    renderPanel();
    act(() => useLayout.setState({ activePageId: null }));
    await new Promise((r) => setTimeout(r, 20));
    expect(mockApi.close).not.toHaveBeenCalled();
  });

  it("dirty 切项目 → confirmDialog 取消 → 不关（面板暂留非活跃项目，尊重用户选择）", async () => {
    const { container } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    mockConfirmDialog.mockResolvedValue(false); // 用户取消丢弃
    act(() => useLayout.setState({ activePageId: "page-b" }));
    await waitFor(() => expect(mockConfirmDialog).toHaveBeenCalled());
    expect(mockConfirmDialog).toHaveBeenCalledWith({
      title: "未保存的修改",
      message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
      kind: "warning",
    });
    await new Promise((r) => setTimeout(r, 20));
    // 取消不关：面板仍在（渲染标记 + dirty 保留）
    expect(mockApi.close).not.toHaveBeenCalled();
    expect(byE2e(container, "settings-panel")).toBeTruthy();
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
  });

  it("dirty 切项目 → confirmDialog 确认 → api.close 调用", async () => {
    const { container } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    act(() => useLayout.setState({ activePageId: "page-b" }));
    await waitFor(() => expect(mockConfirmDialog).toHaveBeenCalled());
    await waitFor(() => expect(mockApi.close).toHaveBeenCalledTimes(1));
  });

  it("空 projects（未水合）首轮评估不关闭不消费 firstRun；水合后重跑走初始评估静默关闭", async () => {
    // 未水合：projects 空 + activePageId 已定（布局恢复场景）→ 首轮被水合门控拦截
    useProjects.setState({ projects: {} });
    useLayout.setState({ activePageId: "page-a" });
    const { container } = renderPanel();
    await new Promise((r) => setTimeout(r, 20));
    expect(mockApi.close).not.toHaveBeenCalled();
    // 证明首轮未消费 firstRun：置 dirty 后水合 + 切项目——若首轮已消费（isFirstRunRef=false），
    // 本轮将走变化触发（dirty → confirmDialog）；仍走初始评估则无视 dirty 静默关（confirmDialog 不弹）
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    act(() => {
      seedProjects(); // 水合：projects 非空，门控放行
      useLayout.setState({ activePageId: "page-b" }); // 他项目 → 触发关闭评估
    });
    await waitFor(() => expect(mockApi.close).toHaveBeenCalledTimes(1));
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("面板归属解析失败（panelId 无 settings- 前缀）→ 不动作，不误关", async () => {
    renderPanel({ panelId: "odd-panel-id" } as unknown as Record<string, unknown>);
    act(() => useLayout.setState({ activePageId: "page-b" }));
    await new Promise((r) => setTimeout(r, 20));
    expect(mockApi.close).not.toHaveBeenCalled();
  });
});
