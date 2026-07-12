// fontSize.test.ts — 字体大小 Zustand Store 自动化测试
//
// 测试模式：Zustand store 纯 JS 测试（参考 sessions.test.ts），无需 DOM/React。
// 通过 getState() 直接操作状态，beforeEach 重置。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── Hoisted mocks ───
const { mockSaveSettings, mockLoadSettings } = vi.hoisted(() => ({
  mockSaveSettings: vi.fn().mockResolvedValue(undefined),
  mockLoadSettings: vi.fn().mockResolvedValue(null),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

import { useFontSize, FONT_SIZE_MIN, FONT_SIZE_MAX, FONT_SIZE_DEFAULT } from "../stores/fontSize";

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

  it("7. loadFromDisk 首次启动（返回 null）→ 保持默认值，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue(null);

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
    expect(state.loaded).toBe(true);
    expect(mockLoadSettings).toHaveBeenCalledOnce();
  });

  it("8. loadFromDisk 读取已保存值", async () => {
    mockLoadSettings.mockResolvedValue({
      terminalFontSize: 18,
      editorFontSize: 12,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(18);
    expect(state.editorFontSize).toBe(12);
    expect(state.loaded).toBe(true);
  });

  it("9. loadFromDisk 已保存值超限 clamp", async () => {
    mockLoadSettings.mockResolvedValue({
      terminalFontSize: 100,
      editorFontSize: 2,
    });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(FONT_SIZE_MAX);
    expect(state.editorFontSize).toBe(FONT_SIZE_MIN);
  });

  it("10. loadFromDisk 仅部分 key 存在 → 缺失 key 保持默认", async () => {
    mockLoadSettings.mockResolvedValue({ terminalFontSize: 20 });

    await useFontSize.getState().loadFromDisk();

    const state = useFontSize.getState();
    expect(state.terminalFontSize).toBe(20);
    expect(state.editorFontSize).toBe(FONT_SIZE_DEFAULT);
  });

  it("11. loadFromDisk 非数字类型 → 保持默认", async () => {
    mockLoadSettings.mockResolvedValue({
      terminalFontSize: "not-a-number",
      editorFontSize: null,
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

  it("16. saveSettings 失败不影响 store（静默吞错）", () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    useFontSize.setState({ loaded: true });

    // 不应抛异常
    expect(() => {
      useFontSize.getState().setTerminalFontSize(16);
      vi.advanceTimersByTime(2000);
    }).not.toThrow();

    // store 状态仍正常
    expect(useFontSize.getState().terminalFontSize).toBe(16);
  });
});
