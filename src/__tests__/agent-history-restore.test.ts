// agent-history-restore.test.ts — FE-06 四步恢复编排测试（L2）
//
// mock 边界（只守 JS 侧形状，真实编排由 Stage 06 E2E 兜底）：
//   stores/projects（useProjects.getState + ID 生成）、features/sidebar（makeEmptyLayout）、
//   workspace/pageApis（switchToPageShared/getPageApi）、ipc/pty（write）、
//   panels/terminal/TerminalRegistry（get）、ipc/notification（sendToastNotification）
// 全部 mock 经 vi.hoisted() 创建，确保模块级 vi.mock 执行前就绪（项目测试惯例）。
//
// Stage 05（MC-315）：第 4 步注入内容 = profile.history.buildRestoreInput 输出、
// addPanel title = session.title ?? profile.tabTitle（人工验证问题 3——初始标题
// 直接用历史会话标题，读不到兜底 claude 名）——side-effect import profiles 注册
// 真实 claude profile（claude-history-cap 交付），注入内容断言与 claude 策略输出
// 逐字一致（`claude --resume <id>` + fork 追加 ` --fork-session` + `\r` 结尾）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  restoreHistorySession,
  waitFor,
} from "../features/agentHistory/restoreSession";
import { resetTerminalPanelSeq } from "../lib/panelId";
import "../features/cliProfiles/profiles";
import type { AgentHistorySession } from "../types/agentHistory";
import type { Project, OperationPage } from "../stores/projects";

// ── vi.hoisted 共享 mock 状态 ──────────────────────────────

const h = vi.hoisted(() => {
  let projects: Record<string, Project> = {};
  const mockAddProject = vi.fn();
  const mockAddPage = vi.fn();
  const mockSwitchToPageShared = vi.fn();
  const mockGetPageApi = vi.fn();
  const mockTerminalRegistryGet = vi.fn();
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
    mockGetPageApi,
    mockTerminalRegistryGet,
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
  getPageApi: h.mockGetPageApi,
}));

vi.mock("../ipc/pty", () => ({
  write: h.mockPtyWrite,
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: { get: h.mockTerminalRegistryGet },
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

describe("restoreHistorySession 四步恢复编排", () => {
  let apiStub: { addPanel: ReturnType<typeof vi.fn> };
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    resetTerminalPanelSeq(); // B14: 模块级每页计数隔离（跨用例残留会使 id 断言漂移）
    h.setProjects({});
    h.mockAddProject.mockReset();
    h.mockAddPage.mockReset();
    h.mockSwitchToPageShared.mockReset().mockResolvedValue(undefined);
    h.mockGetPageApi.mockReset();
    h.mockTerminalRegistryGet.mockReset();
    h.mockPtyWrite.mockReset().mockResolvedValue(undefined);
    h.mockSendToastNotification.mockReset();
    apiStub = { addPanel: vi.fn() };
    h.mockGetPageApi.mockReturnValue(apiStub);
    h.mockTerminalRegistryGet.mockReturnValue({ sessionId: "session-test-1" });
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
      cwd: "C:\\Users\\test\\proj",
      createdAt: expect.any(Number),
      lastAccessedAt: expect.any(Number),
    });

    // 步骤 3：页面切换目标 = 新建页面
    expect(h.mockSwitchToPageShared).toHaveBeenCalledTimes(1);
    expect(h.mockSwitchToPageShared).toHaveBeenCalledWith("page-restore-test");

    // 步骤 4a：addPanel 参数（B14：panelId = terminal-{pageId}-{seq} 单点生成，cwd 透传；
    // 人工验证问题 3：初始标题 = session.title（历史回退链合成结果））
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).toHaveBeenCalledWith({
      id: expect.stringMatching(/^terminal-page-restore-test-\d+$/),
      component: "terminal",
      title: "测试会话",
      params: {
        panelId: expect.stringMatching(/^terminal-page-restore-test-\d+$/),
        cwd: "C:\\Users\\test\\proj",
      },
      renderer: "always",
    });

    // 步骤 4b：pty.write payload（sessionId 来自 TerminalRegistry，命令以 \r 结尾）
    expect(h.mockPtyWrite).toHaveBeenCalledTimes(1);
    const [sessionId, panelId, data] = h.mockPtyWrite.mock
      .calls[0] as [string, string, Uint8Array];
    expect(sessionId).toBe("session-test-1");
    expect(panelId).toMatch(/^terminal-page-restore-test-\d+$/);
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

  it("初始标题两分支（人工验证问题 3）：session.title 非空用它，null 兜底 profile.tabTitle", async () => {
    // 分支 1：title null（新建/读不到名称）→ 兜底 claude profile tabTitle
    await restoreHistorySession(makeSession({ title: null }));
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({ title: "claude" }),
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
    // 每页计数确定性递增：首次 terminal-{pageId}-0、二次 -1
    expect(firstId).toBe("terminal-page-restore-test-0");
    expect(secondId).toBe("terminal-page-restore-test-1");
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

  it("FE-27: getPageApi 恒不就绪 → waitFor 超时 → 统一失败 toast，不 addPanel（signal 接线不破坏超时路径）", async () => {
    vi.useFakeTimers();
    try {
      h.mockGetPageApi.mockReturnValue(undefined);
      const pending = restoreHistorySession(makeSession());
      // 越过 50×100ms 轮询上限 → waitFor 抛超时错 → 外层 toast
      await vi.advanceTimersByTimeAsync(50 * 100);
      await pending;

      expect(h.mockSendToastNotification).toHaveBeenCalledWith(
        "恢复会话失败",
        expect.objectContaining({ body: expect.stringContaining("5s 内未就绪") }),
      );
      expect(apiStub.addPanel).not.toHaveBeenCalled();
      expect(h.mockPtyWrite).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });
});

// ═══════════════════════════════════════════════════════════════════
// FE-27 waitFor AbortSignal 中止（测试专用导出直测）
// ═══════════════════════════════════════════════════════════════════
describe("FE-27 waitFor AbortSignal", () => {
  it("signal 已 abort → 第一轮即抛「已取消」，probe 未被调用", async () => {
    const controller = new AbortController();
    controller.abort();
    const probe = vi.fn(() => undefined);

    await expect(waitFor(probe, "测试条件", controller.signal)).rejects.toThrow(
      "测试条件 已取消",
    );
    expect(probe).not.toHaveBeenCalled();
  });

  it("轮询中 abort → 停止轮询抛「已取消」，probe 次数停在 abort 前", async () => {
    vi.useFakeTimers();
    try {
      const controller = new AbortController();
      const probe = vi.fn(() => undefined);
      const pending = waitFor(probe, "测试条件", controller.signal);

      // 300ms 推进内共 4 次 probe：t=0 首次 + 100/200/300 三个定时器各触发一次
      await vi.advanceTimersByTimeAsync(300);
      expect(probe).toHaveBeenCalledTimes(4);

      // abort → waitFor 尚挂起在 t=300 处未到期的 100ms 定时器上，须再推进
      // 100ms 让其进入下一轮循环（循环开头检查 aborted → 抛错），此后不再 probe。
      // 先注册 rejects 断言再推进——推进触发 reject 时 handler 须已就位（防 unhandled rejection）
      controller.abort();
      const abortAssertion = expect(pending).rejects.toThrow("测试条件 已取消");
      await vi.advanceTimersByTimeAsync(100);
      await abortAssertion;
      expect(probe).toHaveBeenCalledTimes(4); // abort 后不再轮询
    } finally {
      vi.useRealTimers();
    }
  });

  it("无 signal 后向兼容：条件满足即返回，超时仍抛错", async () => {
    let calls = 0;
    const probe = vi.fn(() => {
      calls += 1;
      return calls >= 3 ? "ready" : undefined;
    });
    await expect(waitFor(probe, "测试条件")).resolves.toBe("ready");
    expect(calls).toBe(3);
  });
});
