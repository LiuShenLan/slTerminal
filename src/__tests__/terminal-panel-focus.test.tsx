// terminal-panel-focus.test.tsx — TerminalPanel 键盘焦点联动接线（C4）
//
// 消费链：usePanelActivationFocus(api, panelId, useXterm 返回的 focus,
// ready = container !== null)——容器挂载（state 翻 true）后 ready 补消费
// 挂载期意图；激活事件路径不需意图。xterm 层全 mock，断言 focus 透传。

import { describe, it, expect, afterEach, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  focus: vi.fn(),
  pty: { getWindowsBuildNumber: vi.fn().mockResolvedValue(26100) },
}));

vi.mock("../panels/terminal/useXterm", () => ({
  useXterm: vi.fn(() => ({ focus: mocks.focus })),
}));

vi.mock("../ipc", () => ({
  pty: mocks.pty,
}));

import React from "react";
import { render, act, cleanup } from "@testing-library/react";
import TerminalPanel from "../panels/terminal/TerminalPanel";
import {
  markPanelFocusIntent,
  _resetPanelFocusIntent,
} from "../workspace/panelFocusIntent";

/** fake 面板 api：可控激活态 + 事件捕获（TerminalPanel 其他消费面一并桩齐） */
function fakePanelApi(init: { isActive: boolean; isGroupActive: boolean }) {
  const activeHandlers = new Set<() => void>();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const api: any = {
    title: "terminal-0",
    isActive: init.isActive,
    isGroupActive: init.isGroupActive,
    setTitle: vi.fn(),
    updateParameters: vi.fn(),
    onDidTitleChange: vi.fn(() => ({ dispose: vi.fn() })),
    onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
    onDidActiveChange: (cb: () => void) => {
      activeHandlers.add(cb);
      return { dispose: () => activeHandlers.delete(cb) };
    },
    onDidActiveGroupChange: vi.fn(() => ({ dispose: vi.fn() })),
    close: vi.fn(),
  };
  return {
    api,
    fireActive(isActive: boolean) {
      api.isActive = isActive;
      activeHandlers.forEach((h) => h());
    },
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  _resetPanelFocusIntent();
});

describe("TerminalPanel 键盘焦点联动（C4）", () => {
  it("新建意图 + 挂载即激活 → 容器就绪后 focus 透传（新建终端自动聚焦锚）", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });
    markPanelFocusIntent("page-f1:terminal-0");

    render(
      React.createElement(TerminalPanel, {
        api,
        params: { panelId: "page-f1:terminal-0" },
      }),
    );

    // container state effect → ready 翻 true → 意图补消费 → focus
    expect(mocks.focus).toHaveBeenCalledTimes(1);
  });

  it("无意图挂载即激活 → 不 focus（fromJSON 布局恢复豁免）", () => {
    const { api } = fakePanelApi({ isActive: true, isGroupActive: true });

    render(
      React.createElement(TerminalPanel, {
        api,
        params: { panelId: "page-f2:terminal-0" },
      }),
    );

    expect(mocks.focus).not.toHaveBeenCalled();
  });

  it("挂载未激活不 focus；激活事件（页签点击）→ focus（不需意图）", () => {
    const f = fakePanelApi({ isActive: false, isGroupActive: true });

    render(
      React.createElement(TerminalPanel, {
        api: f.api,
        params: { panelId: "page-f3:terminal-0" },
      }),
    );
    expect(mocks.focus).not.toHaveBeenCalled();

    act(() => f.fireActive(true));
    expect(mocks.focus).toHaveBeenCalledTimes(1);
  });
});
