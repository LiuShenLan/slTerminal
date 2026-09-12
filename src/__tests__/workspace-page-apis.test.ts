// pageapis.test.ts — pageApis.ts 页面切换核心测试（WRK-02，CP-004 单宿主语义适配）
//
// 直接调用 pageApis.ts 导出函数（不经 Workspace 组件），验证：
// - switchToPageShared：DBG-5/9 时序契约——setProjectRoot 先 await 完成、
//   再 setActivePage（spy invocationCallOrder 断言）；幂等短路；reject 降级；
//   rootPath 空/页面不存在跳过 setProjectRoot
// - 宿主语义（CP-004）：getPageApi(pageId) = 宿主 api（页组挂载标记后）；
//   window.__dockviewApi 不随切页重指（宿主唯一，由 WorkspaceDockHost 置位）
// - switchToPageAndFocus：轮询命中（100ms×50 上限）/延迟命中/超时降级（console.warn 不抛异常）
// - findPanelForSession / findPageIdForPanelId（页前缀协议 + 旧格式前缀兜底）

import { describe, it, expect, beforeEach, vi } from "vitest";
import type { DockviewApi } from "dockview-react";
import {
  switchToPageShared,
  switchToPageAndFocus,
  registerHostApi,
  unregisterHostApi,
  markPageGroupMounted,
  getAllPageApis,
  getPageApi,
  findPanelForSession,
  findPageIdForPanelId,
  PAGE_API_READY_EVENT,
} from "../workspace/pageApis";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";

// ─── setProjectRoot 手动控制（hoisted，供 vi.mock 使用） ───
const mocks = vi.hoisted(() => {
  let resolveSPR!: (value: void) => void;
  let rejectSPR!: (reason?: unknown) => void;
  let sprPromise = Promise.resolve();
  const resetDeferred = () => {
    sprPromise = new Promise<void>((res, rej) => {
      resolveSPR = res;
      rejectSPR = rej;
    });
  };
  resetDeferred();
  const mockSetProjectRoot = vi.fn((_path: string) => {
    void _path;
    return sprPromise;
  });
  const mockTerminalGetAll = vi.fn(() => new Map());
  // BE-23：switchToPageShared 失败 toast 断言用
  const mockToastShow = vi.fn();
  return {
    mockSetProjectRoot,
    mockTerminalGetAll,
    mockToastShow,
    get resolve() { return () => { resolveSPR(); }; },
    get reject() { return (err?: unknown) => { rejectSPR(err); }; },
    resetDeferred() {
      resetDeferred();
      mockSetProjectRoot.mockClear();
      mockToastShow.mockClear();
    },
  };
});

vi.mock("../ipc/fs", () => ({
  setProjectRoot: mocks.mockSetProjectRoot,
}));

// BE-23：pageApis 新增 toast import——mock ../lib 隔离断言（其余导出保持真实实现）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: { ...actual.toast, show: mocks.mockToastShow } };
});

// FE-09：findPanelForSession 反查 TerminalRegistry（getAll 经 mockTerminalGetAll 注入）
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: () => mocks.mockTerminalGetAll(),
  },
}));

// ─── 辅助 ───

const ROOT_PATH = "C:\\switch-test";

/** 种子：一项目两页面（activePageA） */
function seedTwoPageProject() {
  const projId = "proj-pa";
  const pageA = "page-alpha";
  const pageB = "page-beta";
  useProjects.getState().addProject({
    projectId: projId,
    name: "pa-test",
    rootPath: ROOT_PATH,
    pages: [
      { pageId: pageA, name: "Alpha", layout: {},
        createdAt: 1, lastAccessedAt: 1 },
      { pageId: pageB, name: "Beta", layout: {},
        createdAt: 2, lastAccessedAt: 2 },
    ],
    activePageId: pageA,
    version: 1,
  });
  useLayout.setState({ activePageId: pageA });
  return { projId, pageA, pageB };
}

/** 构造 fake DockviewApi（getPanel 可定制） */
function makeFakeApi() {
  const focusSpy = vi.fn();
  let getPanelImpl: () => { id: string; focus: ReturnType<typeof vi.fn> } | undefined
    = () => undefined;
  const api = {
    id: "fake-api",
    getPanel: vi.fn(() => getPanelImpl()),
    groups: [] as Array<{ id: string }>,
    /** 让 getPanel 从第 N 次调用起返回面板 */
    setPanelAvailableAfter(attempts: number) {
      let calls = 0;
      getPanelImpl = () => {
        calls++;
        return calls > attempts ? { id: "panel-x", focus: focusSpy } : undefined;
      };
    },
    setPanelAlwaysAvailable() {
      getPanelImpl = () => ({ id: "panel-x", focus: focusSpy });
    },
    neverPanel() {
      getPanelImpl = () => undefined;
    },
    focusSpy,
  };
  return api;
}

/** fake api 断言辅助：仅需要 getPanel 的成员，cast 满足 DockviewApi 签名 */
function castFakeApi(api: ReturnType<typeof makeFakeApi>): DockviewApi {
  return api as unknown as DockviewApi;
}

/** 注册宿主 fake api + 标记页组挂载（测试装配单点） */
function registerFakeHost(pageIds: string[], api?: ReturnType<typeof makeFakeApi>): DockviewApi {
  const host = castFakeApi(api ?? makeFakeApi());
  registerHostApi(host);
  for (const pageId of pageIds) markPageGroupMounted(pageId);
  return host;
}

beforeEach(() => {
  mocks.resetDeferred();
  mocks.mockTerminalGetAll.mockReset();
  mocks.mockTerminalGetAll.mockReturnValue(new Map());
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  window.__dockviewApi = undefined;
  unregisterHostApi();
});

describe("switchToPageShared", () => {
  it("幂等：activePageId 已为目标 → 不调用 setProjectRoot/setActivePage", async () => {
    const { pageA } = seedTwoPageProject();
    const setActivePageSpy = vi.spyOn(useLayout.getState(), "setActivePage");

    await switchToPageShared(pageA);

    expect(mocks.mockSetProjectRoot).not.toHaveBeenCalled();
    expect(setActivePageSpy).not.toHaveBeenCalled();
    expect(useLayout.getState().activePageId).toBe(pageA);
    setActivePageSpy.mockRestore();
  });

  it("DBG-5 时序：setProjectRoot 先 await 完成再 setActivePage（activePageId 在 resolve 前不变）", async () => {
    const { pageA, pageB } = seedTwoPageProject();
    const setActivePageSpy = vi.spyOn(useLayout.getState(), "setActivePage");

    const pending = switchToPageShared(pageB);

    // setProjectRoot 已调用（挂起 await）但 setActivePage 未执行
    await Promise.resolve();
    expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(ROOT_PATH);
    expect(useLayout.getState().activePageId).toBe(pageA);
    expect(setActivePageSpy).not.toHaveBeenCalled();

    // resolve 后 setActivePage 执行
    mocks.resolve();
    await pending;
    expect(useLayout.getState().activePageId).toBe(pageB);
    setActivePageSpy.mockRestore();
  });

  it("DBG-9 时序：setProjectRoot 调用先于 setActivePage（invocationCallOrder）", async () => {
    const { pageB } = seedTwoPageProject();
    const setActivePageSpy = vi.spyOn(useLayout.getState(), "setActivePage");

    const pending = switchToPageShared(pageB);
    await Promise.resolve(); // 挂起在 setProjectRoot await
    mocks.resolve();
    await pending;

    const sprOrder = mocks.mockSetProjectRoot.mock.invocationCallOrder[0];
    const setPageOrder = setActivePageSpy.mock.invocationCallOrder[0];
    expect(sprOrder).toBeLessThan(setPageOrder);
    setActivePageSpy.mockRestore();
  });

  it("setProjectRoot reject → console.error 降级 + 仍完成切换", async () => {
    const { pageB } = seedTwoPageProject();
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const pending = switchToPageShared(pageB);
    await Promise.resolve();
    mocks.reject(new Error("路径不存在"));
    await pending;

    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "[slTerminal] 设置项目根路径失败:",
      expect.any(Error),
    );
    expect(useLayout.getState().activePageId).toBe(pageB);
    consoleErrorSpy.mockRestore();
  });

  it("BE-23：setProjectRoot reject → toast.show warning 告警且切换仍完成", async () => {
    const { pageB } = seedTwoPageProject();
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const pending = switchToPageShared(pageB);
    await Promise.resolve();
    mocks.reject(new Error("路径不存在"));
    await pending;

    // BE-23：失败 toast 可感知（文案照 FE-04 既有先例，与 Workspace SEC-01 effect 一致）
    expect(mocks.mockToastShow).toHaveBeenCalledWith(
      "warning",
      "项目根路径设置失败，文件操作可能被拒绝",
    );
    // 切换仍完成（降级不阻断）
    expect(useLayout.getState().activePageId).toBe(pageB);
    consoleErrorSpy.mockRestore();
  });

  it("CP-004：切页不重指 window.__dockviewApi（宿主唯一——由 WorkspaceDockHost 置位）", async () => {
    const { pageB } = seedTwoPageProject();
    // 宿主已注册场景：__dockviewApi 指向宿主常量
    registerFakeHost(["page-alpha", pageB]);
    window.__dockviewApi = castFakeApi(makeFakeApi());
    const before = window.__dockviewApi;

    const pending = switchToPageShared(pageB);
    await Promise.resolve();
    mocks.resolve();
    await pending;

    expect(useLayout.getState().activePageId).toBe(pageB);
    expect(window.__dockviewApi).toBe(before); // 不随切页变动
  });

  it("getPageApi：宿主就绪 + 页组挂载标记后才返回宿主 api", () => {
    const { pageA, pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    // 未注册宿主 → undefined
    expect(getPageApi(pageA)).toBeUndefined();
    // 注册宿主但未标记页组 → undefined（旧「页面未初始化」语义）
    registerHostApi(castFakeApi(api));
    expect(getPageApi(pageA)).toBeUndefined();
    // 标记后 → 宿主
    markPageGroupMounted(pageA);
    expect(getPageApi(pageA)).toBe(api);
    expect(getPageApi(pageB)).toBeUndefined();
  });

  it("getAllPageApis：单宿主语义 = [宿主]（页组面板过滤由调用方经页前缀完成）", () => {
    const { pageA, pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    registerFakeHost([pageA, pageB], api);
    expect(getAllPageApis()).toEqual([api]);
    unregisterHostApi();
    expect(getAllPageApis()).toEqual([]);
  });

  it("rootPath 为空 → 跳过 setProjectRoot，直接切换", async () => {
    const pageC = "page-gamma";
    useProjects.getState().addProject({
      projectId: "proj-noroot",
      name: "no-root",
      rootPath: "",
      pages: [{
        pageId: pageC, name: "Gamma", layout: {},
        createdAt: 1, lastAccessedAt: 1,
      }],
      activePageId: null,
      version: 1,
    });
    useLayout.setState({ activePageId: null });

    await switchToPageShared(pageC);

    expect(mocks.mockSetProjectRoot).not.toHaveBeenCalled();
    expect(useLayout.getState().activePageId).toBe(pageC);
  });

  it("pageId 不在任何项目 → 不调用 setProjectRoot，仍切换", async () => {
    seedTwoPageProject();
    await switchToPageShared("ghost-page");
    expect(mocks.mockSetProjectRoot).not.toHaveBeenCalled();
    expect(useLayout.getState().activePageId).toBe("ghost-page");
  });
});

describe("switchToPageAndFocus", () => {
  it("轮询立即命中：面板可用 → focus() 且无多余轮询", async () => {
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.setPanelAlwaysAvailable();
    registerFakeHost([pageB], api);

    const pending = switchToPageAndFocus(pageB, "panel-x");
    await Promise.resolve(); // 挂起在 setProjectRoot
    mocks.resolve();
    await pending;

    expect(api.focusSpy).toHaveBeenCalledTimes(1);
    // 第 1 次轮询即命中（不消耗 100ms 定时器）
    expect(api.getPanel).toHaveBeenCalledTimes(1);
  });

  it("延迟命中：面板第 3 次轮询可用 → 100ms×2 后 focus()", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.setPanelAvailableAfter(2); // 第 3 次调用返回面板
    registerFakeHost([pageB], api);

    const pending = switchToPageAndFocus(pageB, "panel-x");
    await Promise.resolve(); // 挂起在 setProjectRoot（deferred 非 timer）
    mocks.resolve();
    // 前 2 次轮询（200ms）无面板
    await vi.advanceTimersByTimeAsync(200);
    // 第 3 次命中
    await vi.advanceTimersByTimeAsync(100);
    await pending;

    expect(api.focusSpy).toHaveBeenCalledTimes(1);
  });

  it("超时降级：50 次轮询（5s）无面板 → console.warn + 不抛异常 + focus 未调用", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.neverPanel();
    registerFakeHost([pageB], api);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const pending = switchToPageAndFocus(pageB, "panel-x");
    await Promise.resolve();
    mocks.resolve();
    await vi.advanceTimersByTimeAsync(50 * 100);
    await pending;

    expect(warnSpy).toHaveBeenCalledWith(
      "[slTerminal] 面板 panel-x 在 5s 内未就绪，无法聚焦",
    );
    expect(api.focusSpy).not.toHaveBeenCalled();
    expect(api.getPanel).toHaveBeenCalledTimes(50);
    warnSpy.mockRestore();
  });

  it("FE-26: abort 后停止轮询——不 focus、不再 getPanel、无 warn（卸载/再次点击场景）", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.neverPanel();
    registerFakeHost([pageB], api);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const controller = new AbortController();
    const pending = switchToPageAndFocus(pageB, "panel-x", controller.signal);
    await Promise.resolve();
    mocks.resolve();
    // 前两次轮询（100ms 间隔）无面板
    await vi.advanceTimersByTimeAsync(100);
    expect(api.getPanel).toHaveBeenCalledTimes(2);

    // abort（调用方卸载/再次点击）→ FE-48：abort listener 立即 clearTimeout + resolve，
    // 不等 100ms 轮询定时器——advance 0 即完成（原实现须等定时器到期才能进下一轮检查）
    controller.abort();
    await vi.advanceTimersByTimeAsync(0);
    await pending;

    expect(api.focusSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled(); // abort 静默退出，不按超时 warn
    expect(api.getPanel).toHaveBeenCalledTimes(2); // 停在 abort 前的轮询次数
    warnSpy.mockRestore();
  });
});

// ═══════════════════════════════════════════════════════════════
// findPanelForSession（FE-09 自 NavTree 上提——复合键反查运行中会话所在终端面板）
// ═══════════════════════════════════════════════════════════════

describe("findPanelForSession", () => {
  /** 构造 RegisteredTerminal 形状条目（agentSession 最小字段） */
  function entry(session: Record<string, unknown>): unknown {
    return { agentSession: { lastEventAt: Date.now(), ...session } };
  }

  it("复合键命中：cliId|sessionId 精确匹配返回对应 panelId（keyOf 同键形态，MC-313）", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([
        ["page-alpha:terminal-0", entry({ sessionId: "s1", cliId: CLAUDE_CLI_ID })],
        ["page-beta:terminal-0", entry({ sessionId: "s2", cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s2")).toBe("page-beta:terminal-0");
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("page-alpha:terminal-0");
  });

  it("usageSourcePath 回退：无 sessionId 时 basename 去 .jsonl 参与匹配", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([
        ["page-alpha:terminal-0", entry({ usageSourcePath: "C:/data/s1.jsonl", cliId: CLAUDE_CLI_ID })],
        ["page-alpha:terminal-1", entry({ usageSourcePath: "C:/data/raw-s2", cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("page-alpha:terminal-0");
    // 非 .jsonl 后缀 basename 原样匹配
    expect(findPanelForSession(CLAUDE_CLI_ID, "raw-s2")).toBe("page-alpha:terminal-1");
  });

  it("cliId 缺省回退：条目无 cliId 时按 CLAUDE_CLI_ID 匹配（keyOf 回退，ZQ-1）", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([["page-alpha:terminal-0", entry({ sessionId: "s1" })]]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("page-alpha:terminal-0");
  });

  it("未命中 → undefined（含无 agentSession 条目与 sessionId/usageSourcePath 双无跳过）", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([
        ["page-alpha:terminal-0", entry({ sessionId: "s1", cliId: CLAUDE_CLI_ID })],
        // 无 agentSession（undefined/null）→ 跳过
        ["page-alpha:terminal-1", { agentSession: undefined }],
        ["page-alpha:terminal-2", { agentSession: null }],
        // sessionId 与 usageSourcePath 双无 → 跳过
        ["page-alpha:terminal-3", entry({ cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "ghost")).toBeUndefined();
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("page-alpha:terminal-0");
  });
});

// ═══════════════════════════════════════════════════════════════
// findPageIdForPanelId（CP-004 页前缀协议快速路径 + 旧格式前缀兜底）
// ═══════════════════════════════════════════════════════════════

describe("findPageIdForPanelId", () => {
  it("页前缀协议 id（新形态）→ 首个冒号前段即属主页", () => {
    seedTwoPageProject(); // 已知页面集合 = page-alpha/page-beta
    expect(findPageIdForPanelId("page-alpha:terminal-0")).toBe("page-alpha");
    expect(findPageIdForPanelId("page-ghost:terminal-3")).toBe("page-ghost");
  });

  it("旧格式前缀匹配兜底：terminal-{pageId}-{...} 归已知页面（B14 语义保留）", () => {
    seedTwoPageProject();
    // 旧格式含 Date.now 数字段——语法切分不可靠，按已知页面集合前缀匹配
    expect(findPageIdForPanelId("terminal-page-alpha-1700000000000-0")).toBe(
      "page-alpha",
    );
    expect(findPageIdForPanelId("terminal-page-beta-1")).toBe("page-beta");
    // settings 旧形态（settings-{pageId}）同样按已知页集合匹配
    expect(findPageIdForPanelId("settings-page-alpha")).toBe("page-alpha");
  });

  it("均未命中 → null（无页前缀且旧格式不在已知页集合）", () => {
    seedTwoPageProject();
    expect(findPageIdForPanelId("foo-1")).toBeNull();
    expect(findPageIdForPanelId("")).toBeNull();
    expect(findPageIdForPanelId("terminal-page-ghost-0")).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════
// markPageGroupMounted 事件派发（CP-042——openSettingsPanel 事件驱动等待的就绪信号源）
// ═══════════════════════════════════════════════════════════════

describe("markPageGroupMounted 事件派发", () => {
  it("页组挂载后 window 收到 slterm:page-api-ready 且 detail === pageId", () => {
    const listener = vi.fn();
    window.addEventListener(PAGE_API_READY_EVENT, listener);
    const pageId = "page-ready-event";
    markPageGroupMounted(pageId);
    expect(listener).toHaveBeenCalledTimes(1);
    const event = listener.mock.calls[0][0] as CustomEvent<string>;
    expect(event.detail).toBe(pageId);
    window.removeEventListener(PAGE_API_READY_EVENT, listener);
  });
});
