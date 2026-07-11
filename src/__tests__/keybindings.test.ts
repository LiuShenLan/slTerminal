// keybindings.test.ts — 快捷键自定义绑定 Zustand Store 自动化测试
//
// 测试模式同 fontSize.test.ts：mock ../ipc/settings，getState() 直接操作，fake timers 测 debounce。

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

import { useKeybindings } from "../stores/keybindings";

describe("keybindings store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useKeybindings.setState({ overrides: {}, loaded: false });
    vi.clearAllMocks();
    mockSaveSettings.mockResolvedValue(undefined);
    mockLoadSettings.mockResolvedValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ── 默认值 ──

  it("1. 默认 overrides 为空、loaded=false", () => {
    const s = useKeybindings.getState();
    expect(s.overrides).toEqual({});
    expect(s.loaded).toBe(false);
  });

  // ── setBinding / clearBinding / resetAll ──

  it("2. setBinding 新增绑定", () => {
    useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Alt+KeyC");
    expect(useKeybindings.getState().overrides).toEqual({ "terminal.copy": "Ctrl+Alt+KeyC" });
  });

  it("3. setBinding null 表示解绑", () => {
    useKeybindings.getState().setBinding("global.closeTab", null);
    expect(useKeybindings.getState().overrides).toEqual({ "global.closeTab": null });
  });

  it("4. setBinding 覆盖同 id 旧值", () => {
    useKeybindings.getState().setBinding("editor.save", "Ctrl+KeyK");
    useKeybindings.getState().setBinding("editor.save", "Ctrl+Alt+KeyS");
    expect(useKeybindings.getState().overrides).toEqual({ "editor.save": "Ctrl+Alt+KeyS" });
  });

  it("5. clearBinding 移除某覆盖 → 回退默认", () => {
    useKeybindings.setState({ overrides: { "editor.save": "Ctrl+KeyK", "terminal.copy": "Ctrl+Alt+KeyC" } });
    useKeybindings.getState().clearBinding("editor.save");
    expect(useKeybindings.getState().overrides).toEqual({ "terminal.copy": "Ctrl+Alt+KeyC" });
  });

  it("6. resetAll 清空全部覆盖", () => {
    useKeybindings.setState({ overrides: { a: "Ctrl+KeyA", b: null } });
    useKeybindings.getState().resetAll();
    expect(useKeybindings.getState().overrides).toEqual({});
  });

  // ── loadFromDisk ──

  it("7. loadFromDisk 首次启动（null）→ 空覆盖，loaded=true", async () => {
    mockLoadSettings.mockResolvedValue(null);
    await useKeybindings.getState().loadFromDisk();
    const s = useKeybindings.getState();
    expect(s.overrides).toEqual({});
    expect(s.loaded).toBe(true);
  });

  it("8. loadFromDisk 读取合法 keybindings 段", async () => {
    mockLoadSettings.mockResolvedValue({
      keybindings: { "terminal.copy": "Ctrl+Alt+KeyC", "global.closeTab": null },
    });
    await useKeybindings.getState().loadFromDisk();
    expect(useKeybindings.getState().overrides).toEqual({
      "terminal.copy": "Ctrl+Alt+KeyC",
      "global.closeTab": null,
    });
  });

  it("9. loadFromDisk sanitize 丢弃非 string|null 脏值", async () => {
    mockLoadSettings.mockResolvedValue({
      keybindings: {
        "a": "Ctrl+KeyA", // 保留
        "b": null,          // 保留
        "c": 123,           // 丢弃
        "d": { nested: 1 }, // 丢弃
        "e": ["x"],         // 丢弃
      },
    });
    await useKeybindings.getState().loadFromDisk();
    expect(useKeybindings.getState().overrides).toEqual({ "a": "Ctrl+KeyA", "b": null });
  });

  it("10. loadFromDisk keybindings 键缺失 → 空覆盖", async () => {
    mockLoadSettings.mockResolvedValue({ terminalFontSize: 14 });
    await useKeybindings.getState().loadFromDisk();
    expect(useKeybindings.getState().overrides).toEqual({});
    expect(useKeybindings.getState().loaded).toBe(true);
  });

  it("11. loadFromDisk keybindings 非对象（数组/字符串）→ 空覆盖", async () => {
    mockLoadSettings.mockResolvedValue({ keybindings: ["not", "an", "object"] });
    await useKeybindings.getState().loadFromDisk();
    expect(useKeybindings.getState().overrides).toEqual({});
  });

  it("12. loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));
    await useKeybindings.getState().loadFromDisk();
    const s = useKeybindings.getState();
    expect(s.overrides).toEqual({});
    expect(s.loaded).toBe(true);
  });

  // ── 持久化 ──

  it("13. loaded=false 时不触发 saveSettings", () => {
    useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Alt+KeyC");
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("14. loaded=true 变更 → 2s debounce → saveSettings({keybindings})", () => {
    useKeybindings.setState({ loaded: true });
    useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Alt+KeyC");

    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      keybindings: { "terminal.copy": "Ctrl+Alt+KeyC" },
    });
  });

  it("15. 多次变更 → debounce 只写最后一次", () => {
    useKeybindings.setState({ loaded: true });
    useKeybindings.getState().setBinding("a", "Ctrl+KeyA");
    vi.advanceTimersByTime(500);
    useKeybindings.getState().setBinding("b", "Ctrl+KeyB");
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      keybindings: { "a": "Ctrl+KeyA", "b": "Ctrl+KeyB" },
    });
  });

  it("16. saveSettings 失败不影响 store（静默吞错）", () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    useKeybindings.setState({ loaded: true });
    expect(() => {
      useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Alt+KeyC");
      vi.advanceTimersByTime(2000);
    }).not.toThrow();
    expect(useKeybindings.getState().overrides).toEqual({ "terminal.copy": "Ctrl+Alt+KeyC" });
  });
});
