// claude-history-view.test.tsx — FE-07/08/09 历史区视图 L2 测试
//
// 覆盖：
//   1. ClaudeHistorySections 结构（搜索框/刷新按钮/两区块头）与默认折叠态
//   2. 历史区首次展开触发 scan()（仅首次；之后靠刷新按钮）
//   3. 搜索框输入过滤行（matchesSearch）+ 无结果提示
//   4. 全部项目区分组折叠（组默认收起——问题 3、组标题计数、展开/收起）
//   5. 空态四文案（该项目暂无历史会话 / 暂无历史会话 / 无活跃项目 / 无匹配的会话）
//   6. 右键菜单可用性矩阵（普通/孤儿/无 cwd/运行中 × 3 操作——重命名已移除，问题 7）
//   7. 双击分派三分支（普通 → 恢复；孤儿/无 cwd → 无操作；运行中 → 动作弹窗
//      「切换到该会话操作页面」/取消——问题 5，分支恢复仅右键菜单）
//   8. AgentStatusView 三区集成（默认态、展开触发 scan、标题覆盖——问题 6、E2E 红线）
//   9. 字号层级（区块标题 13px 粗体，问题 4）
//
// ClaudeHistorySections 为受控组件（useClaudeHistory 上提至 AgentStatusView——问题 6），
// 测试直接注入受控 props；AgentStatusView 集成测试 mock useAgentStatus + useClaudeHistory。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import {
  render,
  cleanup,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import { ClaudeHistorySections } from "../features/claudeHistory/ClaudeHistorySections";
import type { ClaudeHistorySectionsProps } from "../features/claudeHistory/ClaudeHistorySections";
import { AgentStatusView } from "../features/agentStatus/AgentStatusView";
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
    mockUseClaudeHistory: vi.fn(),
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

vi.mock("../features/claudeHistory/restoreSession", () => ({
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

vi.mock("../features/claudeHistory/useClaudeHistory", () => ({
  useClaudeHistory: h.mockUseClaudeHistory,
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

/** 受控 props 工厂（useClaudeHistory 上提后 ClaudeHistorySections 为纯受控） */
function makeSectionsProps(
  overrides: Partial<ClaudeHistorySectionsProps> = {},
): ClaudeHistorySectionsProps {
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
  h.mockUseClaudeHistory.mockReset();
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
});

// ═══════════════════════════════════════════════════════════════
// ClaudeHistorySections 结构与默认态
// ═══════════════════════════════════════════════════════════════

describe("ClaudeHistorySections 结构与默认态", () => {
  it("渲染搜索框 + 刷新按钮 + 两个区块头（位于搜索框之下）", () => {
    seedExplorerProject("C:/project");
    const { container, getByText } = render(
      React.createElement(ClaudeHistorySections, makeSectionsProps()),
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
      React.createElement(ClaudeHistorySections, makeSectionsProps()),
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
        ClaudeHistorySections,
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
      React.createElement(ClaudeHistorySections, makeSectionsProps()),
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
      React.createElement(ClaudeHistorySections, props),
    );
    expect(h.mockScan).not.toHaveBeenCalled();

    // 首次展开当前项目区 → scan 一次，行渲染
    rerender(
      React.createElement(ClaudeHistorySections, {
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
      React.createElement(ClaudeHistorySections, {
        ...props,
        expandedCurrent: false,
      }),
    );
    rerender(
      React.createElement(ClaudeHistorySections, {
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
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

    // NAH-09①：expandedGroups 初始为空（白名单模型）→ 组标题箭头 ▶
    expect(groups[0].textContent).toContain("▶");

    // 点击第一组展开 → 该组 key 加入 expandedGroups（箭头 ▼）+ 该组 2 行渲染，其他组仍收起
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]')[0]
        .textContent,
    ).toContain("▼");
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);

    // 再点收起 → 该组 key 移除（箭头 ▶）→ 恢复 0 行
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]')[0]
        .textContent,
    ).toContain("▶");
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
        makeSectionsProps({ expandedCurrent: true, rootPath: null }),
      ),
    );
    expect(getByText("无活跃项目")).toBeTruthy();
  });

  it("搜索无结果 →「无匹配的会话」提示", () => {
    seedExplorerProject("C:/project");
    const { container, getByText } = render(
      React.createElement(
        ClaudeHistorySections,
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
        ClaudeHistorySections,
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
      new Map([["session-1", "attention"]]),
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
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
        ClaudeHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["session-1", "attention"]]),
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
        ClaudeHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["session-1", "attention"]]),
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
        ClaudeHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["session-1", "attention"]]),
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
        ClaudeHistorySections,
        makeSectionsProps({
          expandedCurrent: true,
          rootPath: "C:/project/src",
          sessions: [normalSession("session-1", "C:/project/src", { title: "运行中会话" })],
          activeStatuses: new Map([["session-1", "attention"]]),
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
    h.mockUseClaudeHistory.mockReturnValue(defaultHistory);
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
    h.mockUseClaudeHistory.mockReturnValue({
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
    h.mockUseClaudeHistory.mockReturnValue({
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
