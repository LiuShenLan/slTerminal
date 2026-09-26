// agent-global-files-store.test.ts — agentGlobalFiles Zustand Store 自动化测试
//
// 测试模式照 cli-aliases.test.ts：mock ../ipc/settings 与 ../lib，getState() 直接操作，
// fake timers 测 debounce。loadFromDisk 的 sanitize 需 profile 已注册（孤儿 cliId 判定）
// ——beforeEach 注册 light fake profile，afterEach _reset 隔离注册表。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../features/cliProfiles/types";

/** light fake profile（iconSrc 不被解析，随意路径） */
function fakeProfile(id: string): CodingCliProfile {
  return {
    id,
    displayName: id,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: {},
  };
}

// ─── Hoisted mocks ───
const { mockSaveSettings, mockLoadSettings, mockToastShow, mockGetErrorMessage } = vi.hoisted(() => ({
  mockSaveSettings: vi.fn().mockResolvedValue(undefined),
  mockLoadSettings: vi.fn().mockResolvedValue({ data: null, corrupted: false }),
  mockToastShow: vi.fn(),
  mockGetErrorMessage: vi.fn((err: unknown) => String(err)),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

vi.mock("../lib", () => ({
  toast: { show: mockToastShow, _reset: vi.fn() },
  getErrorMessage: mockGetErrorMessage,
}));

import { useAgentGlobalFiles, cancelPendingSave } from "../stores/agentGlobalFiles";

describe("agentGlobalFiles store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    cliProfileRegistry._reset();
    cliProfileRegistry.register(fakeProfile("claude"));
    cliProfileRegistry.register(fakeProfile("codex"));
    useAgentGlobalFiles.setState({ configs: {}, loaded: false });
    vi.clearAllMocks();
    mockSaveSettings.mockResolvedValue(undefined);
    mockLoadSettings.mockResolvedValue({ data: null, corrupted: false });
  });

  afterEach(() => {
    cancelPendingSave();
    vi.useRealTimers();
    cliProfileRegistry._reset();
  });

  // ── 默认值 ──

  it("默认 configs 为空、loaded=false", () => {
    const s = useAgentGlobalFiles.getState();
    expect(s.configs).toEqual({});
    expect(s.loaded).toBe(false);
  });

  // ── setMode ──

  it("setMode 无既有配置时从缺省起步（customNames/showRuntimeFiles 补齐缺省值）", () => {
    useAgentGlobalFiles.getState().setMode("claude", "custom");
    expect(useAgentGlobalFiles.getState().configs.claude).toEqual({
      mode: "custom",
      customNames: [],
      showRuntimeFiles: false,
    });
  });

  it("setMode 保留既有名单与开关", () => {
    useAgentGlobalFiles.setState({
      configs: {
        claude: { mode: "all", customNames: ["agents"], showRuntimeFiles: true },
      },
    });
    useAgentGlobalFiles.getState().setMode("claude", "custom");
    expect(useAgentGlobalFiles.getState().configs.claude).toEqual({
      mode: "custom",
      customNames: ["agents"],
      showRuntimeFiles: true,
    });
  });

  // ── addCustomName / removeCustomName ──

  it("addCustomName trim 后追加；不同 cliId 各自独立", () => {
    useAgentGlobalFiles.getState().addCustomName("claude", "  agents  ");
    useAgentGlobalFiles.getState().addCustomName("codex", "skills");
    expect(useAgentGlobalFiles.getState().configs.claude.customNames).toEqual(["agents"]);
    expect(useAgentGlobalFiles.getState().configs.codex.customNames).toEqual(["skills"]);
  });

  it("addCustomName 空白 / 大小写不敏感重复 → no-op", () => {
    useAgentGlobalFiles.getState().addCustomName("claude", "   ");
    expect(useAgentGlobalFiles.getState().configs.claude).toBeUndefined();
    useAgentGlobalFiles.getState().addCustomName("claude", "agents");
    useAgentGlobalFiles.getState().addCustomName("claude", "AGENTS");
    expect(useAgentGlobalFiles.getState().configs.claude.customNames).toEqual(["agents"]);
  });

  it("removeCustomName 移除指定项；未知 cliId no-op", () => {
    useAgentGlobalFiles.setState({
      configs: {
        claude: { mode: "custom", customNames: ["agents", "skills"], showRuntimeFiles: false },
      },
    });
    useAgentGlobalFiles.getState().removeCustomName("claude", "agents");
    expect(useAgentGlobalFiles.getState().configs.claude.customNames).toEqual(["skills"]);
    useAgentGlobalFiles.getState().removeCustomName("ghost", "agents");
    expect(useAgentGlobalFiles.getState().configs.ghost).toBeUndefined();
  });

  // ── setShowRuntimeFiles ──

  it("setShowRuntimeFiles 切换开关，无配置时缺省起步", () => {
    useAgentGlobalFiles.getState().setShowRuntimeFiles("claude", true);
    expect(useAgentGlobalFiles.getState().configs.claude).toEqual({
      mode: "all",
      customNames: [],
      showRuntimeFiles: true,
    });
  });

  // ── loadFromDisk ──

  it("loadFromDisk 首次启动（data:null）→ 空配置，loaded=true", async () => {
    await useAgentGlobalFiles.getState().loadFromDisk();
    const s = useAgentGlobalFiles.getState();
    expect(s.configs).toEqual({});
    expect(s.loaded).toBe(true);
  });

  it("loadFromDisk 读取合法段；sanitize 孤儿 cliId 丢弃 + 非法字段回退", async () => {
    mockLoadSettings.mockResolvedValue({
      data: {
        agentGlobalFiles: {
          claude: { mode: "custom", customNames: ["agents"], showRuntimeFiles: true },
          ghost: { mode: "all", customNames: [], showRuntimeFiles: false },
          codex: { mode: "everything", customNames: "bad", showRuntimeFiles: 1 },
        },
      },
      corrupted: false,
    });
    await useAgentGlobalFiles.getState().loadFromDisk();
    expect(useAgentGlobalFiles.getState().configs).toEqual({
      claude: { mode: "custom", customNames: ["agents"], showRuntimeFiles: true },
      codex: { mode: "all", customNames: [], showRuntimeFiles: false },
    });
  });

  it("loadFromDisk 段缺失 / 非对象 → 空配置", async () => {
    mockLoadSettings.mockResolvedValue({ data: { fontSize: 14 }, corrupted: false });
    await useAgentGlobalFiles.getState().loadFromDisk();
    expect(useAgentGlobalFiles.getState().configs).toEqual({});

    useAgentGlobalFiles.setState({ loaded: false });
    mockLoadSettings.mockResolvedValue({
      data: { agentGlobalFiles: ["not", "object"] },
      corrupted: false,
    });
    await useAgentGlobalFiles.getState().loadFromDisk();
    expect(useAgentGlobalFiles.getState().configs).toEqual({});
  });

  it("loadFromDisk 异常 → 保持默认，loaded=true", async () => {
    mockLoadSettings.mockRejectedValue(new Error("disk error"));
    await useAgentGlobalFiles.getState().loadFromDisk();
    const s = useAgentGlobalFiles.getState();
    expect(s.configs).toEqual({});
    expect(s.loaded).toBe(true);
  });

  it("loadFromDisk corrupted=true → toast 告警（FE-11），数据仍消费", async () => {
    mockLoadSettings.mockResolvedValue({
      data: { agentGlobalFiles: { claude: { mode: "all", customNames: [], showRuntimeFiles: true } } },
      corrupted: true,
    });
    await useAgentGlobalFiles.getState().loadFromDisk();
    expect(useAgentGlobalFiles.getState().configs.claude.showRuntimeFiles).toBe(true);
    expect(mockToastShow).toHaveBeenCalledWith("warning", "配置已损坏，已回退默认值");
  });

  // ── 持久化 ──

  it("loaded=false 时不触发 saveSettings", () => {
    useAgentGlobalFiles.getState().setMode("claude", "custom");
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });

  it("loaded=true 变更 → 2s debounce → saveSettings({agentGlobalFiles}) 段形态", () => {
    useAgentGlobalFiles.setState({ loaded: true });
    useAgentGlobalFiles.getState().setMode("claude", "custom");

    vi.advanceTimersByTime(1500);
    expect(mockSaveSettings).not.toHaveBeenCalled();

    vi.advanceTimersByTime(600);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      agentGlobalFiles: {
        claude: { mode: "custom", customNames: [], showRuntimeFiles: false },
      },
    });
  });

  it("多次变更 → debounce 只写最后一次", () => {
    useAgentGlobalFiles.setState({ loaded: true });
    useAgentGlobalFiles.getState().setMode("claude", "custom");
    vi.advanceTimersByTime(500);
    useAgentGlobalFiles.getState().setShowRuntimeFiles("claude", true);
    vi.advanceTimersByTime(2000);

    expect(mockSaveSettings).toHaveBeenCalledTimes(1);
    expect(mockSaveSettings).toHaveBeenCalledWith({
      agentGlobalFiles: {
        claude: { mode: "custom", customNames: [], showRuntimeFiles: true },
      },
    });
  });

  it("saveSettings 失败 → toast 告警 + console.warn（FE-09），store 状态不受影响", async () => {
    mockSaveSettings.mockRejectedValue(new Error("write error"));
    mockGetErrorMessage.mockReturnValue("write error");
    useAgentGlobalFiles.setState({ loaded: true });

    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(() => {
      useAgentGlobalFiles.getState().setMode("claude", "custom");
    }).not.toThrow();
    await vi.advanceTimersByTimeAsync(2000);

    expect(mockToastShow).toHaveBeenCalledWith("warning", "设置保存失败，重启后将丢失");
    expect(mockGetErrorMessage).toHaveBeenCalledWith(expect.any(Error));
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      expect.stringContaining("[stores/agentGlobalFiles]"),
      "write error",
    );

    expect(useAgentGlobalFiles.getState().configs.claude.mode).toBe("custom");

    consoleWarnSpy.mockRestore();
  });

  it("cancelPendingSave 取消活跃 timer——推进 2s 不再写盘", () => {
    useAgentGlobalFiles.setState({ loaded: true });
    useAgentGlobalFiles.getState().setMode("claude", "custom");
    vi.advanceTimersByTime(1500);
    cancelPendingSave();
    vi.advanceTimersByTime(2000);
    expect(mockSaveSettings).not.toHaveBeenCalled();
  });
});
