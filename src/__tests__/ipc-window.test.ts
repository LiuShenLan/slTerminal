// ipc-window.test.ts — src/ipc/window.ts 自绘标题栏 wrapper 契约测试（TQ-COV-10 / TB-07）
//
// 覆盖 ipc-window-contract.test.ts 未覆盖的 minimizeWindow / toggleMaximizeWindow /
// closeWindow（TB-03 标题栏三钮）+ isWindowMaximized / onWindowResized（TB-07 maximized
// 状态感知）：
//   - 成功路径：调用 getCurrentWindow() 对应方法且无参
//   - reject 传播：Window API 失败 → wrapper 原样 reject（不吞异常）
//   - onWindowResized：事件回调透传 + 清理函数 unlisten
// 三 wrapper 供自绘标题栏三钮调用——按钮层吞错后 UI 无感知（无 toast 通道），
// 契约层锁死错误必须向上传播（调用方 console.error 可观测）。
// mock 策略照 ipc-window-contract.test.ts：文件级 vi.mock 覆盖 setup.ts 全局 mock。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── Hoisted：五 wrapper 的 Window API mock ───
const { mockMinimize, mockToggleMaximize, mockClose, mockIsMaximized, mockOnResized } =
  vi.hoisted(() => ({
    mockMinimize: vi.fn().mockResolvedValue(undefined),
    mockToggleMaximize: vi.fn().mockResolvedValue(undefined),
    mockClose: vi.fn().mockResolvedValue(undefined),
    mockIsMaximized: vi.fn().mockResolvedValue(false),
    // TB-07：捕获注册的 resize 回调供触发；unlisten resolve 清理函数
    mockOnResized: vi.fn().mockResolvedValue(() => {}),
  }));

// ─── Module mock：@tauri-apps/api/window（覆盖 setup.ts 全局 mock）───
vi.mock("@tauri-apps/api/window", () => ({
  UserAttentionType: { Critical: 1, Informational: 2 } as const,
  getCurrentWindow: vi.fn(() => ({
    minimize: mockMinimize,
    toggleMaximize: mockToggleMaximize,
    close: mockClose,
    isMaximized: mockIsMaximized,
    onResized: mockOnResized,
  })),
}));

// ─── 导入被测模块（真实实现）───
import {
  minimizeWindow,
  toggleMaximizeWindow,
  closeWindow,
  isWindowMaximized,
  onWindowResized,
} from "../ipc/window";

beforeEach(() => {
  mockMinimize.mockClear();
  mockToggleMaximize.mockClear();
  mockClose.mockClear();
  mockIsMaximized.mockClear();
  mockOnResized.mockClear();
});

describe("minimizeWindow（TB-03 最小化钮）", () => {
  it("成功路径：调用 getCurrentWindow().minimize() 且无参", async () => {
    await minimizeWindow();

    expect(mockMinimize).toHaveBeenCalledTimes(1);
    expect(mockMinimize).toHaveBeenCalledWith();
  });

  it("reject 传播：minimize 失败 → wrapper 原样 reject（不吞异常）", async () => {
    mockMinimize.mockRejectedValueOnce(new Error("minimize fail"));

    await expect(minimizeWindow()).rejects.toThrow("minimize fail");
    expect(mockMinimize).toHaveBeenCalledTimes(1);
  });
});

describe("toggleMaximizeWindow（TB-03/TB-04 最大化钮与中段双击）", () => {
  it("成功路径：调用 getCurrentWindow().toggleMaximize() 且无参", async () => {
    await toggleMaximizeWindow();

    expect(mockToggleMaximize).toHaveBeenCalledTimes(1);
    expect(mockToggleMaximize).toHaveBeenCalledWith();
  });

  it("reject 传播：toggleMaximize 失败 → wrapper 原样 reject（不吞异常）", async () => {
    mockToggleMaximize.mockRejectedValueOnce(new Error("toggle fail"));

    await expect(toggleMaximizeWindow()).rejects.toThrow("toggle fail");
    expect(mockToggleMaximize).toHaveBeenCalledTimes(1);
  });
});

describe("closeWindow（TB-03 关闭钮——P1-19 关窗链路入口）", () => {
  it("成功路径：调用 getCurrentWindow().close() 且无参（触发 onCloseRequested 杀 PTY 链路）", async () => {
    await closeWindow();

    expect(mockClose).toHaveBeenCalledTimes(1);
    expect(mockClose).toHaveBeenCalledWith();
  });

  it("reject 传播：close 失败 → wrapper 原样 reject（不吞异常）", async () => {
    mockClose.mockRejectedValueOnce(new Error("close fail"));

    await expect(closeWindow()).rejects.toThrow("close fail");
    expect(mockClose).toHaveBeenCalledTimes(1);
  });
});

describe("isWindowMaximized（TB-07 maximized 状态回查）", () => {
  it("成功路径：透传 getCurrentWindow().isMaximized() 的 boolean", async () => {
    mockIsMaximized.mockResolvedValueOnce(true);

    await expect(isWindowMaximized()).resolves.toBe(true);
    expect(mockIsMaximized).toHaveBeenCalledTimes(1);
    expect(mockIsMaximized).toHaveBeenCalledWith();
  });

  it("reject 传播：isMaximized 失败 → wrapper 原样 reject（不吞异常）", async () => {
    mockIsMaximized.mockRejectedValueOnce(new Error("isMaximized fail"));

    await expect(isWindowMaximized()).rejects.toThrow("isMaximized fail");
  });
});

describe("onWindowResized（TB-07 maximized 状态事件源）", () => {
  it("回调注册：订阅 onResized，事件触发时调用传入回调", async () => {
    const cb = vi.fn();
    onWindowResized(cb);

    expect(mockOnResized).toHaveBeenCalledTimes(1);
    // 取出注册进 onResized 的内部 handler 并模拟事件触发
    const handler = mockOnResized.mock.calls[0][0] as () => void;
    handler();
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("清理函数：调用返回的取消函数 → unlisten 生效", async () => {
    const unlistenFn = vi.fn();
    mockOnResized.mockResolvedValueOnce(unlistenFn);

    const cleanup = onWindowResized(() => {});
    cleanup();

    // unlisten.then(fn => fn())——等微任务冲刷后断言清理函数被调用
    await Promise.resolve();
    await Promise.resolve();
    expect(unlistenFn).toHaveBeenCalledTimes(1);
  });
});
