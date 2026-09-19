// agent-history-restore.test.ts — FE-06 四步恢复编排测试（L2）
//
// mock 边界（只守 JS 侧形状，真实编排由 Stage 06 E2E 兜底）：
//   stores/projects(useProjects.getState + ID 生成)、features/navTree(makeEmptyLayout,
//   NAV-06 随 SidebarTree 退役迁入——mock 目标即 ../features/navTree/NavTree)、
//   workspace/pageApis（switchToPageShared/waitPageApi）、ipc/pty（write）、
//   panels/terminal/TerminalRegistry（get/subscribe——事件驱动恢复链的事件派发面）、
//   ipc/notification（sendToastNotification）
// 全部 mock 经 vi.hoisted() 创建，确保模块级 vi.mock 执行前就绪（项目测试惯例）。
//
// Stage 05（MC-315）：第 4 步注入内容 = profile.history.buildRestoreInput 输出、
// addPanel title = session.title ?? session.sessionId 前 8 位（人工验证问题 3——
// 初始标题直接用历史会话标题，读不到兜底 = sessionId 前 8 位，与历史行同口径）——
// side-effect import profiles 注册
// 真实 claude profile（claude-history-cap 交付），注入内容断言与 claude 策略输出
// 逐字一致（`claude --resume <id>` + fork 追加 ` --fork-session` + `\r` 结尾）。
//
// 恢复链三段等待 = 事件驱动（waitPageApi / register 事件 / promptReady 事件），
// 本测试经 mockSubscribe 捕获 listener 主动派发事件驱动——零轮询空转即防复发锁。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  restoreHistorySession,
  waitForTerminalRegister,
  waitForPromptReady,
} from "../features/agentHistory/restoreSession";
import { resetTerminalPanelSeq } from "../lib/panelId";
import "../features/cliProfiles/profiles";
import type { AgentHistorySession } from "../types/agentHistory";
import type { Project, OperationPage } from "../stores/projects";
import type { RegistryEvent } from "../panels/terminal/TerminalRegistry";

// ── vi.hoisted 共享 mock 状态 ──────────────────────────────

const h = vi.hoisted(() => {
  let projects: Record<string, Project> = {};
  const mockAddProject = vi.fn();
  const mockAddPage = vi.fn();
  const mockSwitchToPageShared = vi.fn();
  const mockWaitPageApi = vi.fn();
  const mockTerminalRegistryGet = vi.fn();
  // subscribe 捕获 listener——测试经 fireRegistryEvent 主动派发事件
  let registryListeners: ((e: RegistryEvent) => void)[] = [];
  const mockSubscribe = vi.fn((listener: (e: RegistryEvent) => void) => {
    registryListeners.push(listener);
    return vi.fn(); // 退订 no-op（断言语义在事件派发侧）
  });
  const mockPtyWrite = vi.fn();
  const mockSendToastNotification = vi.fn();
  return {
    // 模拟 useProjects.getState() 快照（projects 为可种子/重置的可变引用）
    getProjectsState: () => ({
      projects,
      addProject: mockAddProject,
      addPage: mockAddPage,
    }),
    setProjects: (p: Record<string, Project>) => {
      projects = p;
    },
    mockAddProject,
    mockAddPage,
    mockSwitchToPageShared,
    mockWaitPageApi,
    mockTerminalRegistryGet,
    mockSubscribe,
    fireRegistryEvent: (e: RegistryEvent) => {
      for (const l of registryListeners) l(e);
    },
    resetRegistryListeners: () => {
      registryListeners = [];
    },
    mockPtyWrite,
    mockSendToastNotification,
  };
});

vi.mock("../stores/projects", () => ({
  useProjects: { getState: () => h.getProjectsState() },
  createProjectId: () => "proj-restore-test",
  createPageId: () => "page-restore-test",
}));

// NAV-06：makeEmptyLayout 随 SidebarTree 退役迁入 navTree（restoreSession 消费点改引用）
vi.mock("../features/navTree/NavTree", () => ({
  makeEmptyLayout: () => ({}),
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageShared: h.mockSwitchToPageShared,
  waitPageApi: h.mockWaitPageApi,
}));

vi.mock("../ipc/pty", () => ({
  write: h.mockPtyWrite,
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    get: h.mockTerminalRegistryGet,
    subscribe: h.mockSubscribe,
  },
}));

vi.mock("../ipc/notification", () => ({
  sendToastNotification: h.mockSendToastNotification,
}));

// ── 测试数据 ──────────────────────────────────────────────

const SESSION_ID = "1a2b3c4d-1111-2222-3333-444455556666";

function makeSession(
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: SESSION_ID,
    cwd: "C:\\Users\\test\\proj",
    title: "测试会话",
    titleSource: "customTitle",
    firstPrompt: "帮我写代码",
    mtimeMs: 123456,
    cwdExists: true,
    cliId: "claude",
    ...overrides,
  };
}

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    projectId: "proj-existing",
    name: "proj",
    rootPath: "C:\\Users\\test\\proj",
    pages: [],
    activePageId: null,
    version: 1,
    ...overrides,
  };
}

function makePage(overrides: Partial<OperationPage> = {}): OperationPage {
  return {
    pageId: "page-existing",
    name: "页面-1",
    layout: {},
    createdAt: 1000,
    lastAccessedAt: 1000,
    ...overrides,
  };
}

/** 从 addPanel 调用记录提取本次恢复的 panelId（事件派发匹配用） */
function panelIdOf(apiStub: { addPanel: ReturnType<typeof vi.fn> }): string {
  return (apiStub.addPanel.mock.calls[0][0] as { id: string }).id;
}

describe("restoreHistorySession 四步恢复编排", () => {
  let apiStub: { addPanel: ReturnType<typeof vi.fn> };
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    resetTerminalPanelSeq(); // B14: 模块级每页计数隔离（跨用例残留会使 id 断言漂移）
    h.setProjects({});
    h.mockAddProject.mockReset();
    h.mockAddPage.mockReset();
    h.mockSwitchToPageShared.mockReset().mockResolvedValue(undefined);
    h.mockWaitPageApi.mockReset();
    h.mockTerminalRegistryGet.mockReset();
    h.mockSubscribe.mockClear();
    h.resetRegistryListeners();
    h.mockPtyWrite.mockReset().mockResolvedValue(undefined);
    h.mockSendToastNotification.mockReset();
    apiStub = { addPanel: vi.fn() };
    h.mockWaitPageApi.mockResolvedValue(apiStub);
    // 默认桩：pwsh + 首个提示符已渲染——就绪闸门直查命中立即放行（不等事件不空转超时）
    h.mockTerminalRegistryGet.mockReturnValue({
      sessionId: "session-test-1",
      shellKind: "pwsh",
      promptReady: true,
    });
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("四步顺序：addProject → addPage → 切页 → addPanel → pty.write", async () => {
    await restoreHistorySession(makeSession());

    // 步骤 1：项目入列（无匹配项目 → addProject，字段形状照 SidebarTree.handleAddProject）
    expect(h.mockAddProject).toHaveBeenCalledTimes(1);
    expect(h.mockAddProject).toHaveBeenCalledWith({
      projectId: "proj-restore-test",
      name: "proj",
      rootPath: "C:\\Users\\test\\proj",
      pages: [],
      activePageId: null,
      version: 1,
    });

    // 步骤 2：页面保障（新项目无页 → addPage，空布局 + 「页面-N」）
    expect(h.mockAddPage).toHaveBeenCalledTimes(1);
    expect(h.mockAddPage).toHaveBeenCalledWith("proj-restore-test", {
      pageId: "page-restore-test",
      name: expect.stringMatching(/^页面-\d+$/),
      layout: {},
      createdAt: expect.any(Number),
      lastAccessedAt: expect.any(Number),
    });

    // 步骤 3：页面切换目标 = 新建页面
    expect(h.mockSwitchToPageShared).toHaveBeenCalledTimes(1);
    expect(h.mockSwitchToPageShared).toHaveBeenCalledWith("page-restore-test");

    // 步骤 4a：addPanel 参数（B14：panelId 页前缀协议单点生成，cwd 透传；
    // 人工验证问题 3：初始标题 = session.title（历史回退链合成结果））
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).toHaveBeenCalledWith({
      id: expect.stringMatching(/^page-restore-test:terminal-\d+$/),
      component: "terminal",
      title: "测试会话",
      params: {
        panelId: expect.stringMatching(/^page-restore-test:terminal-\d+$/),
        cwd: "C:\\Users\\test\\proj",
      },
      renderer: "always",
    });

    // 步骤 4b：pty.write payload（sessionId 来自 TerminalRegistry，命令以 \r 结尾）
    expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
    const [sessionId, panelId, data] = h.mockPtyWrite.mock
      .calls[0] as [string, string, Uint8Array];
    expect(sessionId).toBe("session-test-1");
    expect(panelId).toMatch(/^page-restore-test:terminal-\d+$/);
    // 内容断言为准（vitest mock.calls 参数跨 realm，instanceof 不可靠）；
    // 注入内容 = claude profile.history.buildRestoreInput 输出（MC-315 委托，
    // 与迁出源 restoreSession.ts 字面量逐字一致——断言漂移即实现有误）
    expect(new TextDecoder().decode(data)).toBe(
      `claude --resume ${SESSION_ID}\r`,
    );

    // 五步调用严格按序（invocationCallOrder 全局递增）
    const order = [
      h.mockAddProject.mock.invocationCallOrder[0],
      h.mockAddPage.mock.invocationCallOrder[0],
      h.mockSwitchToPageShared.mock.invocationCallOrder[0],
      apiStub.addPanel.mock.invocationCallOrder[0],
      h.mockPtyWrite.mock.invocationCallOrder[0],
    ];
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });

  it("初始标题两分支（人工验证问题 3）：session.title 非空用它，null 兜底 sessionId 前 8 位", async () => {
    // 分支 1：title null（新建/读不到名称）→ 兜底 = sessionId 前 8 位（与历史行同口径）
    await restoreHistorySession(makeSession({ title: null }));
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({ title: SESSION_ID.slice(0, 8) }),
    );

    // 分支 2：title 非空 → 初始标题直接用历史标题（首个用例已覆盖
    // 「测试会话」断言；此处补 titleSource=firstPrompt 形态兜底链产物）
    apiStub.addPanel.mockClear();
    await restoreHistorySession(
      makeSession({ title: "首条提问", titleSource: "firstPrompt" }),
    );
    expect(apiStub.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({ title: "首条提问" }),
    );
  });

  it("已有匹配项目（大小写/斜杠不敏感）→ 跳过项目入列与建页，直接复用首个页面", async () => {
    // 种子项目 rootPath 用相反大小写 + 正斜杠——验证决策 24 规范化比较
    h.setProjects({
      "proj-existing": makeProject({
        rootPath: "c:/users/test/proj",
        pages: [makePage({ pageId: "page-existing" })],
      }),
    });

    await restoreHistorySession(makeSession());

    expect(h.mockAddProject).not.toHaveBeenCalled();
    expect(h.mockAddPage).not.toHaveBeenCalled();
    expect(h.mockSwitchToPageShared).toHaveBeenCalledWith("page-existing");
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
  });

  it("已有项目但无页面 → 仅补建页面，不重复入列", async () => {
    h.setProjects({ "proj-existing": makeProject() });

    await restoreHistorySession(makeSession());

    expect(h.mockAddProject).not.toHaveBeenCalled();
    expect(h.mockAddPage).toHaveBeenCalledTimes(1);
    expect(h.mockAddPage).toHaveBeenCalledWith(
      "proj-existing",
      expect.objectContaining({ pageId: "page-restore-test" }),
    );
    expect(h.mockSwitchToPageShared).toHaveBeenCalledWith("page-restore-test");
  });

  it("fork 恢复：payload 追加 --fork-session", async () => {
    await restoreHistorySession(makeSession(), { fork: true });

    const [, , data] = h.mockPtyWrite.mock.calls[0] as [
      string,
      string,
      Uint8Array,
    ];
    expect(new TextDecoder().decode(data)).toBe(
      `claude --resume ${SESSION_ID} --fork-session\r`,
    );
  });

  it("同页两次串行恢复 → panelId 相异（B14：模块级每页计数，ZQ-4 语义）", async () => {
    await restoreHistorySession(makeSession());
    await restoreHistorySession(makeSession());

    const [firstId, secondId] = apiStub.addPanel.mock.calls.map(
      (call) => (call[0] as { id: string }).id,
    );
    // 每页计数确定性递增：首次 terminal-0、二次 terminal-1（页前缀协议形态）
    expect(firstId).toBe("page-restore-test:terminal-0");
    expect(secondId).toBe("page-restore-test:terminal-1");
  });

  it("防重入：恢复进行中并发调用直接返回、无副作用，完成后标记复位", async () => {
    // 第一次调用卡在切页步骤（switchToPageShared 挂起），restoring=true 持续
    let resolveSwitch: () => void = () => {};
    h.mockSwitchToPageShared.mockReturnValue(
      new Promise<void>((resolve) => {
        resolveSwitch = resolve;
      }),
    );

    const first = restoreHistorySession(makeSession());
    await vi.waitFor(() =>
      expect(h.mockSwitchToPageShared).toHaveBeenCalled(),
    );

    // 第二次并发调用：直接返回，不再触发任何编排步骤
    await expect(restoreHistorySession(makeSession())).resolves.toBeUndefined();
    expect(h.mockAddProject).toHaveBeenCalledTimes(1);
    expect(h.mockAddPage).toHaveBeenCalledTimes(1);
    expect(h.mockSwitchToPageShared).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).not.toHaveBeenCalled();
    expect(h.mockPtyWrite).not.toHaveBeenCalled();

    // 放行第一次调用 → 完成后 restoring 复位，第三次调用可正常执行（完整编排再次跑通）
    resolveSwitch();
    await first;
    h.mockSwitchToPageShared.mockResolvedValue(undefined);
    await restoreHistorySession(makeSession());
    expect(h.mockAddProject).toHaveBeenCalledTimes(2);
    expect(apiStub.addPanel).toHaveBeenCalledTimes(2);
    expect(h.mockPtyWrite).toHaveBeenCalledTimes(2);
  });

  it("失败路径：addPanel 抛错 → toast + console.error，promise 不 reject", async () => {
    apiStub.addPanel.mockImplementation(() => {
      throw new Error("addPanel boom");
    });

    await expect(restoreHistorySession(makeSession())).resolves.toBeUndefined();

    expect(h.mockSendToastNotification).toHaveBeenCalledTimes(1);
    expect(h.mockSendToastNotification).toHaveBeenCalledWith(
      "恢复会话失败",
      expect.objectContaining({ body: expect.stringContaining("addPanel boom") }),
    );
    expect(consoleErrorSpy).toHaveBeenCalled();
    // 恢复流程中止：不注入恢复命令
    expect(h.mockPtyWrite).not.toHaveBeenCalled();
  });

  it("防御：cwd 为 null 直接失败（toast 携 cwd 错误消息），不触发任何编排步骤", async () => {
    await expect(
      restoreHistorySession(makeSession({ cwd: null, cwdExists: false })),
    ).resolves.toBeUndefined();

    // 守卫经 doRestore 抛错（「会话缺少工作目录（cwd），无法恢复」）→ 外层 toast 上报
    // （NAH-07②：cwd null 守卫的失败以 toast 形式暴露，不 rethrow——封装契约）
    expect(h.mockSendToastNotification).toHaveBeenCalledTimes(1);
    expect(h.mockSendToastNotification).toHaveBeenCalledWith(
      "恢复会话失败",
      expect.objectContaining({ body: expect.stringContaining("cwd") }),
    );
    expect(h.mockAddProject).not.toHaveBeenCalled();
    expect(h.mockAddPage).not.toHaveBeenCalled();
    expect(h.mockSwitchToPageShared).not.toHaveBeenCalled();
    expect(apiStub.addPanel).not.toHaveBeenCalled();
    expect(h.mockPtyWrite).not.toHaveBeenCalled();
  });

  it("FE-27: 页 API 不就绪（waitPageApi 超时返 undefined）→ 统一失败 toast，不 addPanel", async () => {
    h.mockWaitPageApi.mockResolvedValue(undefined);
    await restoreHistorySession(makeSession());

    expect(h.mockSendToastNotification).toHaveBeenCalledWith(
      "恢复会话失败",
      expect.objectContaining({ body: expect.stringContaining("5s 内未就绪") }),
    );
    expect(apiStub.addPanel).not.toHaveBeenCalled();
    expect(h.mockPtyWrite).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════
// 恢复注入就绪闸门（OSC 133;A → promptReady 事件；cmd 固定 500ms；超时兜底注入）
// ═══════════════════════════════════════════════════════════════════
describe("restoreHistorySession 就绪闸门", () => {
  let apiStub: { addPanel: ReturnType<typeof vi.fn> };
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    resetTerminalPanelSeq();
    h.setProjects({});
    h.mockAddProject.mockReset();
    h.mockAddPage.mockReset();
    h.mockSwitchToPageShared.mockReset().mockResolvedValue(undefined);
    h.mockWaitPageApi.mockReset();
    h.mockTerminalRegistryGet.mockReset();
    h.mockSubscribe.mockClear();
    h.resetRegistryListeners();
    h.mockPtyWrite.mockReset().mockResolvedValue(undefined);
    h.mockSendToastNotification.mockReset();
    apiStub = { addPanel: vi.fn() };
    h.mockWaitPageApi.mockResolvedValue(apiStub);
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("promptReady 事件到达即注入（事件驱动——零轮询 tick，防复发锁）", async () => {
    const entry = { sessionId: "session-test-1", shellKind: "pwsh", promptReady: false };
    h.mockTerminalRegistryGet.mockReturnValue(entry);

    const pending = restoreHistorySession(makeSession());
    // 等编排推进到闸门订阅（直查未命中 → subscribe 等 promptReady 事件）
    await vi.waitFor(() => expect(h.mockSubscribe).toHaveBeenCalled());
    expect(h.mockPtyWrite).not.toHaveBeenCalled();

    // 模拟 OSC 133;A 到达 → markPromptReady → promptReady 事件 → 立即放行注入
    // （不经任何定时器推进——事件驱动语义断言）
    h.fireRegistryEvent({ type: "promptReady", panelId: panelIdOf(apiStub) });
    await pending;

    expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
    const [sessionId, , data] = h.mockPtyWrite.mock.calls[0] as [string, string, Uint8Array];
    expect(sessionId).toBe("session-test-1");
    expect(new TextDecoder().decode(data)).toBe(`claude --resume ${SESSION_ID}\r`);
  });

  it("promptReady 永不到达 → 10s 超时兜底仍注入 + console.warn 留痕", async () => {
    vi.useFakeTimers();
    try {
      const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
      h.mockTerminalRegistryGet.mockReturnValue({
        sessionId: "session-test-1",
        shellKind: "pwsh",
        promptReady: false,
      });

      const pending = restoreHistorySession(makeSession());
      // 推进微任务链到闸门订阅（事件驱动等待无轮询 tick，0ms 推进即落定）
      await vi.advanceTimersByTimeAsync(0);
      expect(h.mockSubscribe).toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(10000);
      await pending;

      expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
      expect(consoleWarnSpy).toHaveBeenCalledWith(
        expect.stringContaining("首个提示符等待超时，兜底注入"),
      );
      consoleWarnSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shellKind=cmd（无 shell integration）→ 固定 500ms 延迟后注入，不等 promptReady", async () => {
    vi.useFakeTimers();
    try {
      h.mockTerminalRegistryGet.mockReturnValue({
        sessionId: "session-test-1",
        shellKind: "cmd",
        promptReady: false,
      });

      const pending = restoreHistorySession(makeSession());
      // 500ms 前不注入
      await vi.advanceTimersByTimeAsync(499);
      expect(h.mockPtyWrite).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(1);
      await pending;
      expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("闸门超时 = 10s 自定义预算（非段 2 的 5s）：10s 前不注入不报错", async () => {
    vi.useFakeTimers();
    try {
      h.mockTerminalRegistryGet.mockReturnValue({
        sessionId: "session-test-1",
        shellKind: "pwsh",
        promptReady: false,
      });

      const pending = restoreHistorySession(makeSession());
      // 越过 5s 仍不注入——证明闸门用 PROMPT_READY_TIMEOUT_MS=10s 参数
      await vi.advanceTimersByTimeAsync(9999);
      expect(h.mockPtyWrite).not.toHaveBeenCalled();
      expect(h.mockSendToastNotification).not.toHaveBeenCalled();

      // 到达 10s → 超时兜底注入
      await vi.advanceTimersByTimeAsync(1);
      await pending;
      expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("取消语义穿透：闸门等待 abort → reject 已取消（不兜底注入的分支守卫等价锁定）", async () => {
    // 分支覆盖：`catch (err) { if (signal.aborted) throw err; }`——
    // 公共 API 层防重入（restoring）先于 abort() 返回，在途闸门无法经
    // restoreHistorySession 二次调用 abort（FE-27 防御性死路径，已口头登记）；
    // 本用例经 waitForPromptReady 直测锁定等价语义：已 abort signal → 「已取消」reject
    const controller = new AbortController();
    h.mockTerminalRegistryGet.mockReturnValue({
      sessionId: "session-test-1",
      shellKind: "pwsh",
      promptReady: false,
    });
    controller.abort();
    await expect(
      waitForPromptReady("page-restore-test:terminal-0", controller.signal),
    ).rejects.toThrow("已取消");
  });
});

// ═══════════════════════════════════════════════════════════════════
// 事件驱动等待 helper 直测（FE-27 abort 语义 / 直查命中 / 超时）
// ═══════════════════════════════════════════════════════════════════
describe("waitForTerminalRegister / waitForPromptReady", () => {
  beforeEach(() => {
    h.mockTerminalRegistryGet.mockReset();
    h.mockSubscribe.mockClear();
    h.resetRegistryListeners();
  });

  it("register 直查命中 → 立即返回条目，不订阅", async () => {
    const entry = { sessionId: "s", shellKind: "pwsh", promptReady: false };
    h.mockTerminalRegistryGet.mockReturnValue(entry);

    await expect(waitForTerminalRegister("p1")).resolves.toBe(entry);
    expect(h.mockSubscribe).not.toHaveBeenCalled();
  });

  it("未注册 → subscribe 等 register 事件，事件到达即 resolve 现值", async () => {
    const entry = { sessionId: "s", shellKind: "pwsh", promptReady: false };
    h.mockTerminalRegistryGet.mockReturnValueOnce(undefined).mockReturnValue(entry);

    const pending = waitForTerminalRegister("p2");
    await vi.waitFor(() => expect(h.mockSubscribe).toHaveBeenCalled());
    h.fireRegistryEvent({ type: "register", panelId: "p2" });
    await expect(pending).resolves.toBe(entry);
  });

  it("其它 panelId / 其它事件类型 → 不放行", async () => {
    const entry = { sessionId: "s", shellKind: "pwsh", promptReady: false };
    h.mockTerminalRegistryGet.mockReturnValueOnce(undefined).mockReturnValue(entry);

    const pending = waitForTerminalRegister("p3", undefined, 50);
    await vi.waitFor(() => expect(h.mockSubscribe).toHaveBeenCalled());
    h.fireRegistryEvent({ type: "register", panelId: "other" });
    h.fireRegistryEvent({ type: "promptReady", panelId: "p3" });
    h.fireRegistryEvent({ type: "remove", panelId: "p3" });
    // 50ms 超时 reject 证明上述事件均未放行
    await expect(pending).rejects.toThrow("在 0.05s 内未就绪");
  });

  it("signal 已 abort → 即抛「已取消」", async () => {
    h.mockTerminalRegistryGet.mockReturnValue(undefined);
    const controller = new AbortController();
    controller.abort();
    await expect(
      waitForTerminalRegister("p4", controller.signal),
    ).rejects.toThrow("终端面板 p4 的 PTY 会话 已取消");
  });

  it("等待中 abort → 立即 reject 不等超时（FE-48 语义平移）", async () => {
    h.mockTerminalRegistryGet.mockReturnValue(undefined);
    const controller = new AbortController();
    const pending = waitForTerminalRegister("p5", controller.signal, 60000);
    const assertion = expect(pending).rejects.toThrow("已取消");
    controller.abort();
    await assertion; // 无定时器推进即完成——abort listener 直驱
  });

  it("promptReady 已置位 → 直查命中立即返回，不订阅", async () => {
    h.mockTerminalRegistryGet.mockReturnValue({ promptReady: true });
    await expect(waitForPromptReady("p6")).resolves.toBeUndefined();
    expect(h.mockSubscribe).not.toHaveBeenCalled();
  });

  it("promptReady 未置位 → subscribe 等事件，超时 reject「10s 内未就绪」", async () => {
    vi.useFakeTimers();
    try {
      h.mockTerminalRegistryGet.mockReturnValue({ promptReady: false });
      const pending = waitForPromptReady("p7", undefined, 10000);
      const assertion = expect(pending).rejects.toThrow(
        "终端面板 p7 的首个提示符 在 10s 内未就绪",
      );
      await vi.advanceTimersByTimeAsync(10000);
      await assertion;
    } finally {
      vi.useRealTimers();
    }
  });
});
