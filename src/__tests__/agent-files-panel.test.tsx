// agent-files-panel.test.tsx — 「Agent 全局文件」侧栏视图测试
//
// 覆盖：agent 节点渲染（仅 globalFiles 能力者）/ 展开惰性加载 + pinned watch 生命周期 /
// 目录不存在占位 / 无 agent 空态 / 双击打开守卫（无项目 toast / 有项目 openFileInPage
// rootPath:null）/ store 配置过滤联动 / viewState 槽位回填与上呼。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";
import type { CodingCliProfile } from "../features/cliProfiles/types";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockListAgentDirs = vi.fn();
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn().mockResolvedValue([]);
  const mockStartWatch = vi.fn().mockResolvedValue(undefined);
  const mockStopWatch = vi.fn().mockResolvedValue(undefined);
  const mockOpenFileInPage = vi.fn().mockReturnValue(true);
  const mockToastShow = vi.fn();

  const CLAUDE_DIR = "C:/Users/test/.claude";
  const rootEntries = [
    { name: "settings.json", path: `${CLAUDE_DIR}/settings.json`, isDir: false, size: 100, modified: 1 },
    { name: "projects", path: `${CLAUDE_DIR}/projects`, isDir: true, size: null, modified: null },
    { name: "agents", path: `${CLAUDE_DIR}/agents`, isDir: true, size: null, modified: null },
  ];

  return {
    mockListAgentDirs,
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockStopWatch,
    mockOpenFileInPage,
    mockToastShow,
    CLAUDE_DIR,
    rootEntries,
    resetAll() {
      mockListAgentDirs.mockReset();
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockStopWatch.mockReset();
      mockOpenFileInPage.mockReset();
      mockToastShow.mockReset();
      mockListAgentDirs.mockResolvedValue([
        { cliId: "claude", path: CLAUDE_DIR, exists: true },
      ]);
      mockReadDir.mockResolvedValue({ entries: rootEntries, nextCursor: null });
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
      mockStopWatch.mockResolvedValue(undefined);
      mockOpenFileInPage.mockReturnValue(true);
    },
  };
});

vi.mock("../ipc/agentDirs", () => ({
  listAgentDirs: mocks.mockListAgentDirs,
}));

vi.mock("../ipc/fs", () => ({
  readDirPage: mocks.mockReadDir,
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  stopWatch: mocks.mockStopWatch,
  onFsEvent: () => () => {},
}));

// openFileInPage spy——断言项目外打开形态（rootPath/projectRootPath 均 null）
vi.mock("../workspace/openFile", () => ({
  openFileInPage: mocks.mockOpenFileInPage,
  canOpenFile: () => true,
}));

vi.mock("../lib", () => ({
  toast: { show: mocks.mockToastShow, _reset: vi.fn() },
  getErrorMessage: (err: unknown) => String(err),
}));

import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { useAgentGlobalFiles } from "../stores/agentGlobalFiles";
import { useLayout } from "../stores/layout";
import { AgentFilesPanel } from "../features/agentFiles/AgentFilesPanel";

/** fake profile：globalFiles 能力可选 */
function fakeProfile(id: string, withGlobalFiles = true): CodingCliProfile {
  return {
    id,
    displayName: `${id}-display`,
    commands: [id],
    iconSrc: `/cli-icons/${id}.png`,
    tabTitle: id,
    capabilities: withGlobalFiles
      ? { globalFiles: { configDir: `.${id}`, runtimePaths: ["projects", "sessions"] } }
      : {},
  };
}

/** data-e2e 查询（无 testIdAttribute 配置；container 作用域——本配置无 globals，
 *  RTL 自动 cleanup 不生效，document 全局查会命中前例残留 DOM） */
async function findByE2e(container: HTMLElement, e2e: string): Promise<HTMLElement> {
  let el: HTMLElement | null = null;
  await waitFor(() => {
    el = container.querySelector(`[data-e2e="${e2e}"]`);
    expect(el).not.toBeNull();
  });
  return el!;
}

/** 渲染面板并等 agent_dirs_list 落定（节点出现） */
async function renderPanel(viewState?: unknown) {
  const onViewStateChange = vi.fn();
  const utils = render(
    React.createElement(AgentFilesPanel, {
      switchToPage: vi.fn(),
      onDeletePage: vi.fn(),
      viewState,
      onViewStateChange,
    }),
  );
  await waitFor(() => {
    expect(mocks.mockListAgentDirs).toHaveBeenCalled();
  });
  return { ...utils, onViewStateChange };
}

describe("AgentFilesPanel", () => {
  beforeEach(() => {
    mocks.resetAll();
    cliProfileRegistry._reset();
    cliProfileRegistry.register(fakeProfile("claude"));
    useAgentGlobalFiles.setState({ configs: {}, loaded: true });
    useLayout.setState({ activePageId: null } as never);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__dockviewApi;
  });

  afterEach(() => {
    cleanup(); // 无 globals 配置，手动卸载防 DOM/订阅残留（本文件用例间污染实证）
    cliProfileRegistry._reset();
  });

  it("渲染 agent 节点（displayName）；无 globalFiles 能力的 profile 不参与", async () => {
    cliProfileRegistry.register(fakeProfile("codex", false));
    const { findByText, queryByText } = await renderPanel();
    expect(await findByText("claude-display")).toBeTruthy();
    expect(queryByText("codex-display")).toBeNull();
  });

  it("展开 agent → 惰性加载目录 + startWatch pinned；折叠 → stopWatch", async () => {
    const { findByText, queryByText, container } = await renderPanel();
    const node = await findByE2e(container, "agent-files-node-claude");

    fireEvent.click(node);
    // 树渲染（readDirPage 消费目录）+ pinned watch 启动
    expect(await findByText("settings.json")).toBeTruthy();
    expect(mocks.mockReadDir).toHaveBeenCalledWith(mocks.CLAUDE_DIR);
    expect(mocks.mockStartWatch).toHaveBeenCalledWith(mocks.CLAUDE_DIR, { pinned: true });

    fireEvent.click(node);
    await waitFor(() => {
      expect(queryByText("settings.json")).toBeNull();
    });
    expect(mocks.mockStopWatch).toHaveBeenCalledWith(mocks.CLAUDE_DIR);
  });

  it("目录不存在（exists=false）→ 展开显示占位，不启动监听", async () => {
    mocks.mockListAgentDirs.mockResolvedValue([
      { cliId: "claude", path: mocks.CLAUDE_DIR, exists: false },
    ]);
    const { queryByText, container } = await renderPanel();
    fireEvent.click(await findByE2e(container, "agent-files-node-claude"));

    expect(await findByE2e(container, "agent-files-missing-claude")).toBeTruthy();
    expect(queryByText("settings.json")).toBeNull();
    expect(mocks.mockStartWatch).not.toHaveBeenCalled();
  });

  it("无声明 globalFiles 能力的 agent → 空态", async () => {
    cliProfileRegistry._reset();
    const { findByText } = await renderPanel();
    expect(await findByText("无可用 Agent")).toBeTruthy();
  });

  it("双击文件：无活跃页面/无宿主 → toast「请先创建项目」，不打开", async () => {
    const { findByText, container } = await renderPanel();
    fireEvent.click(await findByE2e(container, "agent-files-node-claude"));
    const item = await findByText("settings.json");

    fireEvent.doubleClick(item);
    expect(mocks.mockToastShow).toHaveBeenCalledWith("warning", "请先创建项目");
    expect(mocks.mockOpenFileInPage).not.toHaveBeenCalled();
  });

  it("双击文件：有活跃页面 + 宿主 → openFileInPage 项目外形态（rootPath/projectRootPath 均 null）", async () => {
    useLayout.setState({ activePageId: "page-1" } as never);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__dockviewApi = { addPanel: vi.fn() };

    const { findByText, container } = await renderPanel();
    fireEvent.click(await findByE2e(container, "agent-files-node-claude"));
    const item = await findByText("settings.json");

    fireEvent.doubleClick(item);
    expect(mocks.mockOpenFileInPage).toHaveBeenCalledWith(
      {
        activePageId: "page-1",
        dockApi: (window as unknown as { __dockviewApi: unknown }).__dockviewApi,
        rootPath: null,
        projectRootPath: null,
      },
      `${mocks.CLAUDE_DIR}/settings.json`,
    );
    expect(mocks.mockToastShow).not.toHaveBeenCalled();
  });

  it("store 配置联动：custom 模式 → 根层只显示名单项；缺省 all 模式排除运行时项", async () => {
    // 缺省（all + 不显示运行时）：projects 命中 runtimePaths → 不展示
    const { findByText, queryByText, unmount, container } = await renderPanel();
    fireEvent.click(await findByE2e(container, "agent-files-node-claude"));
    expect(await findByText("settings.json")).toBeTruthy();
    expect(await findByText("agents")).toBeTruthy();
    expect(queryByText("projects")).toBeNull();
    unmount();

    // custom 模式只留 agents → settings.json 也不展示
    useAgentGlobalFiles.setState({
      configs: {
        claude: { mode: "custom", customNames: ["agents"], showRuntimeFiles: false },
      },
    });
    const panel2 = await renderPanel();
    fireEvent.click(await findByE2e(panel2.container, "agent-files-node-claude"));
    expect(await panel2.findByText("agents")).toBeTruthy();
    expect(panel2.queryByText("settings.json")).toBeNull();
    expect(panel2.queryByText("projects")).toBeNull();
  });

  it("viewState 槽位：expandedAgents 回填即展开；toggle 上呼合并快照", async () => {
    const viewState = {
      expandedAgents: ["claude"],
      trees: {},
    };
    const { findByText, onViewStateChange, container } = await renderPanel(viewState);
    // 回填 → 挂载即展开（树直接渲染）
    expect(await findByText("settings.json")).toBeTruthy();

    // 折叠 → 上呼 { expandedAgents: [], trees }
    fireEvent.click(await findByE2e(container, "agent-files-node-claude"));
    await waitFor(() => {
      expect(onViewStateChange).toHaveBeenCalledWith(
        expect.objectContaining({ expandedAgents: [] }),
      );
    });
  });
});
