// commit-open-file.test.ts — openCommitFile 分派与去重 L2 测试
//
// 覆盖：状态→面板分派、四条守卫路径（未知状态/activePageId 缺失/dockApi 缺失/rootPath 缺失）、
// addPanel 失败降级、recomputeTitles 标题应用、suffix 去重聚焦（B10 正反向）、STATUS_PANEL_MAP 映射表。
// 拆分自原 commit-view.test.tsx（SVC-14）。
// 纯逻辑测试——无组件渲染，stores/titleManager 用真实实现，window.__dockviewApi 手工 mock。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

import { openCommitFile, STATUS_PANEL_MAP, getPanelDispatch } from "../features/commit/openCommitFile";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ── 辅助函数：种子 stores ──
function seedProject(rootPath: string) {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: undefined,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { "proj-1": true },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  titleManager.reset();
}

type Fn = ReturnType<typeof vi.fn>;

/** 构造默认空 dockApi 并返回生效的 Mock 句柄（每用例可覆盖） */
function mockDockApi(overrides: { addPanel?: Fn; getPanel?: Fn } = {}) {
  const addPanel = overrides.addPanel ?? vi.fn();
  const getPanel = overrides.getPanel ?? vi.fn(() => null);
  window.__dockviewApi = {
    addPanel,
    getPanel,
  } as unknown as typeof window.__dockviewApi;
  return { addPanel, getPanel };
}

beforeEach(() => {
  resetStores();
  mockDockApi();
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__dockviewApi; // 防过期 mock 外泄（TQ-B-15）
});

describe("openCommitFile 双击分派", () => {
  it("modified 状态打开 diff 面板", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/a.ts", "modified");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
    expect(callArgs.title).toContain("(git diff)");
    expect(callArgs.params.filePath).toBe("C:/repo/src/a.ts");
    expect(callArgs.params.repoPath).toBe("C:/repo");
  });

  it("added 状态打开 editor 面板含 (git add)", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/b.ts", "added");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("editor");
    expect(callArgs.title).toContain("(git add)");
  });

  it("untracked 状态打开 editor 面板含 (git not add)", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/c.ts", "untracked");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("editor");
    expect(callArgs.title).toContain("(git not add)");
  });

  it("deleted 状态打开 gitshow 面板含 (git delete)", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/d.ts", "deleted");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("gitshow");
    expect(callArgs.title).toContain("(git delete)");
  });

  it("renamed 状态传 oldPath 到 params", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/new.ts", "renamed", "C:/repo/src/old.ts");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
    expect(callArgs.params.oldPath).toBe("C:/repo/src/old.ts");
  });

  it("renamed 未传 oldPath 时 params 不含 oldPath", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/src/new.ts", "renamed");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
    expect(callArgs.params.oldPath).toBeUndefined();
  });
});

describe("openCommitFile 守卫路径", () => {
  it("未知状态直接返回（!dispatch 守卫）", () => {
    seedProject("C:/repo");
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/a.ts", "unknown");

    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("activePageId 缺失时不执行", () => {
    resetStores();
    // useLayout.activePageId 为 null
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/a.ts", "modified");

    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("dockApi 缺失时不执行", () => {
    seedProject("C:/repo");
    window.__dockviewApi = undefined as unknown as typeof window.__dockviewApi;

    // 不应抛异常
    expect(() => openCommitFile("C:/repo/a.ts", "modified")).not.toThrow();
  });

  it("rootPath 缺失时直接返回（activePageId 无对应 page）", () => {
    // activePageId 有值，但 projects 中无匹配 page → rootPath 推导为 null
    useLayout.setState({ activePageId: "ghost-page" });
    const { addPanel: mockAddPanel } = mockDockApi();

    openCommitFile("C:/repo/a.ts", "modified");

    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("addPanel 抛异常时静默降级且不注册 titleManager", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn(() => {
      throw new Error("dockview layout error");
    });
    mockDockApi({ addPanel: mockAddPanel });

    // 不抛异常（addPanel 失败不影响调用方）
    expect(() => openCommitFile("C:/repo/a.ts", "modified")).not.toThrow();

    // titleManager 未被污染（registerEditor 在 addPanel 成功后执行）
    expect(titleManager.findExistingEditor("page-1", "C:/repo/a.ts", "(git diff)")).toBeNull();
  });

  it("addPanel 成功后 recomputeTitles 更新应用到面板标题", () => {
    seedProject("C:/repo");
    const mockSetTitle = vi.fn();
    // 单次 mockDockApi：addPanel 与 getPanel 须在同一窗口对象上（重复调用会替换 window.__dockviewApi）
    const { addPanel: mockAddPanel, getPanel: mockGetPanel } = mockDockApi({
      // getPanel 供 recomputeTitles 循环取面板 api
      getPanel: vi.fn(() => ({ api: { setTitle: mockSetTitle } })),
    });

    openCommitFile("C:/repo/a.ts", "modified");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const addedPanelId = mockAddPanel.mock.calls[0][0].id;
    // 新面板注册后 recomputeTitles 返回更新并执行 setTitle——参数为标题文本（单参，非 panelId）
    expect(mockSetTitle).toHaveBeenCalledTimes(1);
    expect(mockSetTitle.mock.calls[0][0]).toBe("a.ts(git diff)");
    // getPanel 以新增面板 id 查询——标题应用到新面板
    expect(mockGetPanel).toHaveBeenCalledWith(addedPanelId);
  });
});

describe("openCommitFile 去重聚焦", () => {
  it("已有同文件同 suffix 面板时聚焦而不创建", () => {
    seedProject("C:/repo");
    titleManager.registerEditor("page-1", "diff-1", "C:/repo/src/a.ts", "(git diff)");

    const mockFocus = vi.fn();
    const { addPanel: mockAddPanel } = mockDockApi({
      getPanel: vi.fn(() => ({ focus: mockFocus })),
    });

    openCommitFile("C:/repo/src/a.ts", "modified");

    expect(mockFocus).toHaveBeenCalled();
    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("无 suffix 的普通编辑器不被误匹配（B10 正向）", () => {
    seedProject("C:/repo");
    // 注册一个普通编辑器（无 suffix）
    titleManager.registerEditor("page-1", "editor-1", "C:/repo/src/a.ts");

    const { addPanel: mockAddPanel } = mockDockApi();

    // 打开 git diff 面板（带 suffix）→ 不应匹配无 suffix 的普通编辑器
    openCommitFile("C:/repo/src/a.ts", "modified");

    // 应创建新面板
    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
  });

  it("已打开 git 面板时不同 suffix 分派不误聚焦（B10 反向）", () => {
    seedProject("C:/repo");
    // 已打开一个 git diff 面板（suffix="(git diff)"）
    titleManager.registerEditor("page-1", "diff-1", "C:/repo/src/a.ts", "(git diff)");

    const mockFocus = vi.fn();
    const mockSetTitle = vi.fn();
    const { addPanel: mockAddPanel } = mockDockApi({
      // 命中去重时用 focus；未命中新建面板后 recomputeTitles 还需 api.setTitle
      getPanel: vi.fn(() => ({ focus: mockFocus, api: { setTitle: mockSetTitle } })),
    });

    // added 分派携带不同 suffix "(git add)" → 不匹配 → 新建 editor 面板而非聚焦 diff 面板
    openCommitFile("C:/repo/src/a.ts", "added");

    expect(mockFocus).not.toHaveBeenCalled();
    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("editor");
  });
});

describe("STATUS_PANEL_MAP 映射表", () => {
  it("所有契约状态均有映射", () => {
    expect(STATUS_PANEL_MAP.added.panelType).toBe("editor");
    expect(STATUS_PANEL_MAP.added.suffix).toBe("(git add)");
    expect(STATUS_PANEL_MAP.untracked.panelType).toBe("editor");
    expect(STATUS_PANEL_MAP.untracked.suffix).toBe("(git not add)");
    expect(STATUS_PANEL_MAP.deleted.panelType).toBe("gitshow");
    expect(STATUS_PANEL_MAP.deleted.suffix).toBe("(git delete)");
    expect(STATUS_PANEL_MAP.modified.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.modified.suffix).toBe("(git diff)");
    expect(STATUS_PANEL_MAP.renamed.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.renamed.suffix).toBe("(git diff)");
    expect(STATUS_PANEL_MAP.conflict.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.conflict.suffix).toBe("(git diff)");
  });

  it("未知状态返回 null", () => {
    expect(getPanelDispatch("ignored")).toBeNull();
    expect(getPanelDispatch("unknown")).toBeNull();
  });
});
