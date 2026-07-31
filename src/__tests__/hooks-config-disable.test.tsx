// hooks-config-disable.test.tsx — 单条启停禁用状态 L2 测试（P3-TE-15 + P3-TE-16）
//
// TE-15（禁用状态往返）：禁用 → 保存时 writeHooksConfig payload 不含禁用条目 →
// store 持久化 disabledHooks（debounce 自动保存）→ 重载后禁用状态保留 →
// 重新启用后条目恢复（原位置不丢失，保存时不再剔除）。
// TE-16（失效禁用记录）：手动改 JSON 使四元组失配 → UI 显示「失效的禁用记录」→
// 点击启用（= 删除失效记录）后标记消失；注入段条目（isSltermManaged 命中）
// 不渲染禁用 checkbox（C13-8 禁禁用）。
//
// 测试模式照 hooks-config-sync.test.tsx：mock ipc/hooksConfig + ipc/dialog + ipc/settings；
// useHooksConfig 保存路径 renderHook 直测；失效条/常驻提示渲染真实 HooksConfigPanel
// （mock JsonMode 隔离 CM6）；事件树 checkbox 用真实 GuiMode 渲染验证。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockReadHooksConfig, mockWriteHooksConfig, mockAsk, mockLoadSettings, mockSaveSettings, mockJsonMode } =
  vi.hoisted(() => ({
    mockReadHooksConfig: vi.fn(),
    mockWriteHooksConfig: vi.fn().mockResolvedValue(undefined),
    mockAsk: vi.fn().mockResolvedValue(true),
    mockLoadSettings: vi.fn().mockResolvedValue(null),
    mockSaveSettings: vi.fn().mockResolvedValue(undefined),
    // JsonMode mock：渲染 null，测试经 mock 调用参数断言（失效条不依赖 JSON 编辑器）
    mockJsonMode: vi.fn(() => null),
  }));

// mock IPC hooksConfig —— 三层 hooks 子树读写
vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: mockReadHooksConfig,
  writeHooksConfig: mockWriteHooksConfig,
}));

// mock IPC dialog —— 保存失败弹窗 + dirty 确认（不弹真实对话框）
vi.mock("../ipc/dialog", () => ({
  ask: mockAsk,
}));

// mock IPC settings —— hooksConfig store loadFromDisk 的后端读 + debounce 自动保存
vi.mock("../ipc/settings", () => ({
  loadSettings: mockLoadSettings,
  saveSettings: mockSaveSettings,
}));

// mock JsonMode —— 隔离 CM6（失效条 / 常驻提示断言不需要真实 JSON 编辑器）
vi.mock("../panels/hooksConfig/JsonMode", () => ({ default: mockJsonMode }));

import React, { useState } from "react";
import { render, fireEvent, waitFor, act, cleanup, renderHook } from "@testing-library/react";
import { HooksConfigPanel } from "../panels/hooksConfig";
import { useHooksConfig } from "../panels/hooksConfig/useHooksConfig";
import GuiMode from "../panels/hooksConfig/GuiMode";
import type { HookEventGui, HookHandlerGui, HooksConfigGui } from "../panels/hooksConfig/configModel";
import { UNKNOWN_EVENT_GROUP } from "../panels/hooksConfig/configModel";
import { getEventMeta } from "../panels/hooksConfig/eventsCatalog";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { useHooksConfig as useHooksConfigStore, cancelPendingSave } from "../stores/hooksConfig";
import type { HooksConfigJson, DisabledHookKey } from "../types/hooksConfig";

/** 基线合法 hooks 子树（通过 schema 校验）：同组两条 command handler */
const VALID_BASE: HooksConfigJson = {
  PreToolUse: [
    {
      hooks: [
        { type: "command", command: "echo hi" },
        { type: "command", command: "echo keep" },
      ],
    },
  ],
};

/** 被禁用 handler 的禁用 key（user 层） */
const DISABLED_KEY: DisabledHookKey = {
  layer: "user",
  event: "PreToolUse",
  matcher: null,
  command: "echo hi",
};

/** 失配 key：配置中不存在该 command（外部修改/手动改 JSON 导致失配） */
const STALE_KEY: DisabledHookKey = {
  layer: "user",
  event: "PreToolUse",
  matcher: null,
  command: "echo GONE",
};

// ── 辅助：种子 stores（照 hooks-config-sync.test.tsx）──
function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  useHooksConfigStore.setState({ disabledHooks: [], loaded: false });
}

const byE2e = (container: HTMLElement, selector: string): HTMLElement =>
  container.querySelector(`[data-e2e="${selector}"]`) as HTMLElement;

// ═══════════════════════════════════════════════════════════════════
// P3-TE-15 禁用状态往返
// ═══════════════════════════════════════════════════════════════════
describe("P3-TE-15 禁用状态往返", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockLoadSettings.mockReset();
    mockLoadSettings.mockResolvedValue(null);
    mockSaveSettings.mockClear();
    resetStores();
  });

  afterEach(() => {
    cancelPendingSave(); // 清理 store 模块级 debounce timer
    vi.useRealTimers(); // 用例内开启 fake timers 后恢复（防 waitFor 冻结泄漏到后续用例）
    cleanup();
  });

  it("禁用 handler → 保存时 writeHooksConfig payload 不含禁用条目（同组其余保留）→ dirty 置位", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    // 禁用 command "echo hi"（toggleDisable 补全当前层 user）
    act(() => {
      result.current.toggleDisable({ event: "PreToolUse", matcher: null, command: "echo hi" });
    });
    // 启停切换置 dirty（保存按钮可达——剔除需经保存写盘才生效到 claude 配置）
    expect(result.current.dirty).toBe(true);

    await act(async () => {
      await result.current.save();
    });

    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    // 被禁 handler 剔除，同组未禁 handler 保留（matcher 组整体保留）
    expect(mockWriteHooksConfig.mock.calls[0][1]).toEqual({
      PreToolUse: [{ hooks: [{ type: "command", command: "echo keep" }] }],
    });
  });

  it("禁用记录持久化到 store disabledHooks → 2s debounce 自动保存 slTerminal settings", async () => {
    vi.useFakeTimers();
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    mockLoadSettings.mockResolvedValue(null);
    const { result } = renderHook(() => useHooksConfig());
    // fake timers 下 waitFor 轮询冻结——flush 微任务等待 read/loadFromDisk 完成
    await act(async () => {});
    await act(async () => {});
    expect(result.current.loading).toBe(false);

    act(() => {
      result.current.toggleDisable({ event: "PreToolUse", matcher: null, command: "echo hi" });
    });
    // store 即时持有禁用记录（slTerminal 侧状态，与 claude 配置保存解耦）
    expect(useHooksConfigStore.getState().disabledHooks).toEqual([DISABLED_KEY]);

    // debounce 自动保存：payload 键集合精确匹配 { disabledHooks }
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [DISABLED_KEY] });

    // 禁用状态派生到 useHooksConfig（供 GUI checkbox / 保存过滤）
    expect(result.current.disabledKeys).toEqual([DISABLED_KEY]);
    // 记录匹配配置 → 非失效
    expect(result.current.staleDisabledKeys).toEqual([]);
  });

  it("重载后禁用状态保留：loadFromDisk 恢复 disabledHooks → disabledKeys 派生正确", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    mockLoadSettings.mockResolvedValue({ disabledHooks: [DISABLED_KEY] });
    const { result } = renderHook(() => useHooksConfig());
    // 挂载 effect 调 loadFromDisk → store 恢复禁用记录
    await waitFor(() => expect(useHooksConfigStore.getState().loaded).toBe(true));
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(useHooksConfigStore.getState().disabledHooks).toEqual([DISABLED_KEY]);
    expect(result.current.disabledKeys).toEqual([DISABLED_KEY]);
  });

  it("重新启用后条目恢复：toggleDisable 两次 → 记录移除 → 保存 payload 含原条目", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    const { result } = renderHook(() => useHooksConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));

    // 禁用 → 再启用（同一四元组）
    act(() => {
      result.current.toggleDisable({ event: "PreToolUse", matcher: null, command: "echo hi" });
    });
    expect(useHooksConfigStore.getState().disabledHooks).toEqual([DISABLED_KEY]);
    act(() => {
      result.current.toggleDisable({ event: "PreToolUse", matcher: null, command: "echo hi" });
    });
    expect(useHooksConfigStore.getState().disabledHooks).toEqual([]);
    expect(result.current.disabledKeys).toEqual([]);

    await act(async () => {
      await result.current.save();
    });
    // 条目按原位置恢复（禁用从未从 GUI/JSON 移除，保存时不再剔除）
    expect(mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    expect(mockWriteHooksConfig.mock.calls[0][1]).toEqual(VALID_BASE);
  });
});

// ═══════════════════════════════════════════════════════════════════
// P3-TE-16 失效禁用记录
// ═══════════════════════════════════════════════════════════════════
describe("P3-TE-16 失效禁用记录", () => {
  beforeEach(() => {
    mockReadHooksConfig.mockReset();
    mockWriteHooksConfig.mockReset();
    mockAsk.mockReset();
    mockAsk.mockResolvedValue(true);
    mockLoadSettings.mockReset();
    mockLoadSettings.mockResolvedValue(null);
    mockSaveSettings.mockClear();
    mockJsonMode.mockClear();
    resetStores();
  });

  afterEach(() => {
    cancelPendingSave();
    cleanup();
  });

  it("四元组失配（配置中不存在该 command）→ UI 显示「失效的禁用记录」", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    // 种子失配记录：command "echo GONE" 在配置中不存在
    useHooksConfigStore.setState({ disabledHooks: [STALE_KEY], loaded: true });
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(mockJsonMode).toHaveBeenCalled());

    // 失效条显示 + 常驻提示显示（跨模式）
    const staleBar = byE2e(container, "hooks-stale-disabled");
    expect(staleBar).toBeTruthy();
    expect(staleBar.textContent).toContain("失效的禁用记录");
    expect(staleBar.textContent).toContain("PreToolUse");
    expect(staleBar.textContent).toContain("echo GONE");
    expect(byE2e(container, "hooks-disabled-hint").textContent).toContain(
      "禁用条目由 slTerminal 托管，不出现在配置文件中",
    );
  });

  it("点击启用（= 删除失效记录）→ 记录移除 → 失效标记消失", async () => {
    mockReadHooksConfig.mockResolvedValue(VALID_BASE);
    useHooksConfigStore.setState({ disabledHooks: [STALE_KEY], loaded: true });
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(byE2e(container, "hooks-stale-disabled")).toBeTruthy());

    fireEvent.click(byE2e(container, "hooks-stale-enable-0"));
    await waitFor(() => expect(byE2e(container, "hooks-stale-disabled")).toBeNull());
    expect(useHooksConfigStore.getState().disabledHooks).toEqual([]);
  });

  it("失效判定随配置重载更新：失配记录在重载配置恢复匹配后不再标记失效", async () => {
    mockReadHooksConfig.mockResolvedValueOnce(VALID_BASE);
    mockReadHooksConfig.mockResolvedValueOnce({
      PreToolUse: [
        { hooks: [{ type: "command", command: "echo hi" }, { type: "command", command: "echo GONE" }] },
      ],
    });
    useHooksConfigStore.setState({ disabledHooks: [STALE_KEY], loaded: true });
    const { container } = render(React.createElement(HooksConfigPanel));
    await waitFor(() => expect(byE2e(container, "hooks-stale-disabled")).toBeTruthy());

    // 模拟外部修改：重新 read 返回含 "echo GONE" 的配置（reload 无 dirty 直接重读，
    // 照 hooks-config-panel.test.tsx 的 focus 触发模式）
    await act(async () => {
      fireEvent.focus(container.firstElementChild as HTMLElement);
    });
    await waitFor(() => expect(byE2e(container, "hooks-stale-disabled")).toBeNull());
  });
});

// ═══════════════════════════════════════════════════════════════════
// P3-TE-16 补充：事件树启停 checkbox（真实 GuiMode 渲染）
// ═══════════════════════════════════════════════════════════════════
describe("P3-TE-16 事件树启停 checkbox", () => {
  afterEach(() => {
    cleanup();
  });

  /** 构造单事件 GUI 条目（照 hooks-config-gui.test.tsx 模式） */
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

  /** 受控 Harness（模拟父层受控链路，照 hooks-config-gui.test.tsx） */
  function Harness({
    initialGui,
    disabledKeys,
    onToggleDisabled,
  }: {
    initialGui: HooksConfigGui;
    disabledKeys?: DisabledHookKey[];
    onToggleDisabled?: (key: Omit<DisabledHookKey, "layer">) => void;
  }) {
    const [gui, setGui] = useState(initialGui);
    return React.createElement(GuiMode, {
      gui,
      onChange: (g: HooksConfigGui) => setGui(g),
      disabledKeys,
      onToggleDisabled,
    });
  }

  function renderGui(
    initialGui: HooksConfigGui,
    opts: { disabledKeys?: DisabledHookKey[]; onToggleDisabled?: (key: Omit<DisabledHookKey, "layer">) => void } = {},
  ) {
    return render(React.createElement(Harness, { initialGui, ...opts }));
  }

  it("每条 handler 显示启停 checkbox：普通条目有、注入段条目无（C13-8 禁禁用）", () => {
    const gui: HooksConfigGui = {
      events: [
        makeEvent("PreToolUse", [
          {
            matcher: "Bash",
            handlers: [managedHandler, { type: "command", command: "echo hi" }],
          },
        ]),
      ],
    };
    const { container } = renderGui(gui);

    // 注入段条目行：无禁用 checkbox（仅托管标记）
    const managedRow = byE2e(container, "gui-tree-handler-PreToolUse-0-0");
    expect(managedRow).toBeTruthy();
    expect(managedRow.querySelector('input[type="checkbox"]')).toBeNull();
    expect(managedRow.textContent).toContain("slTerminal 托管");

    // 普通条目行：有禁用 checkbox 且未勾选
    const normalRow = byE2e(container, "gui-tree-handler-PreToolUse-0-1");
    const cb = normalRow.querySelector('input[type="checkbox"]') as HTMLInputElement | null;
    expect(cb).not.toBeNull();
    expect(cb?.checked).toBe(false);
  });

  it("点击启停 checkbox → onToggleDisabled 携带四元组（matcher/command 映射正确）", () => {
    const gui: HooksConfigGui = {
      events: [
        makeEvent("PreToolUse", [
          { matcher: "Bash", handlers: [{ type: "command", command: "echo hi" }] },
        ]),
      ],
    };
    const onToggleDisabled = vi.fn();
    const { container } = renderGui(gui, { onToggleDisabled });

    const cb = byE2e(container, "gui-tree-disable-PreToolUse-0-0") as HTMLInputElement;
    fireEvent.click(cb);
    // matcher 非空 → 原样传递；command 型 → command 值
    expect(onToggleDisabled).toHaveBeenCalledWith({
      event: "PreToolUse",
      matcher: "Bash",
      command: "echo hi",
    });
  });

  it("禁用条目视觉区分：置灰 + 文字删除线 + checkbox 勾选；启用态恢复", () => {
    const gui: HooksConfigGui = {
      events: [
        makeEvent("PreToolUse", [
          { handlers: [{ type: "command", command: "echo hi" }] },
        ]),
      ],
    };
    // 禁用记录命中（matcher null ↔ 省略 matcher 键 = 全匹配）
    const disabledKeys: DisabledHookKey[] = [
      { layer: "user", event: "PreToolUse", matcher: null, command: "echo hi" },
    ];
    const { container } = renderGui(gui, { disabledKeys });

    const row = byE2e(container, "gui-tree-handler-PreToolUse-0-0");
    const cb = row.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(cb.checked).toBe(true);
    // 摘要 span 删除线 + 行置灰（DIM_FG = #999999）
    const summary = row.querySelector("span") as HTMLElement;
    expect(summary.style.textDecoration).toContain("line-through");
    expect(row.style.color).toBe("rgb(153, 153, 153)");

    // 重新启用（记录移除）→ 视觉恢复
    const { container: c2 } = renderGui(gui, { disabledKeys: [] });
    const row2 = byE2e(c2, "gui-tree-handler-PreToolUse-0-0");
    const cb2 = row2.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(cb2.checked).toBe(false);
    expect(row2.querySelector("span")?.style.textDecoration).not.toContain("line-through");
  });
});
