// cli-aliases.test.ts — CLI 别名 Zustand Store 自动化测试
//
// 测试模式同 keybindings.test.ts：mock ../ipc/settings，getState() 直接操作，
// fake timers 测 debounce。额外依赖：loadFromDisk 的 sanitize 需 profile 已注册
// （孤儿 cliId 判定）——beforeEach 注册 light fake profile（claude/codex 各一，
// 不引 heavy claudeProfile 依赖链），afterEach _reset 隔离注册表。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";

/** light fake profile（iconSrc 不被解析，随意路径） */
function fakeProfile(id: string, commands: string[] = [id]): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands,
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {},
  };
}

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

import { useCliAliases, cancelPendingSave } from "../stores/cliAliases";

describe("cliAliases store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // sanitize 依赖：注册 claude + codex 两 light profile（孤儿判定真值源）
    cliProfileRegistry._reset();
    cliProfileRegistry.register(fakeProfile("claude", ["claude"]));
    cliProfileRegistry.register(fakeProfile("codex", ["codex"]));
    useCliAliases.setState({ aliases: {}, loaded: false });
    vi.clearAllMocks();
    mockSaveSettings.mockResolvedValue(undefined);
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: false });
  });

  afterEach(() => {
    // STS-06：先取消活跃 debounce timer（fake timers 下仍有效），再切回真实 timer
    cancelPendingSave();
    vi.useRealTimers();
    cliProfileRegistry._reset();
  });

  // ── 默认值 ──

  it("默认 aliases 为空、loaded=false", () => {
    const s = useCliAliases.getState();
    expect(s.aliases).toEqual({});
    expect(s.loaded).toBe(false);
  });

  // ── addAlias / removeAlias ──

  it("addAlias 新 cliId 创建数组并追加", () => {
    useCliAliases.getState().addAlias("claude", "cc");
    useCliAliases.getState().addAlias("claude", "c");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc", "c"] });
  });

  it("addAlias 同值重复防御性 no-op（大小写敏感：cc 与 CC 不同值）", () => {
    useCliAliases.getState().addAlias("claude", "cc");
    useCliAliases.getState().addAlias("claude", "cc");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
    useCliAliases.getState().addAlias("claude", "CC");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc", "CC"] });
  });

  it("addAlias 不同 cliId 各自独立", () => {
    useCliAliases.getState().addAlias("claude", "cc");
    useCliAliases.getState().addAlias("codex", "cx");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"], codex: ["cx"] });
  });

  it("removeAlias 移除指定别名", () => {
    useCliAliases.setState({ aliases: { claude: ["cc", "c"] } });
    useCliAliases.getState().removeAlias("claude", "cc");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["c"] });
  });

  it("removeAlias 删光后键保留为空数组（形态稳定防抖动）", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    useCliAliases.getState().removeAlias("claude", "cc");
    expect(useCliAliases.getState().aliases).toEqual({ claude: [] });
  });

  it("removeAlias 未知 cliId/未知别名 no-op", () => {
    useCliAliases.setState({ aliases: { claude: ["cc"] } });
    useCliAliases.getState().removeAlias("ghost", "cc");
    useCliAliases.getState().removeAlias("claude", "absent");
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
  });

  // ── loadFromDisk ──

  it("loadFromDisk 首次启动（data:null）→ 空别名，loaded=true", async () => {
    await useCliAliases.getState().loadFromDisk();
    const s = useCliAliases.getState();
    expect(s.aliases).toEqual({});
    expect(s.loaded).toBe(true);
  });

  it("loadFromDisk 读取合法 cliAliases 段（sanitize 保留）", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { cliAliases: { claude: ["cc", "c"], codex: [] } },
      corrupted: false,
    });
    await useCliAliases.getState().loadFromDisk();
    expect(useCliAliases.getState().aliases).toEqual({
      claude: ["cc", "c"],
      codex: [],
    });
  });

  it("loadFromDisk sanitize：孤儿 cliId 丢弃、语法不过丢弃、撞内置丢弃、跨源重复先到先占", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        cliAliases: {
          claude: ["claude", "ok", "c c", "cc", "bad 'q'", "-x"],
          codex: ["cc", "cx"], // cc 已归 claude（注册序先），此处丢弃
          ghost: ["gg"],       // 孤儿 cliId → 整键丢弃
        },
      },
      corrupted: false,
    });
    await useCliAliases.getState().loadFromDisk();
    expect(useCliAliases.getState().aliases).toEqual({
      claude: ["ok", "cc"],
      codex: ["cx"],
    });
  });

  it("loadFromDisk cliAliases 键缺失 → 空别名", async () => {
    mockLoadSettings.mockResolvedValue({ data: { fontSize: 14 }, corrupted: false });
    await useCliAliases.getState().loadFromDisk();
    expect(useCliAliases.getState().aliases).toEqual({});
    expect(useCliAliases.getState().loaded).toBe(true);
  });

  it("loadFromDisk cliAliases 非对象 → 空别名", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { cliAliases: ["not", "object"] },
      corrupted: false,
    });
    await useCliAliases.getState().loadFromDisk();
    expect(useCliAliases.getState().aliases).toEqual({});
  });

  it("loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));
    await useCliAliases.getState().loadFromDisk();
    const s = useCliAliases.getState();
    expect(s.aliases).toEqual({});
    expect(s.loaded).toBe(true);
  });

  it("loadFromDisk corrupted=true → toast 告警（FE-11），数据仍消费", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { cliAliases: { claude: ["cc"] } },
      corrupted: true,
    });
    await useCliAliases.getState().loadFromDisk();
    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  // ── 持久化 ──

  it("loaded=false 时不触发 saveSettings", () => {
    useCliAliases.getState().addAlias("claude", "cc");
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("loaded=true 变更 → 2s debounce → saveSettings({cliAliases})", () => {
    useCliAliases.setState({ loaded: true });
    useCliAliases.getState().addAlias("claude", "cc");

    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      cliAliases: { claude: ["cc"] },
    });
  });

  it("多次变更 → debounce 只写最后一次", () => {
    useCliAliases.setState({ loaded: true });
    useCliAliases.getState().addAlias("claude", "cc");
    vi.advanceTimersByTime(500);
    useCliAliases.getState().addAlias("codex", "cx");
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      cliAliases: { claude: ["cc"], codex: ["cx"] },
    });
  });

  it("saveSettings 失败 → toast 告警 + console.warn（FE-09），store 状态不受影响", async () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    mockGetErrorMessage.mockReturnValue("write error");
    useCliAliases.setState({ loaded: true });

    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(() => {
      useCliAliases.getState().addAlias("claude", "cc");
    }).not.toThrow();
    // 异步推进 timer 并排空微任务：saveSettings reject 的 .catch 在微任务队列执行
    await vi.advanceTimersByTimeAsync(2000);

    expect(mockToastShow).toHaveBeenCalledWith("warning", "设置保存失败，重启后将丢失");
    expect(mockGetErrorMessage).toHaveBeenCalledWith(expect.any(Error));
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[stores/cliAliases]"),
      "write error",
    );

    expect(useCliAliases.getState().aliases).toEqual({ claude: ["cc"] });

    consoleWarnSpy.mockRestore();
  });

  it("cancelPendingSave 取消活跃 timer——推进 2s 不再写盘", () => {
    useCliAliases.setState({ loaded: true });
    useCliAliases.getState().addAlias("claude", "cc"); // 产生 debounce timer
    vi.advanceTimersByTime(1500);                       // 未到 2s，timer 仍活跃
    cancelPendingSave();                                // 关窗冲刷：取消待执行保存
    vi.advanceTimersByTime(2000);                       // 越过原定触发点
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });
});
