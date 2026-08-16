// agent-history-view.test.tsx — FE-07/08/09 历史区视图 L2 测试
//
// 覆盖：
//   1. AgentHistorySections 结构（搜索框/刷新按钮/两区块头）与默认折叠态
//   2. 历史区首次展开触发 scan()（仅首次；之后靠刷新按钮）
//   3. 搜索框输入过滤行（matchesSearch）+ 无结果提示
//   4. 全部项目区分组折叠（组默认收起——问题 3、组标题计数、展开/收起）
//   5. 空态四文案（该项目暂无历史会话 / 暂无历史会话 / 无活跃项目 / 无匹配的会话）
//   6. 右键菜单可用性矩阵（普通/孤儿/无 cwd/运行中 × 3 操作——重命名已移除，问题 7；
//      复制命令委托 profile.history.buildResumeCommand、分支恢复按 supportsFork 显隐——MC-316）
//   7. 双击分派三分支（普通 → 恢复；孤儿/无 cwd → 无操作；运行中 → 动作弹窗
//      「切换到该会话操作页面」/取消——问题 5，分支恢复仅右键菜单；
//      反查 = 复合键 cliId|sessionId 精确匹配——MC-313，键构造经 keyOf 单点
//      （cliId 缺省回退 + 转义——ZQ-1/ZQ-7））
//   8. AgentStatusView 三区集成（默认态、展开触发 scan、标题覆盖——问题 6，复合键 MC-314、E2E 红线）
//   9. 字号层级（区块标题 13px 粗体，问题 4）
//
// AgentHistorySections 为受控组件（useAgentHistory 上提至 AgentStatusView——问题 6），
// 测试直接注入受控 props；AgentStatusView 集成测试 mock useAgentStatus + useAgentHistory。
// Stage 05：side-effect import profiles 注册真实 claude profile（菜单/命令委托消费）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import {
  render,
  cleanup,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import { AgentHistorySections } from "../features/agentHistory/AgentHistorySections";
import type { AgentHistorySectionsProps } from "../features/agentHistory/AgentHistorySections";
import { AgentStatusView } from "../features/agentStatus/AgentStatusView";
import "../features/cliProfiles/profiles";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { claudeProfile } from "../features/cliProfiles/profiles/claude";
import {
  resetProjectStores,
  seedExplorerProject,
} from "./helpers/workspace-setup";
import type { AgentHistorySession } from "../types/agentHistory";
import type { AgentStatus } from "../lib/agentStatus";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => {
  const listeners = new Set<() => void>();
  const all = new Map<string, Record<string, unknown>>();
  return {
    listeners,
    all,
    mockGetAll: vi.fn(() => new Map(all)),
    mockSubscribe: vi.fn((cb: () => void) => {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    }),
    mockDeleteHistorySession: vi.fn(),
    mockAsk: vi.fn(),
    mockWriteText: vi.fn(),
    mockRestore: vi.fn(),
    mockUseAgentStatus: vi.fn(),
    mockUseAgentHistory: vi.fn(),
    mockScan: vi.fn(),
    mockRemoveLocal: vi.fn(),
    mockSwitchToPageAndFocus: vi.fn(),
    mockSendToastNotification: vi.fn(),
  };
});

// ── 模块级 mock ──

vi.mock("../ipc/agentHistory", () => ({
  deleteHistorySession: h.mockDeleteHistorySession,
}));

vi.mock("../ipc/dialog", () => ({
  ask: h.mockAsk,
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: h.mockWriteText,
}));

vi.mock("../ipc/notification", () => ({
  sendToastNotification: h.mockSendToastNotification,
}));

vi.mock("../features/agentHistory/restoreSession", () => ({
  restoreHistorySession: h.mockRestore,
}));

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: h.mockGetAll,
    subscribe: h.mockSubscribe,
  },
}));

vi.mock("../features/agentStatus/useAgentStatus", () => ({
  useAgentStatus: h.mockUseAgentStatus,
}));

vi.mock("../features/agentHistory/useAgentHistory", () => ({
  useAgentHistory: h.mockUseAgentHistory,
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageAndFocus: h.mockSwitchToPageAndFocus,
}));

// ── 辅助函数 ──

/** 最小 AgentHistorySession 工厂 */
function makeSession(
  id: string,
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: id,
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId: "claude",
    ...overrides,
  };
}

/** 常规（可恢复）会话：cwd 存在且目录存在 */
function normalSession(id: string, cwd: string, overrides: Partial<AgentHistorySession> = {}) {
  return makeSession(id, { cwd, cwdExists: true, ...overrides });
}

/** 受控 props 工厂（useAgentHistory 上提后 AgentHistorySections 为纯受控） */
function makeSectionsProps(
  overrides: Partial<AgentHistorySectionsProps> = {},
): AgentHistorySectionsProps {
  return {
    expandedCurrent: false,
    expandedAll: false,
    onToggleCurrent: vi.fn(),
    onToggleAll: vi.fn(),
    historyState: "idle",
    sessions: [],
    activeStatuses: new Map<string, AgentStatus>(),
    rootPath: null,
    scan: h.mockScan,
    removeLocal: h.mockRemoveLocal,
    ...overrides,
  };
}

beforeEach(() => {
  resetProjectStores();
  h.listeners.clear();
  h.all.clear();
  h.mockGetAll.mockClear();
  h.mockSubscribe.mockClear();
  h.mockDeleteHistorySession.mockReset();
  h.mockDeleteHistorySession.mockResolvedValue(undefined);
  h.mockAsk.mockReset();
  h.mockAsk.mockResolvedValue(true);
  h.mockWriteText.mockReset();
  h.mockRestore.mockReset();
  h.mockRestore.mockResolvedValue(undefined);
  h.mockUseAgentStatus.mockReset();
  h.mockUseAgentHistory.mockReset();
  h.mockScan.mockReset();
  h.mockRemoveLocal.mockReset();
  h.mockSwitchToPageAndFocus.mockReset();
  h.mockSwitchToPageAndFocus.mockResolvedValue(undefined);
  h.mockSendToastNotification.mockReset();
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  // 还原 profile 注册表（supportsFork 显隐用例局部覆写后复原，照 cli-profile 测试先例）
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
});

// ═══════════════════════════════════════════════════════════════
// AgentHistorySections 结构与默认态
// ═══════════════════════════════════════════════════════════════

describe("AgentHistorySections 结构与默认态", () => {
  it("渲染搜索框 + 刷新按钮 + 两个区块头（位于搜索框之下）", () => {
    seedExplorerProject("C:/project");
    const { container, getByText } = render(
      React.createElement(AgentHistorySections, makeSectionsProps()),
    );

    expect(
      container.querySelector('[data-e2e="agent-history-search"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-refresh"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-section-current"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-section-all"]'),
    ).toBeTruthy();
    expect(getByText("当前项目历史会话")).toBeTruthy();
    expect(getByText("全部项目历史会话")).toBeTruthy();

    // 搜索框位于两个区块之上（DOM 顺序）
    const search = container.querySelector('[data-e2e="agent-history-search"]')!;
    const current = container.querySelector(
      '[data-e2e="agent-history-section-current"]',
    )!;
    const all = container.querySelector('[data-e2e="agent-history-section-all"]')!;
    expect(search.compareDocumentPosition(current) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(current.compareDocumentPosition(all) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("区块标题 13px 粗体（问题 4：折叠框名层级 > 行标题 12px）", () => {
    seedExplorerProject("C:/project");
    const { getByText } = render(
      React.createElement(AgentHistorySections, makeSectionsProps()),
    );
    for (const label of ["当前项目历史会话", "全部项目历史会话"]) {
      const el = getByText(label).parentElement as HTMLElement;
      expect(el.style.fontSize).toBe("13px");
      expect(el.style.fontWeight).toBe("bold");
    }
  });

  it("默认两历史区收起：不渲染行、不触发 scan", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          sessions: [normalSession("session-1", "C:/project/src")],
        }),
      ),
    );

    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
    expect(h.mockScan).not.toHaveBeenCalled();
  });

  it("点击刷新按钮触发 scan()", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(AgentHistorySections, makeSectionsProps()),
    );

    const refresh = container.querySelector(
      '[data-e2e="agent-history-refresh"]',
    ) as HTMLElement;
    fireEvent.click(refresh);

    expect(h.mockScan).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════
// 展开触发 scan（仅首次）
// ═══════════════════════════════════════════════════════════════

describe("历史区首次展开触发 scan", () => {
  it("展开当前项目区 → scan 一次；收起再展开另一区 → 不重复 scan", async () => {
    seedExplorerProject("C:/project");
    const props = makeSectionsProps({
      sessions: [normalSession("session-1", "C:/project/src")],
      rootPath: "C:/project/src",
    });

    const { rerender } = render(
      React.createElement(AgentHistorySections, props),
    );
    expect(h.mockScan).not.toHaveBeenCalled();

    // 首次展开当前项目区 → scan 一次，行渲染
    rerender(
      React.createElement(AgentHistorySections, {
        ...props,
        expandedCurrent: true,
      }),
    );
    expect(h.mockScan).toHaveBeenCalledTimes(1);
    expect(
      document.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(1);

    // 收起当前区再展开全部区 → 不重复 scan（仅首次）
    rerender(
      React.createElement(AgentHistorySections, {
        ...props,
        expandedCurrent: false,
      }),
    );
    rerender(
      React.createElement(AgentHistorySections, {
        ...props,
        expandedAll: true,
      }),
    );
    expect(h.mockScan).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════
// 搜索过滤
// ═══════════════════════════════════════════════════════════════

describe("搜索框过滤", () => {
  it("输入关键词过滤当前展开的列表（标题/首条 prompt，大小写不敏感）", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [
            normalSession("session-1", "C:/project/src", {
              title: "修复登录 bug",
              firstPrompt: "排查 token 过期问题",
            }),
            normalSession("session-2", "C:/project/src", {
              title: "重构界面",
              firstPrompt: "调整布局",
            }),
          ],
        }),
      ),
    );

    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);

    const search = container.querySelector(
      '[data-e2e="agent-history-search"]',
    ) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "登录" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(1);

    fireEvent.change(search, { target: { value: "TOKEN" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(1);

    fireEvent.change(search, { target: { value: "" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);
  });
});

// ═══════════════════════════════════════════════════════════════
// 全部项目区分组折叠（问题 3：组默认收起 + 计数）
// ═══════════════════════════════════════════════════════════════

describe("全部项目区分组折叠", () => {
  it("组默认收起：组标题可见（含 (N) 计数）、组内行不可见；点击展开/收起", async () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedAll: true,
          sessions: [
            normalSession("s1", "D:/a/projA", { mtimeMs: 4000 }),
            normalSession("s2", "D:/a/projA", { mtimeMs: 3000 }),
            normalSession("s3", "D:/b/projB", { mtimeMs: 2000 }),
            makeSession("s4", { cwd: null, mtimeMs: 1000 }),
          ],
        }),
      ),
    );

    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-group"]').length,
      ).toBe(3);
    });

    // 组默认收起：组内行全部不可见
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);

    // 组标题 = basename + (N) 计数；title 悬停完整路径；未知目录组带计数
    const groups = Array.from(
      container.querySelectorAll('[data-e2e="agent-history-group"]'),
    );
    expect(groups[0].textContent).toContain("projA");
    expect(groups[0].textContent).toContain("(2)");
    expect(groups[0].getAttribute("title")).toBe("D:/a/projA");
    expect(groups[1].textContent).toContain("(1)");
    expect(groups[2].textContent).toContain("(未知目录)");
    expect(groups[2].textContent).toContain("(1)");
    expect(groups[2].getAttribute("title")).toBeNull();

    // NAH-09①：expandedGroups 初始为空（白名单模型）→ 组标题箭头为收起态
    // （lucide ChevronRight path d="m9 18 6-6-6-6"，IC-05 chevron 化）
    expect(
      groups[0].querySelector("svg path")?.getAttribute("d"),
    ).toContain("m9 18 6-6-6-6");

    // 点击第一组展开 → 该组 key 加入 expandedGroups（箭头 ChevronDown）+ 该组 2 行渲染，其他组仍收起
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]')[0]
        .querySelector("svg path")
        ?.getAttribute("d"),
    ).toContain("m6 9 6 6 6-6");
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);

    // 再点收起 → 该组 key 移除（箭头 ChevronRight）→ 恢复 0 行
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]')[0]
        .querySelector("svg path")
        ?.getAttribute("d"),
    ).toContain("m9 18 6-6-6-6");
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
  });
});

// ═══════════════════════════════════════════════════════════════
// 空态四文案（FE-09）
// ═══════════════════════════════════════════════════════════════

describe("空态与提示文案", () => {
  it("当前项目无历史会话 →「该项目暂无历史会话」", () => {
    seedExplorerProject("C:/project");
    const { getByText, container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("s1", "D:/other", { title: "其他项目会话" })],
        }),
      ),
    );
    expect(getByText("该项目暂无历史会话")).toBeTruthy();
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
  });

  it("全部项目无任何会话 →「暂无历史会话」", () => {
    seedExplorerProject("C:/project");
    const { getByText, container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({ expandedAll: true }),
      ),
    );
    expect(getByText("暂无历史会话")).toBeTruthy();
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]').length,
    ).toBe(0);
  });

  it("无活跃项目（rootPath null）→ 当前项目区显示「无活跃项目」", () => {
    const { getByText } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({ expandedCurrent: true, rootPath: null }),
      ),
    );
    expect(getByText("无活跃项目")).toBeTruthy();
  });

  it("搜索无结果 →「无匹配的会话」提示", () => {
    seedExplorerProject("C:/project");
    const { container, getByText } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("s1", "C:/project/src", { title: "修复登录" })],
        }),
      ),
    );

    const search = container.querySelector(
      '[data-e2e="agent-history-search"]',
    ) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "不存在的关键词" } });
    expect(getByText("无匹配的会话")).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 右键菜单可用性矩阵（普通/孤儿/无 cwd/运行中 × 3 操作——重命名已移除，问题 7）
// ═══════════════════════════════════════════════════════════════

describe("右键菜单可用性矩阵", () => {
  /** 渲染全部区（组展开）并返回「第一行」元素 */
  function renderAllAndGetFirstRow(
    sessions: AgentHistorySession[],
    activeStatuses: Map<string, AgentStatus> = new Map(),
  ) {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedAll: true,
          sessions,
          activeStatuses,
        }),
      ),
    );
    // 展开第一个组（默认收起）
    const group = container.querySelector('[data-e2e="agent-history-group"]') as HTMLElement;
    fireEvent.click(group);
    return container.querySelector('[data-e2e="agent-history-row"]') as HTMLElement;
  }

  /** 右键打开第一行的菜单，返回 { container, labels } */
  function openMenu(row: HTMLElement) {
    fireEvent.contextMenu(row);
    const container = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    ) as HTMLElement;
    expect(container).toBeTruthy();
    const labels = Array.from(container.querySelectorAll("div")).map(
      (d) => d.textContent ?? "",
    );
    return { container, labels };
  }

  it("普通行：三项操作全部可用（复制/分支恢复/删除），无重命名项", () => {
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    const { labels } = openMenu(row);

    expect(labels).toContain("复制恢复命令");
    expect(labels).toContain("分支恢复");
    expect(labels).toContain("删除");
    expect(labels).not.toContain("重命名");
  });

  it("supportsFork=false（能力未声明）→ 不展示「分支恢复」菜单项（MC-316 显隐）", () => {
    // 用例内局部注册测试 profile：覆写 claude profile 的 history 能力 supportsFork=false
    // （afterEach _reset + 重注册真实 claudeProfile 复原——照 cli-profile 测试先例）
    cliProfileRegistry.register({
      ...claudeProfile,
      capabilities: {
        ...claudeProfile.capabilities,
        history: { ...claudeProfile.capabilities!.history!, supportsFork: false },
      },
    });
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    const { labels } = openMenu(row);

    expect(labels).toContain("复制恢复命令");
    expect(labels).not.toContain("分支恢复");
    expect(labels).toContain("删除");
  });

  it("复制恢复命令：写入剪贴板，格式 = cd '<cwd>' && claude --resume <id>", () => {
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    openMenu(row);

    fireEvent.click(document.body.querySelector('[data-e2e="agent-history-menu"]')!.children[0]!);
    expect(h.mockWriteText).toHaveBeenCalledWith(
      "cd 'D:/a/projA' && claude --resume session-1",
    );
  });

  it("复制恢复命令：无 cwd 行 → 仅 claude --resume <id>", () => {
    const row = renderAllAndGetFirstRow([
      makeSession("session-1", { title: "无 cwd 会话" }),
    ]);
    openMenu(row);

    fireEvent.click(document.body.querySelector('[data-e2e="agent-history-menu"]')!.children[0]!);
    expect(h.mockWriteText).toHaveBeenCalledWith("claude --resume session-1");
  });

  it("分支恢复：普通行可用 → restoreHistorySession(session, { fork: true })", () => {
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    // items[0]=复制 items[1]=分支恢复 items[2]=删除
    fireEvent.click(items[1]!);
    expect(h.mockRestore).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: "session-1" }),
      { fork: true },
    );
  });

  it("孤儿行：分支恢复禁用（点击无效果）", () => {
    const row = renderAllAndGetFirstRow([
      makeSession("session-1", { cwd: "D:/gone", cwdExists: false, title: "孤儿会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[1]!);
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("无 cwd 行：分支恢复禁用", () => {
    const row = renderAllAndGetFirstRow([
      makeSession("session-1", { cwd: null, title: "无 cwd 会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[1]!);
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("运行中行：删除禁用（activeStatuses 含 session-1）", () => {
    const row = renderAllAndGetFirstRow(
      [normalSession("session-1", "D:/a/projA", { title: "运行中会话" })],
      new Map([["claude|session-1", "attention"]]),
    );
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[2]!); // 删除（禁用项无 onClick）
    expect(h.mockAsk).not.toHaveBeenCalled();
    expect(h.mockDeleteHistorySession).not.toHaveBeenCalled();
  });

  it("删除：ask 确认 → deleteHistorySession → removeLocal 即时移除行", async () => {
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "待删会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[2]!);

    await waitFor(() => {
      expect(h.mockAsk).toHaveBeenCalledWith(
        '确定删除会话"待删会话"？此操作不可撤销。',
        expect.any(Object),
      );
      expect(h.mockDeleteHistorySession).toHaveBeenCalledWith(
        "claude",
        "session-1",
      );
    });
    await waitFor(() => {
      expect(h.mockRemoveLocal).toHaveBeenCalledWith("session-1");
    });
  });

  it("删除：ask 取消 → 不删除", async () => {
    h.mockAsk.mockResolvedValue(false);
    const row = renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "待删会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[2]!);

    await waitFor(() => {
      expect(h.mockAsk).toHaveBeenCalled();
    });
    expect(h.mockDeleteHistorySession).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════
// 双击分派三分支（问题 5：运行中 → 动作弹窗）
// ═══════════════════════════════════════════════════════════════

describe("双击分派三分支", () => {
  it("普通行双击 → restoreHistorySession(session)", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "普通会话" })],
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: "session-1" }),
    );
  });

  it("孤儿行双击 → 无操作（不恢复、不开弹窗）", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedAll: true,
          sessions: [
            makeSession("session-1", { cwd: "C:/gone", cwdExists: false, title: "孤儿会话" }),
          ],
        }),
      ),
    );

    const group = container.querySelector('[data-e2e="agent-history-group"]') as HTMLElement;
    fireEvent.click(group); // 展开组
    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).not.toHaveBeenCalled();
    expect(
      document.querySelector('[data-e2e="agent-history-action-dialog"]'),
    ).toBeNull();
  });

  it("无 cwd 行双击 → 无操作", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedAll: true,
          sessions: [makeSession("session-1", { cwd: null, title: "无 cwd 会话" })],
        }),
      ),
    );

    const group = container.querySelector('[data-e2e="agent-history-group"]') as HTMLElement;
    fireEvent.click(group);
    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("运行中行双击 → 动作弹窗打开（含「切换到该会话操作页面」，无分支恢复）", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    // 弹窗打开（非 ask 弹窗）；无分支恢复项
    const dialog = document.querySelector(
      '[data-e2e="agent-history-action-dialog"]',
    );
    expect(dialog).toBeTruthy();
    expect(dialog!.textContent).toContain("切换到该会话操作页面");
    expect(dialog!.textContent).not.toContain("分支恢复");
    expect(h.mockAsk).not.toHaveBeenCalled();
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("点击「切换到该会话操作页面」→ 反查 panelId → 切页 + 聚焦终端页签", async () => {
    seedExplorerProject("C:/project");
    // TerminalRegistry 含 session-1 的终端（panelId → pageId 可解析）
    h.all.set("terminal-page1-0", {
      agentSession: { sessionId: "session-1", status: "attention" },
    });
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const switchBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "切换到该会话操作页面") as HTMLElement;
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(h.mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
      );
    });
  });

  it("B14: 旧恢复格式 panelId（含 Date.now 段）→ 前缀匹配命中真实页面而非幽灵页面", async () => {
    seedExplorerProject("C:/project");
    // 种子页面 pageId = "page-1"（含连字符）。旧格式 panelId 经 parseTerminalPageId
    // 会切分出 "page-1-1700000000000"（幽灵页面，导航后主区空白根因）——
    // 前缀匹配应命中已知页面 "page-1"
    h.all.set("terminal-page-1-1700000000000-1", {
      agentSession: { sessionId: "session-1", status: "attention" },
    });
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const switchBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "切换到该会话操作页面") as HTMLElement;
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(h.mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page-1",
        "terminal-page-1-1700000000000-1",
      );
    });
  });

  it("同 sessionId 不同 cliId → 反查命中本 CLI 所在面板（复合键 cliId|sessionId，MC-313）", async () => {
    seedExplorerProject("C:/project");
    // 注册表两个面板共享 sessionId "dup-1" 但 cliId 不同——claude 会话行必须命中
    // claude 面板（terminal-page1-0），不得被 mockcli 面板（terminal-page2-0）抢走
    h.all.set("terminal-page1-0", {
      agentSession: { cliId: "claude", sessionId: "dup-1", status: "attention" },
    });
    h.all.set("terminal-page2-0", {
      agentSession: { cliId: "mockcli", sessionId: "dup-1", status: "attention" },
    });
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [
            normalSession("dup-1", "C:/project/src", { title: "运行中会话" }),
          ],
          activeStatuses: new Map([["claude|dup-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const switchBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "切换到该会话操作页面") as HTMLElement;
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(h.mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
      );
    });
  });

  it("反查不到 panelId（会话已结束）→ toast 提示，不切页", async () => {
    seedExplorerProject("C:/project");
    // TerminalRegistry 无 session-1 条目
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const switchBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "切换到该会话操作页面") as HTMLElement;
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(h.mockSendToastNotification).toHaveBeenCalled();
    });
    expect(h.mockSwitchToPageAndFocus).not.toHaveBeenCalled();
  });

  it("取消按钮关闭弹窗（无切换动作）", () => {
    seedExplorerProject("C:/project");
    render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      document.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const cancelBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "取消") as HTMLElement;
    fireEvent.click(cancelBtn);

    expect(
      document.querySelector('[data-e2e="agent-history-action-dialog"]'),
    ).toBeNull();
    expect(h.mockSwitchToPageAndFocus).not.toHaveBeenCalled();
  });

  it("消费方回退：session.cliId 为 null（旧数据形态）→ rowFlags 经 keyOf 命中 CLAUDE_CLI_ID 键（ZQ-1）", () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [
            makeSession("session-1", {
              cliId: null as unknown as string, // 旧数据形态（类型上 string，运行时缺省）
              cwd: "C:/project/src",
              cwdExists: true,
              title: "旧数据会话",
            }),
          ],
          // 生产侧键 = keyOf(claude, session-1) = "claude|session-1"
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    // 运行中判定命中（消费方缺省回退生效）→ 动作弹窗而非恢复
    expect(
      document.querySelector('[data-e2e="agent-history-action-dialog"]'),
    ).toBeTruthy();
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("findPanelForSession 两侧回退：会话与注册表条目均无 cliId → 经 keyOf 归一后命中（ZQ-1）", async () => {
    seedExplorerProject("C:/project");
    // 注册表条目无 cliId（旧数据），会话 cliId 亦为 null——两侧键都回退到 "claude|session-1"
    h.all.set("terminal-page1-0", {
      agentSession: { sessionId: "session-1", status: "attention" },
    });
    const { container } = render(
      React.createElement(
        AgentHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [
            makeSession("session-1", {
              cliId: null as unknown as string,
              cwd: "C:/project/src",
              cwdExists: true,
              title: "运行中会话",
            }),
          ],
          activeStatuses: new Map([["claude|session-1", "attention"]]),
        }),
      ),
    );

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    const switchBtn = Array.from(
      document.querySelectorAll('[data-e2e="agent-history-action-dialog"] button'),
    ).find((b) => b.textContent === "切换到该会话操作页面") as HTMLElement;
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(h.mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
      );
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// AgentStatusView 三区集成（FE-08 + 问题 6 标题覆盖）
// ═══════════════════════════════════════════════════════════════

describe("AgentStatusView 三区集成", () => {
  const defaultAgentStatus = {
    state: { kind: "ready" as const },
    rows: [
      {
        panelId: "terminal-page1-0",
        pageId: "page1",
        projectId: "proj-1",
        cliId: "claude",
        sessionId: "session-1",
        title: "claude",
        status: "attention" as AgentStatus,
        lastEventAt: Date.now(),
        usage: undefined,
      },
    ],
    currentProjectName: "测试项目",
  };

  const defaultHistory = {
    state: "idle" as const,
    sessions: [] as AgentHistorySession[],
    activeStatuses: new Map<string, AgentStatus>(),
    rootPath: null,
    scan: h.mockScan,
    removeLocal: h.mockRemoveLocal,
  };

  beforeEach(() => {
    h.mockUseAgentStatus.mockReturnValue(defaultAgentStatus);
    h.mockUseAgentHistory.mockReturnValue(defaultHistory);
  });

  it("默认态：活跃展开（行可见）、两历史区收起（无历史行、不触发 scan）", () => {
    seedExplorerProject("C:/project");
    const { container, getByText } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );

    // E2E 红线：根容器 + 活跃行 + 标题栏
    expect(
      container.querySelector('[data-e2e="agent-status-view"]'),
    ).toBeTruthy();
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(1);
    expect(getByText("AGENT STATUS")).toBeTruthy();

    // 三区块头
    expect(getByText("活跃会话")).toBeTruthy();
    expect(getByText("当前项目历史会话")).toBeTruthy();
    expect(getByText("全部项目历史会话")).toBeTruthy();

    // 历史区收起：无历史行、scan 未触发
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
    expect(h.mockScan).not.toHaveBeenCalled();
  });

  it("点击历史区标题展开 → 触发 scan()；再次收起展开不重复", () => {
    seedExplorerProject("C:/project");
    const { getByText } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );

    fireEvent.click(getByText("当前项目历史会话"));
    expect(h.mockScan).toHaveBeenCalledTimes(1);

    fireEvent.click(getByText("当前项目历史会话"));
    fireEvent.click(getByText("全部项目历史会话"));
    expect(h.mockScan).toHaveBeenCalledTimes(1);
  });

  it("活跃区标题覆盖：历史区 scan 数据中同 sessionId 标题覆盖行标题（问题 6）", () => {
    seedExplorerProject("C:/project");
    // 活跃区行标题为「claude」，历史区 scan 结果同 sessionId 标题为「新标题」→ 显示新标题
    h.mockUseAgentHistory.mockReturnValue({
      ...defaultHistory,
      sessions: [
        normalSession("session-1", "C:/project/src", {
          title: "重命名后的标题",
        }),
      ],
    });
    const { container } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );

    const row = container.querySelector(
      '[data-e2e="agent-status-row"]',
    ) as HTMLElement;
    expect(row.textContent).toContain("重命名后的标题");
    expect(row.textContent).not.toContain("claude");
  });

  it("标题覆盖回退：无匹配 sessionId 或无标题 → 显示行原标题", () => {
    seedExplorerProject("C:/project");
    // 无匹配（sessions 空）
    const { container, rerender } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );
    expect(
      (container.querySelector('[data-e2e="agent-status-row"]') as HTMLElement)
        .textContent,
    ).toContain("claude");

    // sessions 有同 sessionId 但 title 为 null → 不覆盖
    h.mockUseAgentHistory.mockReturnValue({
      ...defaultHistory,
      sessions: [makeSession("session-1", { title: null })],
    });
    rerender(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );
    expect(
      (container.querySelector('[data-e2e="agent-status-row"]') as HTMLElement)
        .textContent,
    ).toContain("claude");
  });

  it("区块标题 13px 粗体 + 活跃区内容缩进引导线（问题 4）", () => {
    seedExplorerProject("C:/project");
    const { getByText, container } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );

    for (const label of ["活跃会话", "当前项目历史会话", "全部项目历史会话"]) {
      const el = getByText(label).parentElement as HTMLElement;
      expect(el.style.fontSize).toBe("13px");
      expect(el.style.fontWeight).toBe("bold");
    }

    // 活跃区内容容器：borderLeft 引导线 + 12px 缩进
    // DOM 链：row > listContainer(2px 0) > sectionBody(引导线容器)
    const row = container.querySelector(
      '[data-e2e="agent-status-row"]',
    ) as HTMLElement;
    const body = row.parentElement!.parentElement as HTMLElement;
    expect(body.style.paddingLeft).toBe("12px");
    expect(body.style.borderLeft).toContain("1px solid");
  });

  it("点击活跃会话标题收起 → 活跃行隐藏", () => {
    seedExplorerProject("C:/project");
    const { getByText, container } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(1);

    fireEvent.click(getByText("活跃会话"));
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(0);
  });
});
