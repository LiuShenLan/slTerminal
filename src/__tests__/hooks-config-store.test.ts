// hooks-config-store.test.ts — hooks 配置禁用状态 Zustand Store 自动化测试（P3-TE-07）
//
// 测试模式同 keybindings.test.ts：mock ../ipc/settings，getState() 直接操作，fake timers 测 debounce。

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

import { useHooksConfig, cancelPendingSave } from "../stores/hooksConfig";
import type { DisabledHookKey } from "../types/hooksConfig";

// 测试用四元组 key
const KEY_A: DisabledHookKey = { layer: "user", event: "PreToolUse", matcher: null, command: "node a.js" };
const KEY_B: DisabledHookKey = { layer: "project", event: "Stop", matcher: "*.ts", command: "node b.js" };

describe("hooksConfig store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useHooksConfig.setState({ disabledHooks: [], loaded: false });
    vi.clearAllMocks();
    mockSaveSettings.mockResolvedValue(undefined);
    mockLoadSettings.mockResolvedValue(null);
  });

  afterEach(() => {
    cancelPendingSave(); // 清理模块级 timer，防跨测试泄漏
    vi.useRealTimers();
  });

  // ── 默认值 ──

  it("1. 默认 disabledHooks 为空、loaded=false", () => {
    const s = useHooksConfig.getState();
    expect(s.disabledHooks).toEqual([]);
    expect(s.loaded).toBe(false);
  });

  // ── disableHook / enableHook / isDisabled ──

  it("2. disableHook 追加记录", () => {
    useHooksConfig.getState().disableHook(KEY_A);
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A]);
  });

  it("3. disableHook 重复添加同一 key 不重复", () => {
    useHooksConfig.getState().disableHook(KEY_A);
    useHooksConfig.getState().disableHook(KEY_A);
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A]);
  });

  it("4. enableHook 移除记录", () => {
    useHooksConfig.setState({ disabledHooks: [KEY_A, KEY_B] });
    useHooksConfig.getState().enableHook(KEY_A);
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_B]);
  });

  it("5. enableHook 不存在的记录无副作用", () => {
    useHooksConfig.setState({ disabledHooks: [KEY_A] });
    useHooksConfig.getState().enableHook(KEY_B);
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A]);
  });

  it("6. isDisabled 匹配判断", () => {
    expect(useHooksConfig.getState().isDisabled(KEY_A)).toBe(false);
    useHooksConfig.getState().disableHook(KEY_A);
    expect(useHooksConfig.getState().isDisabled(KEY_A)).toBe(true);
    expect(useHooksConfig.getState().isDisabled(KEY_B)).toBe(false);
  });

  it("7. isDisabled 四字段全等匹配（matcher 不同不匹配）", () => {
    useHooksConfig.setState({
      disabledHooks: [{ layer: "user", event: "Stop", matcher: "*.ts", command: "node a.js" }],
    });
    expect(
      useHooksConfig.getState().isDisabled({ layer: "user", event: "Stop", matcher: "*.js", command: "node a.js" }),
    ).toBe(false);
    expect(
      useHooksConfig.getState().isDisabled({ layer: "user", event: "Stop", matcher: "*.ts", command: "node a.js" }),
    ).toBe(true);
  });

  // ── loadFromDisk ──

  it("8. loadFromDisk 首次启动（null）→ 空列表，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue(null);
    await useHooksConfig.getState().loadFromDisk();
    const s = useHooksConfig.getState();
    expect(s.disabledHooks).toEqual([]);
    expect(s.loaded).toBe(true);
  });

  it("9. loadFromDisk 读取合法 disabledHooks 段", async () => {
    mockLoadSettings.mockResolvedValue({ disabledHooks: [KEY_A, KEY_B] });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A, KEY_B]);
  });

  it("10. loadFromDisk sanitize 丢弃非四元组脏元素", async () => {
    mockLoadSettings.mockResolvedValue({
      disabledHooks: [
        KEY_A, // 合法 → 保留
        { layer: "global", event: "Stop", matcher: null, command: "node x.js" }, // layer 非法 → 丢弃
        { layer: "user", event: 123, matcher: null, command: "node x.js" }, // event 非字符串 → 丢弃
        { layer: "user", event: "Stop", matcher: 42, command: "node x.js" }, // matcher 非 string/null → 丢弃
        { layer: "user", event: "Stop", matcher: null, command: 123 }, // command 非字符串 → 丢弃
        "not-an-object", // 非对象 → 丢弃
        null, // null → 丢弃
        ["x"], // 数组 → 丢弃
      ],
    });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A]);
  });

  it("11. loadFromDisk matcher 键缺失（undefined）→ 归 null 保留（全匹配语义）", async () => {
    mockLoadSettings.mockResolvedValue({
      disabledHooks: [{ layer: "user", event: "Stop", command: "node a.js" }],
    });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([
      { layer: "user", event: "Stop", matcher: null, command: "node a.js" },
    ]);
  });

  it("12. loadFromDisk disabledHooks 非数组（对象/字符串）→ 空列表", async () => {
    mockLoadSettings.mockResolvedValue({ disabledHooks: { layer: "user" } });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([]);

    useHooksConfig.setState({ loaded: false, disabledHooks: [KEY_A] });
    mockLoadSettings.mockResolvedValue({ disabledHooks: "garbage" });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([]);
  });

  it("13. loadFromDisk disabledHooks 键缺失 → 空列表", async () => {
    mockLoadSettings.mockResolvedValue({ terminalFontSize: 14 });
    await useHooksConfig.getState().loadFromDisk();
    expect(useHooksConfig.getState().disabledHooks).toEqual([]);
    expect(useHooksConfig.getState().loaded).toBe(true);
  });

  it("14. loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));
    await useHooksConfig.getState().loadFromDisk();
    const s = useHooksConfig.getState();
    expect(s.disabledHooks).toEqual([]);
    expect(s.loaded).toBe(true);
  });

  // ── 持久化 ──

  it("15. loaded=false 时不触发 saveSettings", () => {
    useHooksConfig.getState().disableHook(KEY_A);
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("16. loaded=true 变更 → 2s debounce → saveSettings({disabledHooks}) payload 键集合精确匹配", () => {
    useHooksConfig.setState({ loaded: true });
    useHooksConfig.getState().disableHook(KEY_A);

    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [KEY_A] });
  });

  it("17. 多次变更 → debounce 只写最后一次", () => {
    useHooksConfig.setState({ loaded: true });
    useHooksConfig.getState().disableHook(KEY_A);
    vi.advanceTimersByTime(500);
    useHooksConfig.getState().disableHook(KEY_B);
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [KEY_A, KEY_B] });
  });

  it("18. enableHook 变更同样触发 debounce 保存", () => {
    useHooksConfig.setState({ loaded: true, disabledHooks: [KEY_A, KEY_B] });
    useHooksConfig.getState().enableHook(KEY_A);
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [KEY_B] });
  });

  it("19. saveSettings 失败不影响 store（静默吞错）", () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    useHooksConfig.setState({ loaded: true });
    expect(() => {
      useHooksConfig.getState().disableHook(KEY_A);
      vi.advanceTimersByTime(2000);
    }).not.toThrow();
    expect(useHooksConfig.getState().disabledHooks).toEqual([KEY_A]);
  });

  it("20. cancelPendingSave 取消待执行保存，后续变更重新计时", () => {
    useHooksConfig.setState({ loaded: true });
    useHooksConfig.getState().disableHook(KEY_A);
    cancelPendingSave();
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    useHooksConfig.getState().disableHook(KEY_B);
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [KEY_A, KEY_B] });
  });

  it("21. saveToDisk 立即写盘", async () => {
    useHooksConfig.setState({ loaded: true, disabledHooks: [KEY_A] });
    await useHooksConfig.getState().saveToDisk();
    expect(mockSaveSettings).toHaveBeenCalledWith({ disabledHooks: [KEY_A] });
  });
});
