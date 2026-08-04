// claude-history-restore.test.ts — FE-06 四步恢复编排测试（L2）
//
// mock 边界（只守 JS 侧形状，真实编排由 Stage 06 E2E 兜底）：
//   stores/projects（useProjects.getState + ID 生成）、features/sidebar（makeEmptyLayout）、
//   workspace/pageApis（switchToPageShared/getPageApi）、ipc/pty（write）、
//   panels/terminal/TerminalRegistry（get）、ipc/notification（sendToastNotification）
// 全部 mock 经 vi.hoisted() 创建，确保模块级 vi.mock 执行前就绪（项目测试惯例）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { restoreHistorySession } from "../features/claudeHistory/restoreSession";
import type { HistorySession } from "../types/claudeHistory";
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

vi.mock("../features/sidebar", () => ({
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

function makeSession(overrides: Partial<HistorySession> = {}): HistorySession {
  return {
    sessionId: SESSION_ID,
    cwd: "C:\\Users\\test\\proj",
    title: "测试会话",
    titleSource: "customTitle",
    firstPrompt: "帮我写代码",
    mtimeMs: 123456,
    cwdExists: true,
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

    // 步骤 4a：addPanel 参数（panelId = terminal-{pageId}-{Date.now()}，cwd 透传）
    expect(apiStub.addPanel).toHaveBeenCalledTimes(1);
    expect(apiStub.addPanel).toHaveBeenCalledWith({
      id: expect.stringMatching(/^terminal-page-restore-test-\d+$/),
      component: "terminal",
      title: "claude",
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
    // 内容断言为准（vitest mock.calls 参数跨 realm，instanceof 不可靠）
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
});
