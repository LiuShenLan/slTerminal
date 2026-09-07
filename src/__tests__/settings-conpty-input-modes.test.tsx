// settings-conpty-input-modes.test.tsx — 设置中心「终端输入模式」页 + store L2 测试（CP-009）
//
// 覆盖：四开关渲染（默认矩阵 = inheritCursor/resizeQuirk/win32InputMode 开、
// passthroughMode 关——零默认漂移前端侧）+ passthrough 常驻警示文案断言 /
// 开关切换 → store 状态立即变更（防复发：页面行数据常量 MODE_ROWS 的字段名与后端
// flags 位对应，本测试经 checked 态锁死字段名拼写）/ debounce 保存 payload 顶层键
// = conptyInputModes 段（SEC-11 白名单第 7 键，防平铺回归）/ loadFromDisk 段部分
// 缺失 → 逐字段回退默认 / loaded 守卫：加载完成前变更不触发保存。
//
// mock 策略：文件级 vi.mock 接管 ../ipc/settings（loadSettings/saveSettings hoisted
// 可控）；../lib 保留真实实现仅替换 toast（store 内 FE-09/FE-11 告警断言）。
// store 为真实模块级单例——beforeEach setState 重置 + afterEach 清 timer（隔离纪律）。
// data-e2e 查询用 document.querySelector（无 testIdAttribute 配置，照 cli-aliases 先例）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import ConptyInputModesPage from "../panels/settings/pages/ConptyInputModesPage";
import {
  useConptyInputModes,
  cancelPendingSave,
  DEFAULT_CONPTY_INPUT_MODES,
} from "../stores/conptyInputModes";
import { PERSIST_DEBOUNCE_MS } from "../stores/projects";

const h = vi.hoisted(() => ({
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  toastShow: vi.fn(),
}));

vi.mock("../ipc/settings", () => ({
  loadSettings: h.loadSettings,
  saveSettings: h.saveSettings,
}));

// 保留 lib 真实实现，仅替换 toast（store FE-09/FE-11 告警断言）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: { show: h.toastShow, _reset: vi.fn() } };
});

/** data-e2e 查询辅助（与既有 settings 页测试同款） */
function byE2e(name: string): HTMLElement | null {
  return document.querySelector(`[data-e2e="${name}"]`);
}

/** 重置 store 至加载完成 + 默认矩阵（每用例独立） */
function resetStore(loaded = true) {
  useConptyInputModes.setState({
    modes: { ...DEFAULT_CONPTY_INPUT_MODES },
    loaded,
  });
}

describe("「终端输入模式」配置页渲染", () => {
  beforeEach(() => {
    h.loadSettings.mockReset();
    h.saveSettings.mockReset();
    h.toastShow.mockReset();
    resetStore();
  });

  afterEach(() => {
    cancelPendingSave();
    vi.useRealTimers();
    cleanup();
  });

  it("四开关渲染且默认矩阵状态正确（passthrough 默认关，其余默认开）", () => {
    render(<ConptyInputModesPage />);

    // 页容器
    expect(byE2e("settings-conpty-input-modes-page")).not.toBeNull();

    // 四开关逐一断言（checkbox checked 态锁死字段名 ↔ flags 位对应关系）
    const inherit = byE2e("settings-conpty-inherit-cursor-checked") as HTMLInputElement;
    const resize = byE2e("settings-conpty-resize-quirk-checked") as HTMLInputElement;
    const win32 = byE2e("settings-conpty-win32-input-mode-checked") as HTMLInputElement;
    const passthrough = byE2e("settings-conpty-passthrough-mode-checked") as HTMLInputElement;

    expect(inherit.checked).toBe(true);
    expect(resize.checked).toBe(true);
    expect(win32.checked).toBe(true);
    expect(passthrough.checked).toBe(false);

    // 行标签可读（四开关 label 均带 flag 位后缀）
    const pageText = byE2e("settings-conpty-input-modes-page")?.textContent ?? "";
    expect(pageText).toContain("继承光标位置（0x1）");
    expect(pageText).toContain("Resize 兼容（0x2）");
    expect(pageText).toContain("Win32 输入模式（0x4）");
    expect(pageText).toContain("Passthrough 直通模式（0x8）");
  });

  it("passthrough 行带常驻警示文案（0x8 滚轮失效 + 实测验证口径）", () => {
    render(<ConptyInputModesPage />);

    const warning = byE2e("settings-conpty-passthrough-warning");
    expect(warning).not.toBeNull();
    const text = warning?.textContent ?? "";
    expect(text).toContain("claude");
    expect(text).toContain("鼠标滚轮失效");
    expect(text).toContain("实测验证");
  });

  it("开关切换 → store 状态立即变更（无 dirty 暂存，立即提交型）", () => {
    render(<ConptyInputModesPage />);

    const passthrough = byE2e("settings-conpty-passthrough-mode-checked") as HTMLInputElement;
    fireEvent.click(passthrough);

    expect(useConptyInputModes.getState().modes.passthroughMode).toBe(true);
    // 其余开关不受影响
    const modes = useConptyInputModes.getState().modes;
    expect(modes.inheritCursor).toBe(true);
    expect(modes.resizeQuirk).toBe(true);
    expect(modes.win32InputMode).toBe(true);
  });

  it("切换后 debounce 保存 payload 顶层键 = conptyInputModes 段（SEC-11 白名单第 7 键，防平铺回归）", () => {
    vi.useFakeTimers();
    h.saveSettings.mockResolvedValue(undefined); // 成功路径（store 内 .catch 需要 Promise）
    render(<ConptyInputModesPage />);

    const win32 = byE2e("settings-conpty-win32-input-mode-checked") as HTMLInputElement;
    fireEvent.click(win32); // 关掉 win32InputMode

    expect(h.saveSettings).not.toHaveBeenCalled(); // debounce 未到期
    vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS);

    expect(h.saveSettings).toHaveBeenCalledTimes(1);
    const payload = h.saveSettings.mock.calls[0][0] as Record<string, unknown>;
    // 顶层键必须为段名（平铺字段会走后端白名单拒绝——契约断链先例锁死）
    expect(Object.keys(payload)).toEqual(["conptyInputModes"]);
    const section = payload.conptyInputModes as Record<string, boolean>;
    expect(section).toEqual({
      inheritCursor: true,
      resizeQuirk: true,
      win32InputMode: false,
      passthroughMode: false,
    });
  });
});

describe("conptyInputModes store 段形态", () => {
  beforeEach(() => {
    h.loadSettings.mockReset();
    h.saveSettings.mockReset();
    h.toastShow.mockReset();
    resetStore(false);
    vi.useFakeTimers();
  });

  afterEach(() => {
    cancelPendingSave();
    vi.useRealTimers();
  });

  it("loadFromDisk 段缺失（data:null）→ 默认矩阵 + loaded 置位", async () => {
    h.loadSettings.mockResolvedValue({ data: null, corrupted: false });
    await useConptyInputModes.getState().loadFromDisk();

    const s = useConptyInputModes.getState();
    expect(s.loaded).toBe(true);
    expect(s.modes).toEqual(DEFAULT_CONPTY_INPUT_MODES);
    expect(h.toastShow).not.toHaveBeenCalled();
  });

  it("loadFromDisk 段部分字段缺失 → 逐字段回退默认（后端 serde default 同口径，零漂移）", async () => {
    h.loadSettings.mockResolvedValue({
      data: { conptyInputModes: { passthroughMode: true } },
      corrupted: false,
    });
    await useConptyInputModes.getState().loadFromDisk();

    const modes = useConptyInputModes.getState().modes;
    // 缺失字段回退默认
    expect(modes.inheritCursor).toBe(true);
    expect(modes.resizeQuirk).toBe(true);
    expect(modes.win32InputMode).toBe(true);
    // 显式字段生效（启用责任在人工门禁，页面已有警示）
    expect(modes.passthroughMode).toBe(true);
  });

  it("loaded 守卫：加载完成前 store 变更不触发 debounce 保存（防空写覆盖磁盘）", () => {
    useConptyInputModes.getState().setMode({ resizeQuirk: false });
    vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS * 2);
    expect(h.saveSettings).not.toHaveBeenCalled();
  });

  it("corrupted → FE-11 toast 告警且仍回退默认", async () => {
    h.loadSettings.mockResolvedValue({ data: null, corrupted: true });
    await useConptyInputModes.getState().loadFromDisk();

    expect(h.toastShow).toHaveBeenCalledWith("warning", expect.stringContaining("损坏"));
    expect(useConptyInputModes.getState().modes).toEqual(DEFAULT_CONPTY_INPUT_MODES);
  });

  it("保存失败 → FE-09 toast 告警", async () => {
    h.saveSettings.mockRejectedValue(new Error("boom"));
    resetStore(); // loaded=true
    useConptyInputModes.getState().setMode({ resizeQuirk: false });
    vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS);
    // flush microtask（saveSettings rejection → catch → toast）
    await vi.waitFor(() => {
      expect(h.toastShow).toHaveBeenCalledWith(
        "warning",
        expect.stringContaining("保存失败"),
      );
    });
  });
});
