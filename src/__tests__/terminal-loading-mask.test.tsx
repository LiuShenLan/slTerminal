// L2 终端加载遮罩测试——首帧驱动隐藏 + 1500ms 超时兜底
// 防复发锚：旧实现遮罩仅靠固定 1500ms 定时器隐藏（与首帧脱钩），
// 每次新建终端必现 1.5s 保底感知延迟（「打开慢」回归根因）
import { describe, it, expect, afterEach, vi } from "vitest";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  // useXterm 捕获 onFirstOutput 回调（生产链 = usePtyOutput 首块 PTY 输出触发）
  let onFirstOutput: (() => void) | null = null;
  return {
    focus: vi.fn(),
    pty: { getWindowsBuildNumber: vi.fn().mockResolvedValue(26100) },
    capture: (cb: (() => void) | undefined) => {
      onFirstOutput = cb ?? null;
    },
    fireFirstOutput: () => onFirstOutput?.(),
    reset: () => {
      onFirstOutput = null;
    },
  };
});

vi.mock("../panels/terminal/useXterm", () => ({
  useXterm: vi.fn((opts: { onFirstOutput?: () => void }) => {
    mocks.capture(opts.onFirstOutput);
    return { focus: mocks.focus };
  }),
}));

vi.mock("../ipc", () => ({
  pty: mocks.pty,
}));

import React from "react";
import { render, screen, act, cleanup } from "@testing-library/react";
import TerminalPanel from "../panels/terminal/TerminalPanel";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const fakeApi: any = {
  title: "terminal-0",
  setTitle: vi.fn(),
  updateParameters: vi.fn(),
  onDidTitleChange: vi.fn(() => ({ dispose: vi.fn() })),
  onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
  close: vi.fn(),
};

function renderPanel(panelId: string): void {
  render(React.createElement(TerminalPanel, { api: fakeApi, params: { panelId } }));
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.clearAllMocks();
  mocks.reset();
});

describe("TerminalPanel 加载遮罩（首帧驱动 + 超时兜底）", () => {
  it("挂载后无首帧：1499ms 时遮罩仍在（防误提前隐藏）", () => {
    vi.useFakeTimers();
    renderPanel("mask-p1");
    expect(screen.getByText("正在连接...")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(screen.getByText("正在连接...")).toBeTruthy();
  });

  it("首帧输出到达（onFirstOutput）→ 遮罩立即隐藏，不等满 1500ms（防复发锚）", () => {
    vi.useFakeTimers();
    renderPanel("mask-p2");
    expect(screen.getByText("正在连接...")).toBeTruthy();
    // 模拟首帧在 ~300ms 到达（远早于 1500ms 兜底）
    act(() => {
      vi.advanceTimersByTime(300);
      mocks.fireFirstOutput();
    });
    expect(screen.queryByText("正在连接...")).toBeNull();
  });

  it("首帧永不到达 → 1500ms 到点兜底隐藏（防永远盖死）", () => {
    vi.useFakeTimers();
    renderPanel("mask-p3");
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(screen.queryByText("正在连接...")).toBeNull();
  });

  it("onFirstOutput 二次触发幂等（遮罩保持隐藏，无异常）", () => {
    vi.useFakeTimers();
    renderPanel("mask-p4");
    act(() => {
      mocks.fireFirstOutput();
    });
    expect(screen.queryByText("正在连接...")).toBeNull();
    // 二次触发（生产由 firstOutputSeenRef 保证一次性，此处锁面板侧幂等）
    act(() => {
      mocks.fireFirstOutput();
      vi.advanceTimersByTime(1500);
    });
    expect(screen.queryByText("正在连接...")).toBeNull();
  });
});
