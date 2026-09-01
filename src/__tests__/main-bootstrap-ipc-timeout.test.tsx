// main-bootstrap.test.tsx — main.tsx bootstrap 失败分支（WRK-10）
//
// Tauri IPC（window.__TAURI_INTERNALS__）10s 未就绪 → catch 分支：
// console.error + 页面显示错误信息（暗色错误样式，不白屏、不挂载 App）。

import { describe, it, expect, afterEach, vi } from "vitest";

describe("main.tsx bootstrap", () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });

  it("IPC 10s 未就绪 → console.error + 页面显示错误信息（不白屏）", async () => {
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.useFakeTimers();
    // jsdom 默认无 __TAURI_INTERNALS__，显式删除防残留
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.resetModules();

    // 动态导入 main.tsx 触发 bootstrap()（fake timers 下 interval 不自动推进）
    await import("../main");

    // 推进 200 × 50ms = 10s，触发超时 reject → catch 分支
    await vi.advanceTimersByTimeAsync(200 * 50);

    // 错误已记录（含超时消息）
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "[slTerminal]",
      expect.stringContaining("Tauri IPC 初始化超时"),
    );
    // 页面显示错误信息（暗色错误样式）——非白屏
    expect(document.body.innerHTML).toContain("Tauri IPC 初始化超时");
    // SEC-10：fail-safe 页改 createElement + textContent + style 赋值——
    // 颜色经 CSSOM 写入 style attribute，jsdom 将 hex 规范化为 rgb 形态（html-panel.test.tsx 同先例）
    expect(document.body.innerHTML).toContain("rgb(217, 112, 107)");
    // 未挂载 React（无 App 启动中文案）
    expect(document.body.innerHTML).not.toContain("slTerminal 启动中");
  });
});
