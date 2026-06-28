// bootstrap.test.ts — main.tsx 启动引导测试
//
// P2-51: 验证 main.tsx bootstrap 函数的 IPC 就绪轮询 + ReactDOM 挂载逻辑。
// 使用 mock ReactDOM.createRoot + 假定时器控制 setInterval 轮询。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── Hoisted mocks（import 前先 mock） ───
const {
  mockRender,
  mockCreateRoot,
  mockSetInterval,
  mockClearInterval,
  capturedSetIntervalCallbacks,
} = vi.hoisted(() => {
  const mockRender = vi.fn();
  const mockCreateRoot = vi.fn(() => ({ render: mockRender }));
  // 捕获 setInterval 回调列表（供手动轮询触发用）
  const capturedSetIntervalCallbacks: Array<() => void> = [];

  return {
    mockRender,
    mockCreateRoot,
    mockSetInterval: vi.fn((cb: () => void) => {
      capturedSetIntervalCallbacks.push(cb);
      return capturedSetIntervalCallbacks.length; // 返回索引作为 intervalId
    }),
    mockClearInterval: vi.fn(),
    capturedSetIntervalCallbacks,
  };
});

// mock ReactDOM（main.tsx 用 default import: import ReactDOM from "react-dom/client"）
vi.mock("react-dom/client", () => ({
  default: { createRoot: mockCreateRoot },
}));

// mock App 组件（避免导入整棵依赖树）
vi.mock("./App", () => ({ default: () => null }));

// mock App.css（空模块）
vi.mock("./App.css", () => ({}));

// ─── 全局 mock ───

// 存储原始值供 afterEach 恢复
const origSetInterval = globalThis.setInterval;
const origClearInterval = globalThis.clearInterval;
describe("main.tsx bootstrap", () => {
  let rootDiv: HTMLDivElement;

  beforeEach(async () => {
    // 重置 mock
    vi.clearAllMocks();
    mockRender.mockClear();
    mockCreateRoot.mockClear();
    capturedSetIntervalCallbacks.length = 0;

    // 创建 div#root
    rootDiv = document.createElement("div");
    rootDiv.id = "root";
    document.body.appendChild(rootDiv);

    // 重置 getElementById mock
    vi.spyOn(document, "getElementById").mockImplementation((id: string) => {
      if (id === "root") return rootDiv;
      return null;
    });

    // 清除 __TAURI_INTERNALS__（默认不存在）
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;

    // 替换 setInterval / clearInterval 为 mock
    globalThis.setInterval = mockSetInterval as unknown as typeof setInterval;
    globalThis.clearInterval = mockClearInterval as unknown as typeof clearInterval;
  });

  afterEach(() => {
    // 恢复全局函数
    globalThis.setInterval = origSetInterval;
    globalThis.clearInterval = origClearInterval;
    vi.restoreAllMocks();
    document.body.removeChild(rootDiv);
  });

  it("1. __TAURI_INTERNALS__ 已存在 → 立即挂载 React，不轮询", async () => {
    // 场景：IPC 在模块加载前已初始化
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};

    // 动态导入 main.tsx（触发 bootstrap()）
    vi.resetModules();
    // 确保 vi.mock 已应用
    await import("../main");

    // 验证：未启动任何轮询（setInterval 未调用）
    expect(mockSetInterval).not.toHaveBeenCalled();

    // 验证：ReactDOM.createRoot 被调用
    expect(mockCreateRoot).toHaveBeenCalledWith(rootDiv);
    expect(mockCreateRoot).toHaveBeenCalledTimes(1);

    // 验证：render 被调用（React.StrictMode 包裹 App）
    expect(mockRender).toHaveBeenCalledTimes(1);
  });

  it("2. __TAURI_INTERNALS__ 不存在 → 启动 50ms 轮询，就绪后挂载 React", async () => {
    // 场景：IPC 尚未初始化（正常启动路径）
    vi.resetModules();
    const importPromise = import("../main");

    // 等待 microtask：bootstrap 进入 await Promise（setInterval 已启动）
    await importPromise;
    // 再等一个 tick 确保 Promise 回调中的 setInterval 被调用
    await new Promise((r) => setTimeout(r, 10));

    // 验证：已启动轮询
    expect(mockSetInterval).toHaveBeenCalledWith(expect.any(Function), 50);

    // 验证：ReactDOM.createRoot 尚未调用（还在轮询等待）
    // 注：此时 await 抛出，main 模块已完成导入，但 bootstrap 中的 await 仍在等待
    // 由于 setInterval 是 mock 的，回调不会自动执行
    expect(mockCreateRoot).not.toHaveBeenCalled();

    // 设置 __TAURI_INTERNALS__（模拟 IPC 就绪）
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};

    // 手动触发一次 setInterval 回调（模拟轮询检测到 IPC 就绪）
    expect(capturedSetIntervalCallbacks.length).toBe(1);
    capturedSetIntervalCallbacks[0]();

    // 等待 Promise resolve 后 ReactDOM 挂载
    await new Promise((r) => setTimeout(r, 10));

    // 验证：ReactDOM.createRoot 被调用
    expect(mockCreateRoot).toHaveBeenCalledWith(rootDiv);
    expect(mockCreateRoot).toHaveBeenCalledTimes(1);
    expect(mockRender).toHaveBeenCalledTimes(1);

    // 验证：setInterval 被清理（clearInterval called）
    expect(mockClearInterval).toHaveBeenCalled();
  });

  it("3. __TAURI_INTERNALS__ 不存在且永远不就绪 → 永不调用 createRoot", async () => {
    // 场景：IPC 永远不就绪（理论路径，实际不应发生）
    vi.resetModules();
    const importPromise = import("../main");

    await importPromise;
    await new Promise((r) => setTimeout(r, 10));

    // 轮询已启动
    expect(mockSetInterval).toHaveBeenCalledWith(expect.any(Function), 50);

    // 多次触发轮询回调，但 __TAURI_INTERNALS__ 始终不存在
    for (let i = 0; i < 10; i++) {
      capturedSetIntervalCallbacks[0]();
    }

    await new Promise((r) => setTimeout(r, 10));

    // 验证：ReactDOM 从未被调用
    expect(mockCreateRoot).not.toHaveBeenCalled();
    expect(mockRender).not.toHaveBeenCalled();
  });
});
