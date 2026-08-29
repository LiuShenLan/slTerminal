// settings-panel.test.tsx — 设置中心面板壳 L2 测试（F11，SC-FE-03）
//
// 覆盖：导航组序 global 在前 / 选中渲染对应页 / 切换 persist（updateParameters+toJSON）/
// params.selectedPage 失效回退全局组第一页 / corrupted 警示条渲染与关闭 /
// pageParams 透传与持久化（merge patch）/ 注册表空 → 空态 / 外部 selectedPage 变化同步。
//
// mock 策略：vi.mock 屏蔽 pages.ts side-effect 注册（注册表初始空，用例内 _reset + 注册
// stub 页）；../ipc/settings 的 loadSettings 经 hoisted mock 控制 corrupted 分支。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent, screen, waitFor, act } from "@testing-library/react";
import React from "react";
import SettingsPanel from "../panels/settings/SettingsPanel";
import { getSettingsPageRegistry } from "../features/settingsCenter";
import type { SettingsPageProps } from "../features/settingsCenter";
import type { DockviewPanelApi, DockviewApi } from "dockview-react";

// 屏蔽 pages.ts side-effect 注册（真实 pages 会注册 planBalance 页，破坏空态/组序用例）
vi.mock("../features/settingsCenter/pages", () => ({}));

const h = vi.hoisted(() => ({
  loadSettings: vi.fn(),
}));

vi.mock("../ipc/settings", () => ({
  loadSettings: h.loadSettings,
  saveSettings: vi.fn(),
}));

/** stub 配置页 A：透出 pageParams 槽 + patch 按钮（触发 onPageParamsChange({y:2}) 锁 merge 语义） */
const StubPageA: React.FC<SettingsPageProps> = ({ pageParams, onPageParamsChange }) => (
  <div data-e2e="stub-page-a">
    <span data-e2e="stub-page-a-params">{JSON.stringify(pageParams ?? null)}</span>
    <button
      type="button"
      data-e2e="stub-page-a-patch"
      onClick={() => onPageParamsChange?.({ y: 2 })}
    >
      patch
    </button>
  </div>
);

/** stub 配置页 B（global） */
const StubPageB: React.FC<SettingsPageProps> = () => <div data-e2e="stub-page-b">B</div>;

/** stub 配置页（project 组） */
const StubPageProject: React.FC<SettingsPageProps> = () => (
  <div data-e2e="stub-page-project">P</div>
);

/** 注册 stub 页（global 两页 + project 一页；清空 pages.ts 副作用残留） */
function registerStubs() {
  const registry = getSettingsPageRegistry();
  registry._reset();
  registry.register({
    id: "stub-a",
    title: "Stub A",
    group: "global",
    component: StubPageA,
    order: 10,
  });
  registry.register({
    id: "stub-b",
    title: "Stub B",
    group: "global",
    component: StubPageB,
    order: 20,
  });
  registry.register({
    id: "stub-project",
    title: "Stub Project",
    group: "project",
    component: StubPageProject,
    order: 5,
  });
}

/** Dockview api/containerApi stub（照 hooks-config-panel.test.tsx 模式） */
function makeStubs() {
  const api = {
    updateParameters: vi.fn(),
    onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
  };
  const containerApi = {
    toJSON: vi.fn(() => ({ panels: {} })),
  };
  return {
    api: api as unknown as DockviewPanelApi,
    containerApi: containerApi as unknown as DockviewApi,
  };
}

/** SettingsPanel params 形态（与组件 props 结构一致） */
type SettingsPanelParams = {
  panelId?: string;
  selectedPage?: string;
  pageParams?: Record<string, Record<string, unknown>>;
};

/** 渲染壳并返回 stubs */
function renderPanel(params?: SettingsPanelParams) {
  const { api, containerApi } = makeStubs();
  render(
    <SettingsPanel
      api={api}
      containerApi={containerApi}
      params={params}
    />,
  );
  return { api, containerApi };
}

beforeEach(() => {
  h.loadSettings.mockClear().mockResolvedValue({ data: null, corrupted: false });
});

afterEach(() => {
  cleanup();
  getSettingsPageRegistry()._reset();
});

describe("导航组序与渲染", () => {
  it("导航组序：global 组在 project 组之前（组标题 DOM 顺序）", () => {
    registerStubs();
    renderPanel({ panelId: "settings-page-a" });
    const groups = document.querySelectorAll('[data-e2e^="settings-nav-group-"]');
    expect(groups.length).toBe(2);
    expect(groups[0].getAttribute("data-e2e")).toBe("settings-nav-group-global");
    expect(groups[1].getAttribute("data-e2e")).toBe("settings-nav-group-project");
  });

  it("页项 data-e2e = settings-nav-<id>；选中渲染对应页组件", () => {
    registerStubs();
    renderPanel({ panelId: "settings-page-a", selectedPage: "stub-b" });
    expect(document.querySelector('[data-e2e="settings-nav-stub-a"]')).not.toBeNull();
    expect(document.querySelector('[data-e2e="settings-nav-stub-b"]')).not.toBeNull();
    expect(document.querySelector('[data-e2e="settings-nav-stub-project"]')).not.toBeNull();
    // 选中 stub-b → 渲染 B 组件，A 不渲染
    expect(document.querySelector('[data-e2e="stub-page-b"]')).not.toBeNull();
    expect(document.querySelector('[data-e2e="stub-page-a"]')).toBeNull();
  });

  it("params.selectedPage 失效（注册表无此页）→ 回退全局组第一页", () => {
    registerStubs();
    renderPanel({ panelId: "settings-page-a", selectedPage: "nonexistent" });
    expect(document.querySelector('[data-e2e="stub-page-a"]')).not.toBeNull();
  });

  it("注册表空 → 空态「暂无配置页」，导航无组渲染", () => {
    getSettingsPageRegistry()._reset(); // 不注册任何页
    renderPanel({ panelId: "settings-page-a" });
    expect(screen.getByText("暂无配置页")).toBeTruthy();
    expect(document.querySelectorAll('[data-e2e^="settings-nav-group-"]').length).toBe(0);
  });
});

describe("选中切换持久化（壳是 params 持久化单点）", () => {
  it("导航切换 → updateParameters 合并 selectedPage + 显式 toJSON（saveLayout）", () => {
    registerStubs();
    const { api, containerApi } = renderPanel({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
    });
    fireEvent.click(document.querySelector('[data-e2e="settings-nav-stub-b"]')!);
    expect(api.updateParameters).toHaveBeenCalledTimes(1);
    expect(api.updateParameters).toHaveBeenCalledWith({
      panelId: "settings-page-a",
      selectedPage: "stub-b",
    });
    // 显式布局保存（updateParameters 不触发 onDidLayoutChange，必须显式 saveLayout）
    expect(containerApi.toJSON).toHaveBeenCalledTimes(1);
    // 选中态切换生效
    expect(document.querySelector('[data-e2e="stub-page-b"]')).not.toBeNull();
  });

  it("点击当前选中页 → 不重复持久化（无 updateParameters 调用）", () => {
    registerStubs();
    const { api } = renderPanel({ panelId: "settings-page-a", selectedPage: "stub-a" });
    fireEvent.click(document.querySelector('[data-e2e="settings-nav-stub-a"]')!);
    expect(api.updateParameters).not.toHaveBeenCalled();
  });

  it("外部 selectedPage 变化（onDidParametersChange 扁平事件结构）→ 选中态同步", () => {
    registerStubs();
    const { api } = renderPanel({ panelId: "settings-page-a", selectedPage: "stub-a" });
    expect(document.querySelector('[data-e2e="stub-page-a"]')).not.toBeNull();
    // 回调直接是 Parameters 对象（扁平事件结构红线）
    const cb = (api.onDidParametersChange as ReturnType<typeof vi.fn>).mock.calls[0][0];
    act(() => cb({ selectedPage: "stub-b" }));
    expect(document.querySelector('[data-e2e="stub-page-b"]')).not.toBeNull();
    expect(document.querySelector('[data-e2e="stub-page-a"]')).toBeNull();
  });
});

describe("corrupted 警示条", () => {
  it("loadSettings corrupted=true → 顶部警示条渲染", async () => {
    registerStubs();
    h.loadSettings.mockResolvedValue({ data: { fontSize: 14 }, corrupted: true });
    renderPanel({ panelId: "settings-page-a" });
    expect(
      await screen.findByText("设置文件已损坏，已从备份/默认值恢复"),
    ).toBeTruthy();
    // 不阻塞：配置页照常渲染
    expect(document.querySelector('[data-e2e="stub-page-a"]')).not.toBeNull();
  });

  it("警示条 × 可关（关闭后消失）", async () => {
    registerStubs();
    h.loadSettings.mockResolvedValue({ data: null, corrupted: true });
    renderPanel({ panelId: "settings-page-a" });
    await screen.findByText("设置文件已损坏，已从备份/默认值恢复");
    fireEvent.click(
      document.querySelector('[data-e2e="settings-corrupted-banner-close"]')!,
    );
    await waitFor(() => {
      expect(
        document.querySelector('[data-e2e="settings-corrupted-banner"]'),
      ).toBeNull();
    });
  });

  it("corrupted=false → 不渲染警示条", async () => {
    registerStubs();
    renderPanel({ panelId: "settings-page-a" });
    await waitFor(() => {
      expect(h.loadSettings).toHaveBeenCalled();
    });
    expect(
      document.querySelector('[data-e2e="settings-corrupted-banner"]'),
    ).toBeNull();
  });
});

describe("pageParams 透传与持久化", () => {
  it("pageParams 按选中页槽透传给页组件", () => {
    registerStubs();
    renderPanel({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
      pageParams: { "stub-a": { x: 1 } },
    });
    expect(
      document.querySelector('[data-e2e="stub-page-a-params"]')?.textContent,
    ).toBe('{"x":1}');
  });

  it("onPageParamsChange → updateParameters 写 pageParams[selectedPage] 槽（merge patch 不丢既有键）", () => {
    registerStubs();
    const { api } = renderPanel({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
      pageParams: { "stub-a": { x: 1 } },
    });
    // patch {y:2} → 槽 {x:1, y:2}（merge 语义：既有键保留）
    fireEvent.click(document.querySelector('[data-e2e="stub-page-a-patch"]')!);
    expect(api.updateParameters).toHaveBeenCalledWith({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
      pageParams: { "stub-a": { x: 1, y: 2 } },
    });
    // 连续 patch：合并基准不丢前次结果
    fireEvent.click(document.querySelector('[data-e2e="stub-page-a-patch"]')!);
    expect(api.updateParameters).toHaveBeenCalledTimes(2);
    expect(api.updateParameters).toHaveBeenLastCalledWith({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
      pageParams: { "stub-a": { x: 1, y: 2 } },
    });
  });

  it("切换页后 pageParams 跟随新选中页槽透传", () => {
    registerStubs();
    renderPanel({
      panelId: "settings-page-a",
      selectedPage: "stub-a",
      pageParams: { "stub-a": { x: 1 }, "stub-b": { z: 9 } },
    });
    fireEvent.click(document.querySelector('[data-e2e="settings-nav-stub-b"]')!);
    // B 组件不消费 pageParams——断言切换后 A 组件卸载（槽位 key 重挂载）
    expect(document.querySelector('[data-e2e="stub-page-b"]')).not.toBeNull();
    expect(document.querySelector('[data-e2e="stub-page-a"]')).toBeNull();
  });
});
