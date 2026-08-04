// notification.test.ts — notification IPC 封装分支覆盖（IHE-02）
//
// 覆盖 src/ipc/notification.ts 的未测分支：
// - ensureNotificationPermission 三路径（已授权 / 请求授权 granted / 请求授权被拒）
// - ensureNotificationPermission 底层调用 reject 时异常传播（不吞）
// - sendToastNotification 正常发送 + 同步抛错时 catch 静默（console.error 记录，不向上抛）
//
// mock @tauri-apps/plugin-notification（Tauri 原生插件，jsdom 下无真实后端）。

import { describe, it, expect, afterEach, vi } from "vitest";

// vi.hoisted：mock 工厂与测试侧引用必须共享同一 mock 实例——
// 否则 notification.ts（re-export 插件函数）与测试侧各持一份 vi.fn()，
// 断言计数/行为会分裂（用例间调用计数泄漏）
const notificationPluginMock = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-notification", () => notificationPluginMock);

// 测试经 mock 验证插件交互——import 仅用于取 mock 句柄，同 ipc-contract 对
// @tauri-apps/api/event 的先例（no-restricted-imports 例外仅限测试文件）
// eslint-disable-next-line no-restricted-imports
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import * as notification from "../ipc/notification";

afterEach(() => {
  // 清调用计数（实现每用例内重设）；console.error spy 由 restoreAllMocks 兜底
  vi.clearAllMocks();
  vi.restoreAllMocks();
});

// ═══════════════════════════════════════════════════════════════════
// ensureNotificationPermission
// ═══════════════════════════════════════════════════════════════════

describe("ensureNotificationPermission", () => {
  it("已授权时直接返回 true，不发起授权请求", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);

    const result = await notification.ensureNotificationPermission();

    expect(result).toBe(true);
    expect(requestPermission).not.toHaveBeenCalled();
  });

  it("未授权但请求授予 granted 时返回 true", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockResolvedValue("granted");

    const result = await notification.ensureNotificationPermission();

    expect(isPermissionGranted).toHaveBeenCalledTimes(1);
    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(result).toBe(true);
  });

  it("请求授权被拒（denied）时返回 false（拒绝路径）", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockResolvedValue("denied");

    const result = await notification.ensureNotificationPermission();

    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(result).toBe(false);
  });

  it("请求授权返回 default 时也返回 false（仅 granted 视为可发送）", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockResolvedValue("default");

    const result = await notification.ensureNotificationPermission();

    expect(result).toBe(false);
  });

  it("isPermissionGranted reject 时异常传播（不吞错）", async () => {
    vi.mocked(isPermissionGranted).mockRejectedValue(
      new Error("权限查询失败"),
    );

    await expect(notification.ensureNotificationPermission()).rejects.toThrow(
      "权限查询失败",
    );
  });

  it("requestPermission reject 时异常传播（不吞错）", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockRejectedValue(
      new Error("授权请求失败"),
    );

    await expect(notification.ensureNotificationPermission()).rejects.toThrow(
      "授权请求失败",
    );
  });
});

// ═══════════════════════════════════════════════════════════════════
// sendToastNotification
// ═══════════════════════════════════════════════════════════════════

describe("sendToastNotification", () => {
  it("正常路径：调用 sendNotification 并传 { title, body }", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    notification.sendToastNotification("任务完成", { body: "测试正文" });

    expect(sendNotification).toHaveBeenCalledWith({
      title: "任务完成",
      body: "测试正文",
    });
    expect(errorSpy).not.toHaveBeenCalled();
  });

  it("sendNotification 同步抛错时 catch 静默（console.error 记录，不向上抛）", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    // 未打包 Win32 WebView2 探针实测：sendNotification 同步抛
    // TypeError（shim 无 close）——catch 静默即本分支
    vi.mocked(sendNotification).mockImplementation(() => {
      throw new TypeError("n.close is not a function");
    });

    expect(() =>
      notification.sendToastNotification("标题", { body: "正文" }),
    ).not.toThrow();

    expect(errorSpy).toHaveBeenCalledWith(
      "sendToastNotification 失败:",
      expect.any(TypeError),
    );
  });

  it("sendNotification 多次失败均静默（不中断后续调用）", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(sendNotification).mockImplementation(() => {
      throw new Error("banner suppressed");
    });

    notification.sendToastNotification("a", { body: "1" });
    notification.sendToastNotification("b", { body: "2" });

    expect(errorSpy).toHaveBeenCalledTimes(2);
  });
});
