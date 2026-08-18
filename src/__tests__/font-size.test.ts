// fontSize.test.ts — 字体大小 Zustand Store 自动化测试
//
// 测试模式：Zustand store 纯 JS 测试（参考 sessions.test.ts），无需 DOM/React。
// 通过 getState() 直接操作状态，beforeEach 重置。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── Hoisted mocks ───
const { mockSaveSettings, mockLoadSettings, mockToastShow, mockGetErrorMessage } = vi.hoisted(() => ({
  mockSaveSettings: vi.fn().mockResolvedValue(undefined),
  // FE-11/D11：wrapper 返回 { data, corrupted }——无文件 = data:null, corrupted:false
  mockLoadSettings: vi.fn().mockResolvedValue({ data: null, corrupted: false }),
  mockToastShow: vi.fn(),
  // FE-09：错误消息统一经 getErrorMessage（契约），默认兜底 String(err)
  mockGetErrorMessage: vi.fn((err: unknown) => String(err)),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

// FE-09：store 保存失败经 src/lib 的 toast/getErrorMessage——mock 隔离断言
vi.mock("../lib", () => ({
  toast: { show: mockToastShow, _reset: vi.fn() },
  getErrorMessage: mockGetErrorMessage,
}));

import {
  useFontSize,
  cancelPendingSave,
  FONT_SIZE_MIN,
  FONT_SIZE_MAX,
  FONT_SIZE_DEFAULT,
} from "../stores/fontSize";

describe("fontSize store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // 重置 state（loaded: false 防止 debounce 触发）
    useFontSize.setState({
      terminalFontSize: FONT_SIZE_DEFAULT,
      editorFontSize: FONT_SIZE_DEFAULT,
      loaded: false,
    });
    vi.clearAllMocks();
  });

  afterEach(() => {
    // STS-06：先取消活跃 debounce timer（fake timers 下仍有效），再切回真实 timer
    cancelPendingSave();
    vi.useRealTimers();
  });

  // ── 默认值 ──

  it("1. 默认值：terminalFontSize=14, editorFontSize=14", () => {
    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(14);
    expect(state.editorFontSize).toBe(14);
  });

  // ── setTerminalFontSize ──

  it("2. setTerminalFontSize 正常设置", () => {
    useFontSize.getState().setTerminalFontSize(16);
    expect(useFontSize.getState().terminalFontSize).toBe(16);
  });

  it("3. setTerminalFontSize 下限 clamp：设 5 → 保持 8", () => {
    useFontSize.getState().setTerminalFontSize(5);
    expect(useFontSize.getState().terminalFontSize).toBe(FONT_SIZE_MIN);
  });

  it("4. setTerminalFontSize 上限 clamp：设 40 → 保持 32", () => {
    useFontSize.getState().setTerminalFontSize(40);
    expect(useFontSize.getState().terminalFontSize).toBe(FONT_SIZE_MAX);
  });

  it("5. setTerminalFontSize 小数取整：12.7 → 13", () => {
    useFontSize.getState().setTerminalFontSize(12.7);
    expect(useFontSize.getState().terminalFontSize).toBe(13);
  });

  // ── setEditorFontSize ──

  it("6. setEditorFontSize 正常设置 + clamp", () => {
    useFontSize.getState().setEditorFontSize(20);
    expect(useFontSize.getState().editorFontSize).toBe(20);

    useFontSize.getState().setEditorFontSize(100);
    expect(useFontSize.getState().editorFontSize).toBe(FONT_SIZE_MAX);

    useFontSize.getState().setEditorFontSize(3);
    expect(useFontSize.getState().editorFontSize).toBe(FONT_SIZE_MIN);
  });

  // ── loadFromDisk ──

  it("7. loadFromDisk 首次启动（data:null）→ 保持默认值，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: false });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.loaded).toBe(true);
    expect(mockLoadSettings).toHaveBeenCalledOnce();
  });

  it("8. loadFromDisk 读取已保存值", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { terminalFontSize: 18, editorFontSize: 12 },
      corrupted: false,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(18);
    expect(state.editorFontSize).toBe(12);
    expect(state.loaded).toBe(true);
  });

  it("9. loadFromDisk 已保存值超限 clamp", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { terminalFontSize: 100, editorFontSize: 2 },
      corrupted: false,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_MAX);
    expect(state.editorFontSize).toBe(FONT_SIZE_MIN);
  });

  it("10. loadFromDisk 仅部分 key 存在 → 缺失 key 保持默认", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { terminalFontSize: 20 },
      corrupted: false,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(20);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
  });

  it("11. loadFromDisk 非数字类型 → 保持默认", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        terminalFontSize: "not-a-number",
        editorFontSize: null,
      },
      corrupted: false,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
  });

  it("12. loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.loaded).toBe(true);
  });

  it("12b. loadFromDisk corrupted=true → 默认值 + toast 告警（FE-11）", async () => {
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: true });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.loaded).toBe(true);
    // FE-11：配置损坏统一 toast 告警（warning + 固定文案）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  it("12c. loadFromDisk corrupted=true 且带数据 → 消费数据 + toast 告警（FE-11）", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { terminalFontSize: 18 },
      corrupted: true,
    });

    await useFontSize.getState().loadFromDisk();

    // 损坏时后端返回默认/可用数据，仍正常消费
    expect(useFontSize.getState().terminalFontSize).toBe(18);
    expect(useFontSize.getState().editorFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  // ── 持久化 ──

  it("13. loaded=false 时不触发 saveSettings", () => {
    // loaded 当前为 false（beforeEach 重置）
    useFontSize.getState().setTerminalFontSize(16);

    // 推进 2s
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("14. loaded=true 时变更 → 2s debounce → saveSettings", () => {
    useFontSize.setState({ loaded: true });

    useFontSize.getState().setTerminalFontSize(16);

    // 不到 2s 不触发
    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    // 到达 2s 触发
    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      terminalFontSize: 16,
      editorFontSize: FONT_SIZE_DEFAULT,
    });
  });

  it("15. 多次变更 → debounce 只写入最后一次", () => {
    useFontSize.setState({ loaded: true });

    useFontSize.getState().setTerminalFontSize(16);
    vi.advanceTimersByTime(500);
    useFontSize.getState().setTerminalFontSize(18);
    vi.advanceTimersByTime(500);
    useFontSize.getState().setTerminalFontSize(20);

    // 还没到 2s
    expect(mockSaveSettings).not.toHaveBeenCalled();

    // 推进到 2s
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      terminalFontSize: 20,
      editorFontSize: FONT_SIZE_DEFAULT,
    });
  });

  it("16. saveSettings 失败 → toast 告警 + console.warn（FE-09），store 状态不受影响", async () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    mockGetErrorMessage.mockReturnValue("write error");
    useFontSize.setState({ loaded: true });

    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    // 不应抛异常
    expect(() => {
      useFontSize.getState().setTerminalFontSize(16);
    }).not.toThrow();
    // 异步推进 timer 并排空微任务：saveSettings reject 的 .catch 在微任务队列执行
    await vi.advanceTimersByTimeAsync(2000);

    // FE-09：保存失败统一 toast 告警（warning + 固定文案）
    expect(mockToastShow).toHaveBeenCalledWith("warning", "设置保存失败，重启后将丢失");
    // 错误详情统一经 getErrorMessage 提取后 console.warn 记录
    expect(mockGetErrorMessage).toHaveBeenCalledWith(expect.any(Error));
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[stores/fontSize]"),
      "write error",
    );

    // store 状态仍正常
    expect(useFontSize.getState().terminalFontSize).toBe(16);

    consoleWarnSpy.mockRestore();
  });

  it("17. cancelPendingSave 取消活跃 timer——推进 2s 不再写盘", () => {
    useFontSize.setState({ loaded: true });
    useFontSize.getState().setTerminalFontSize(16); // 产生 debounce timer
    vi.advanceTimersByTime(1500);                   // 未到 2s，timer 仍活跃
    cancelPendingSave();                            // 关窗冲刷：取消待执行保存
    vi.advanceTimersByTime(2000);                   // 越过原定触发点
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });
});
