// terminal-interrupt-status.test.tsx — TerminalPanel 本地中断状态转换（CP-020）
//
// 防复发对照：修复前 Ctrl+C 中断后页签滞留 working（无本地事件源）——本测试锁定
// handleInterrupt 仅在 tabStatus=working 时置 attention（幂等），done/null 不调用。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import TerminalPanel from "../panels/terminal/TerminalPanel";
import type { TerminalPanelProps } from "../panels/terminal/TerminalPanel";
import type { UseXtermOptions, UseXtermReturn } from "../panels/terminal/useXterm";
import type { DockviewPanelApi } from "dockview-react";

// ─── Hoisted mocks ───

const { mockUseXterm } = vi.hoisted(() => ({
  // 默认返回 UseXtermReturn 完整形态（组件解构 focus，onClick 依赖）
  mockUseXterm: vi.fn<(opts: UseXtermOptions) => UseXtermReturn>(() => ({
    focus: vi.fn(),
    _test: { cancelPendingFlush: vi.fn(), flushBuffer: vi.fn(), getPendingBuffer: () => [] },
  })),
}));

// useXterm 模块整体 mock——捕获 options（含 onInterrupt）供用例触发
vi.mock("../panels/terminal/useXterm", () => ({
  useXterm: mockUseXterm,
}));

// TerminalPanel 依赖的 PTY IPC（仅用 getWindowsBuildNumber 动态 build 号）
vi.mock("../ipc", () => ({
  pty: {
    getWindowsBuildNumber: vi.fn().mockResolvedValue(22621),
  },
}));

// TerminalRegistry 单点元数据——本测试不涉及会话，get 恒空、subscribe 空操作
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    get: () => undefined,
    subscribe: () => () => {},
  },
}));

// ─── 辅助 ───

/** 构造 Dockview api 桩：updateParameters 同步回放参数变化（贴近真实 dockview 语义，
 *  使组件侧 latestParamsRef 随 updateParameters 合并——幂等断言依赖此往返） */
function makeDockviewApi(initialParams: Record<string, unknown>) {
  const paramState: Record<string, unknown> = { ...initialParams };
  const listeners: Array<(patch: Record<string, unknown>) => void> = [];
  const api = {
    title: "terminal",
    updateParameters: vi.fn((patch: Record<string, unknown>) => {
      Object.assign(paramState, patch);
      for (const l of listeners) l(patch);
    }),
    onDidParametersChange: vi.fn((listener: (patch: Record<string, unknown>) => void) => {
      listeners.push(listener);
      return { dispose: () => {} };
    }),
    onDidTitleChange: vi.fn(() => ({ dispose: () => {} })),
    setTitle: vi.fn(),
  };
  return { api: api as unknown as DockviewPanelApi, paramState };
}

/** 当前渲染绑定的 onInterrupt（取最近一次 useXterm 调用的 options——buildNumber
 *  异步就绪会重渲染，handleInterrupt 以 [api] 为依赖恒为同一实例） */
function getOnInterrupt() {
  const calls = mockUseXterm.mock.calls;
  return calls[calls.length - 1]![0].onInterrupt!;
}

/** 以指定 tabStatus 挂载面板并等待挂载副作用落定（buildNumber promise + syncTabLogo） */
async function mountWithStatus(status: "working" | "done" | null) {
  const params = {
    panelId: "terminal-cp020-0",
    tabStatus: status,
  } as unknown as TerminalPanelProps["params"];
  const { api } = makeDockviewApi(params as unknown as Record<string, unknown>);
  const view = render(
    <TerminalPanel
      api={api}
      params={params}
    />,
  );
  // 冲掉挂载微任务：setBuildNumber 重渲染 + syncTabLogo 的 updateParameters(tabLogo)
  await act(async () => {});
  return { api, view };
}

// ─── 测试套件 ───

describe("TerminalPanel 本地中断状态（CP-020）", () => {
  beforeEach(() => {
    mockUseXterm.mockClear();
  });

  it("working → handleInterrupt → updateParameters 一次，tabStatus 置 attention", async () => {
    const { api, view } = await mountWithStatus("working");
    const updateParams = api.updateParameters as ReturnType<typeof vi.fn>;
    updateParams.mockClear(); // 清掉挂载期 syncTabLogo 的写入，专注本次断言

    act(() => {
      getOnInterrupt()();
    });

    expect(updateParams).toHaveBeenCalledTimes(1);
    expect(updateParams).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );
    view.unmount();
  });

  it("working → 双路径连续两次调用 → 第二次为 no-op（幂等，updateParameters 仅一次）", async () => {
    const { api, view } = await mountWithStatus("working");
    const updateParams = api.updateParameters as ReturnType<typeof vi.fn>;
    updateParams.mockClear();

    // window capture 与 xterm attachCustomKeyEventHandler 委托双路径各调一次
    act(() => {
      getOnInterrupt()();
      getOnInterrupt()();
    });

    // 第一次置 attention 后参数已回放（latestParamsRef.tabStatus=attention）→ 第二次拦截
    expect(updateParams).toHaveBeenCalledTimes(1);
    expect(updateParams).toHaveBeenCalledWith(
      expect.objectContaining({ tabStatus: "attention" }),
    );
    view.unmount();
  });

  it("done → handleInterrupt → 不调用（中断滞留 working 防复发对照）", async () => {
    const { api, view } = await mountWithStatus("done");
    const updateParams = api.updateParameters as ReturnType<typeof vi.fn>;
    updateParams.mockClear();

    act(() => {
      getOnInterrupt()();
    });

    expect(updateParams).not.toHaveBeenCalled();
    view.unmount();
  });

  it("null（无状态）→ handleInterrupt → 不调用", async () => {
    const { api, view } = await mountWithStatus(null);
    const updateParams = api.updateParameters as ReturnType<typeof vi.fn>;
    updateParams.mockClear();

    act(() => {
      getOnInterrupt()();
    });

    expect(updateParams).not.toHaveBeenCalled();
    view.unmount();
  });

  it("working 中 rerender 为 done → handleInterrupt 读最新参数 → 不调用", async () => {
    const { api, view } = await mountWithStatus("working");
    const updateParams = api.updateParameters as ReturnType<typeof vi.fn>;

    const paramsDone = {
      panelId: "terminal-cp020-0",
      tabStatus: "done",
    } as unknown as TerminalPanelProps["params"];
    view.rerender(
      <TerminalPanel
        api={api}
        params={paramsDone}
      />,
    );
    updateParams.mockClear();

    act(() => {
      getOnInterrupt()();
    });

    expect(updateParams).not.toHaveBeenCalled();
    view.unmount();
  });
});
