// settings-panel-dirty.test.tsx — 设置中心壳 dirty 汇聚守卫 L2 测试（F11，SC-FE-07）
//
// 覆盖：切配置页守卫三态（dirty → confirmDialog 确认切换 / 取消不切换 /
// 非 dirty 直切）+ 导航项 dirty 圆点显隐 + 确认丢弃后圆点清除 + dirtyRegistry 同步
// （壳与 DefaultTab × 关闭守卫共享真值源）。
//
// mock 边界：pages.ts 注册 side-effect 被 mock 为空（防真实配置页副作用——hooks 页
// 挂载会触发 hooks IPC）；注册表真实 + 测试自管桩页（P1Page/P2Page 渲染可识别标记）；
// saveLayout mock（persistParams 显式保存链）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockConfirmDialog, mockSaveLayout } = vi.hoisted(() => ({
  mockConfirmDialog: vi.fn(async () => true),
  mockSaveLayout: vi.fn(() => ({ mockLayout: true })),
}));

// 注册 side-effect mock 为空：SettingsPanel import 本模块触发注册，但测试自管
// 桩页注册（防真实 planBalance/hooks 页副作用——hooks 页挂载会触发 hooks IPC）
vi.mock("../features/settingsCenter/pages", () => ({}));

// mock ConfirmDialog（src/lib barrel 再导出）——切页守卫确认弹窗（SC-FE-07）
vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: mockConfirmDialog,
}));

// mock IPC settings —— corrupted 警示条 loadSettings
vi.mock("../ipc/settings", () => ({
  loadSettings: vi.fn(async () => ({ data: null, corrupted: false })),
  saveSettings: vi.fn(async () => {}),
}));

// mock saveLayout —— persistParams 显式保存链终点（updateParameters 不触发
// onDidLayoutChange，壳必须显式保存）
vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: mockSaveLayout,
}));

import SettingsPanel from "../panels/settings/SettingsPanel";
import { getSettingsPageRegistry } from "../features/settingsCenter";
import type { SettingsPageProps } from "../features/settingsCenter/types";
import { clearSettingsDirty, isSettingsDirty } from "../features/settingsCenter/dirtyRegistry";

/** 桩页 P1：可经按钮上报 dirty（模拟真实配置页编辑） */
const P1Page: React.FC<SettingsPageProps> = ({ onDirtyChange }) => (
  <div data-e2e="page-p1">
    <button data-e2e="p1-dirty" onClick={() => onDirtyChange?.(true)}>
      置 dirty
    </button>
    <button data-e2e="p1-clean" onClick={() => onDirtyChange?.(false)}>
      清 dirty
    </button>
  </div>
);

/** 桩页 P2：渲染可识别标记（切页断言目标） */
const P2Page: React.FC<SettingsPageProps> = () => <div data-e2e="page-p2" />;

// ── 壳 props mock（SettingsPanel 为 Dockview content component）──
const mockApi = {
  updateParameters: vi.fn(),
  onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
  getParameters: vi.fn(() => ({})),
};
const mockContainerApi = { toJSON: vi.fn(() => ({ mockLayout: true })) };

const PANEL_ID = "settings-page-a";

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

describe("SettingsPanel dirty 汇聚守卫（SC-FE-07）", () => {
  beforeEach(() => {
    mockConfirmDialog.mockReset();
    mockConfirmDialog.mockResolvedValue(true);
    mockSaveLayout.mockClear();
    mockApi.updateParameters.mockReset();
    mockApi.getParameters.mockReset();
    mockApi.getParameters.mockReturnValue({});
    // 注册表自管基线：两桩页（global 组，order 序）
    getSettingsPageRegistry()._reset();
    getSettingsPageRegistry().register({
      id: "p1",
      title: "页一",
      group: "global",
      component: P1Page,
      order: 1,
    });
    getSettingsPageRegistry().register({
      id: "p2",
      title: "页二",
      group: "global",
      component: P2Page,
      order: 2,
    });
    clearSettingsDirty(PANEL_ID);
  });

  afterEach(() => {
    cleanup();
    getSettingsPageRegistry()._reset();
    clearSettingsDirty(PANEL_ID);
  });

  it("导航项 dirty 圆点：dirty 上报 → 圆点出现；清 dirty → 圆点消失", async () => {
    const { container } = renderPanel();
    // 初始无 dirty → 无圆点
    expect(byE2e(container, "settings-nav-dirty-p1")).toBeNull();
    // 桩页上报 dirty → 圆点出现（7px 中性色 token，SC-FE-07 口径）
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(byE2e(container, "settings-nav-dirty-p1")).toBeTruthy());
    // 清 dirty → 圆点消失
    fireEvent.click(byE2e(container, "p1-clean") as HTMLElement);
    await waitFor(() => expect(byE2e(container, "settings-nav-dirty-p1")).toBeNull());
  });

  it("dirty 上报同步 dirtyRegistry（× 关闭守卫真值源）", async () => {
    const { container, unmount } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    // 卸载 → clearSettingsDirty（面板关闭后无「未保存修改」）
    unmount();
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("dirty 切页 → confirmDialog 确认 → 切换 + 清当前页圆点 + dirtyRegistry 同步", async () => {
    const { container } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    mockConfirmDialog.mockResolvedValue(true); // 用户确认丢弃
    // 点击 p2 导航 → 确认弹窗（文案契约：切换丢弃）
    fireEvent.click(byE2e(container, "settings-nav-p2") as HTMLElement);
    await waitFor(() => expect(mockConfirmDialog).toHaveBeenCalled());
    expect(mockConfirmDialog).toHaveBeenCalledWith({
      title: "未保存的修改",
      message: "当前配置页有未保存的修改，切换将丢弃这些修改。",
      kind: "warning",
    });
    // 确认后切换：p2 渲染 + 选中态持久化
    await waitFor(() => expect(byE2e(container, "page-p2")).toBeTruthy());
    expect(mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ selectedPage: "p2" }),
    );
    expect(mockSaveLayout).toHaveBeenCalled();
    // 确认丢弃后清 dirty：p1 圆点消失 + dirtyRegistry 同步 false
    await waitFor(() => expect(byE2e(container, "settings-nav-dirty-p1")).toBeNull());
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("dirty 切页 → confirmDialog 取消 → 不切换（选中态保持，patch 零调用）", async () => {
    const { container } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    mockConfirmDialog.mockResolvedValue(false); // 用户取消丢弃
    fireEvent.click(byE2e(container, "settings-nav-p2") as HTMLElement);
    await waitFor(() => expect(mockConfirmDialog).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 20));
    // 不切换：p2 未渲染、updateParameters 未被调用、p1 仍在
    expect(byE2e(container, "page-p2")).toBeNull();
    expect(byE2e(container, "page-p1")).toBeTruthy();
    expect(mockApi.updateParameters).not.toHaveBeenCalled();
    // 取消不丢弃：dirty 保留（圆点仍在 + dirtyRegistry 不变）
    expect(byE2e(container, "settings-nav-dirty-p1")).toBeTruthy();
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
  });

  it("非 dirty 直切：confirmDialog 不被调用，直接切换", async () => {
    const { container } = renderPanel();
    // 无 dirty 上报 → 点击 p2 → 直接切换
    fireEvent.click(byE2e(container, "settings-nav-p2") as HTMLElement);
    await waitFor(() => expect(byE2e(container, "page-p2")).toBeTruthy());
    expect(mockConfirmDialog).not.toHaveBeenCalled();
    expect(mockApi.updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ selectedPage: "p2" }),
    );
  });

  it("确认丢弃后圆点清除：后续 clean 上报不再误弹守卫（dirty 状态已清）", async () => {
    const { container } = renderPanel();
    fireEvent.click(byE2e(container, "p1-dirty") as HTMLElement);
    await waitFor(() => expect(isSettingsDirty(PANEL_ID)).toBe(true));
    // 确认切换 p2
    fireEvent.click(byE2e(container, "settings-nav-p2") as HTMLElement);
    await waitFor(() => expect(byE2e(container, "page-p2")).toBeTruthy());
    mockConfirmDialog.mockClear();
    // 从 p2 切回 p1：p1 已确认丢弃（非 dirty）→ 直切不弹窗
    fireEvent.click(byE2e(container, "settings-nav-p1") as HTMLElement);
    await waitFor(() => expect(byE2e(container, "page-p1")).toBeTruthy());
    expect(mockConfirmDialog).not.toHaveBeenCalled();
  });
});
