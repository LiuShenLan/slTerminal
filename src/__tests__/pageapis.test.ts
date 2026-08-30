// pageapis.test.ts — pageApis.ts 页面切换核心测试（WRK-02）
//
// 直接调用 pageApis.ts 导出函数（不经 Workspace 组件），验证：
// - switchToPageShared：DBG-5/9 时序契约——setProjectRoot 先 await 完成、
//   再 setActivePage（spy invocationCallOrder 断言）；幂等短路；reject 降级；
//   __dockviewApi 重指（D7 时序断言）；rootPath 空/页面不存在跳过 setProjectRoot
// - switchToPageAndFocus：轮询命中（100ms×50 上限）/延迟命中/超时降级（console.warn 不抛异常）

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import type { DockviewApi } from "dockview-react";
import {
  switchToPageShared,
  switchToPageAndFocus,
  registerPageApi,
  unregisterPageApi,
  getAllPageApis,
  findPanelForSession,
  findPageIdForPanelId,
} from "../workspace/pageApis";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";

/** fake 面板（getPanel 命中时的返回值） */
interface FakePanel {
  id: string;
  focus: ReturnType<typeof vi.fn>;
}

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
      { pageId: pageA, name: "Alpha", layout: {}, cwd: ROOT_PATH,
        createdAt: 1, lastAccessedAt: 1 },
      { pageId: pageB, name: "Beta", layout: {}, cwd: ROOT_PATH,
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
  let getPanelImpl: () => FakePanel | undefined = () => undefined;
  const api = {
    id: "fake-api",
    getPanel: vi.fn(() => getPanelImpl()),
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

/** fake api 断言辅助：仅需要 getPanel 的成员，cast 满足 registerPageApi 的 DockviewApi 签名 */
function castFakeApi(api: ReturnType<typeof makeFakeApi>): DockviewApi {
  return api as unknown as DockviewApi;
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
});

afterEach(() => {
  vi.useRealTimers();
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
    const { pageB } = seedTwoPageProject();
    const setActivePageSpy = vi.spyOn(useLayout.getState(), "setActivePage");

    const pending = switchToPageShared(pageB);

    // setProjectRoot 已调用（挂起 await）但 setActivePage 未执行
    await Promise.resolve();
    expect(mocks.mockSetProjectRoot).toHaveBeenCalledWith(ROOT_PATH);
    expect(useLayout.getState().activePageId).toBe("page-alpha");
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

  it("D7：__dockviewApi 重指向目标页 api（页面已初始化时）", async () => {
    const { pageB } = seedTwoPageProject();
    const apiB = makeFakeApi();
    registerPageApi(pageB, castFakeApi(apiB));

    const pending = switchToPageShared(pageB);
    await Promise.resolve();
    mocks.resolve();
    await pending;

    expect(window.__dockviewApi).toBe(apiB);
    unregisterPageApi(pageB);
  });

  it("__dockviewApi 不重指未注册页面（handlePageApiReady 兜底路径不动它）", async () => {
    const { pageB } = seedTwoPageProject();
    const pending = switchToPageShared(pageB);
    await Promise.resolve();
    mocks.resolve();
    await pending;

    expect(window.__dockviewApi).toBeUndefined();
    expect(useLayout.getState().activePageId).toBe(pageB);
  });

  it("getAllPageApis 遍历全部已注册页面 api（含隐藏页面；E2E 兜底清理用）", async () => {
    const { pageA, pageB } = seedTwoPageProject();
    const apiA = castFakeApi(makeFakeApi());
    const apiB = castFakeApi(makeFakeApi());
    registerPageApi(pageA, apiA);
    registerPageApi(pageB, apiB);

    const apis = getAllPageApis();
    expect(apis).toContain(apiA);
    expect(apis).toContain(apiB);

    unregisterPageApi(pageA);
    unregisterPageApi(pageB);
    expect(getAllPageApis()).not.toContain(apiA);
    expect(getAllPageApis()).not.toContain(apiB);
  });

  it("rootPath 为空 → 跳过 setProjectRoot，直接切换", async () => {
    const pageC = "page-gamma";
    useProjects.getState().addProject({
      projectId: "proj-noroot",
      name: "no-root",
      rootPath: "",
      pages: [{
        pageId: pageC, name: "Gamma", layout: {}, cwd: "",
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
    registerPageApi(pageB, castFakeApi(api));

    const pending = switchToPageAndFocus(pageB, "panel-x");
    await Promise.resolve(); // 挂起在 setProjectRoot
    mocks.resolve();
    await pending;

    expect(api.focusSpy).toHaveBeenCalledTimes(1);
    // 第 1 次轮询即命中（不消耗 100ms 定时器）
    expect(api.getPanel).toHaveBeenCalledTimes(1);
    unregisterPageApi(pageB);
  });

  it("延迟命中：面板第 3 次轮询可用 → 100ms×2 后 focus()", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.setPanelAvailableAfter(2); // 第 3 次调用返回面板
    registerPageApi(pageB, castFakeApi(api));

    const pending = switchToPageAndFocus(pageB, "panel-x");
    await Promise.resolve(); // 挂起在 setProjectRoot（deferred 非 timer）
    mocks.resolve();
    // 前 2 次轮询（200ms）无面板
    await vi.advanceTimersByTimeAsync(200);
    // 第 3 次命中
    await vi.advanceTimersByTimeAsync(100);
    await pending;

    expect(api.focusSpy).toHaveBeenCalledTimes(1);
    unregisterPageApi(pageB);
  });

  it("超时降级：50 次轮询（5s）无面板 → console.warn + 不抛异常 + focus 未调用", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.neverPanel();
    registerPageApi(pageB, castFakeApi(api));
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
    unregisterPageApi(pageB);
  });

  it("FE-26: abort 后停止轮询——不 focus、不再 getPanel、无 warn（卸载/再次点击场景）", async () => {
    vi.useFakeTimers();
    const { pageB } = seedTwoPageProject();
    const api = makeFakeApi();
    api.neverPanel();
    registerPageApi(pageB, castFakeApi(api));
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
    unregisterPageApi(pageB);
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
        ["terminal-page-alpha-0", entry({ sessionId: "s1", cliId: CLAUDE_CLI_ID })],
        ["terminal-page-beta-0", entry({ sessionId: "s2", cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s2")).toBe("terminal-page-beta-0");
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("terminal-page-alpha-0");
  });

  it("usageSourcePath 回退：无 sessionId 时 basename 去 .jsonl 参与匹配", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([
        ["terminal-page-alpha-0", entry({ usageSourcePath: "C:/data/s1.jsonl", cliId: CLAUDE_CLI_ID })],
        ["terminal-page-alpha-1", entry({ usageSourcePath: "C:/data/raw-s2", cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("terminal-page-alpha-0");
    // 非 .jsonl 后缀 basename 原样匹配
    expect(findPanelForSession(CLAUDE_CLI_ID, "raw-s2")).toBe("terminal-page-alpha-1");
  });

  it("cliId 缺省回退：条目无 cliId 时按 CLAUDE_CLI_ID 匹配（keyOf 回退，ZQ-1）", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([["terminal-page-alpha-0", entry({ sessionId: "s1" })]]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("terminal-page-alpha-0");
  });

  it("未命中 → undefined（含无 agentSession 条目与 sessionId/usageSourcePath 双无跳过）", () => {
    mocks.mockTerminalGetAll.mockReturnValue(
      new Map([
        ["terminal-page-alpha-0", entry({ sessionId: "s1", cliId: CLAUDE_CLI_ID })],
        // 无 agentSession（undefined/null）→ 跳过
        ["terminal-page-alpha-1", { agentSession: undefined }],
        ["terminal-page-alpha-2", { agentSession: null }],
        // sessionId 与 usageSourcePath 双无 → 跳过
        ["terminal-page-alpha-3", entry({ cliId: CLAUDE_CLI_ID })],
      ]),
    );
    expect(findPanelForSession(CLAUDE_CLI_ID, "ghost")).toBeUndefined();
    expect(findPanelForSession(CLAUDE_CLI_ID, "s1")).toBe("terminal-page-alpha-0");
  });
});

// ═══════════════════════════════════════════════════════════════
// findPageIdForPanelId（FE-09 自 NavTree 上提——B14 防御分层：前缀匹配优先 + parse 兜底）
// ═══════════════════════════════════════════════════════════════

describe("findPageIdForPanelId", () => {
  it("前缀匹配优先：旧恢复格式 terminal-{pageId}-{Date.now}-{seq} 归已知页面（B14）", () => {
    seedTwoPageProject(); // 已知页面集合 = page-alpha/page-beta
    // 旧格式含 Date.now 数字段——语法切分会把数字段误并入 pageId 得幽灵页面，前缀匹配可靠
    expect(findPageIdForPanelId("terminal-page-alpha-1700000000000-0")).toBe(
      "page-alpha",
    );
    expect(findPageIdForPanelId("terminal-page-beta-1")).toBe("page-beta");
  });

  it("parse 兜底：新格式 panelId 不在已知页面集合 → parseTerminalPageId", () => {
    seedTwoPageProject();
    expect(findPageIdForPanelId("terminal-page-ghost-0")).toBe("page-ghost");
  });

  it("均未命中 → null（无前缀匹配且 parse 不出）", () => {
    seedTwoPageProject();
    expect(findPageIdForPanelId("foo-1")).toBeNull();
    expect(findPageIdForPanelId("")).toBeNull();
  });
});
