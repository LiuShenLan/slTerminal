// claude-history-view.test.tsx — FE-07/08/09 历史区视图 L2 测试
//
// 覆盖：
//   1. ClaudeHistorySections 结构（搜索框/刷新按钮/两区块头）与默认折叠态
//   2. 历史区首次展开触发 scan()（仅首次；之后靠刷新按钮）
//   3. 搜索框输入过滤行（matchesSearch）+ 无结果提示
//   4. 全部项目区分组折叠（组标题 basename + title 悬停 + 展开/收起）
//   5. 空态四文案（该项目暂无历史会话 / 暂无历史会话 / 无活跃项目 / 无匹配的会话）
//   6. 右键菜单可用性矩阵（普通/孤儿/无 cwd/⚡ × 4 操作）与各操作链路
//   7. 双击分派三分支（普通 → 恢复；孤儿/无 cwd → 无操作；⚡ → ask 引导 fork）
//   8. AgentStatusView 三区集成（默认态：活跃展开、历史收起；展开触发 scan；E2E 红线）
//
// 使用真实 useClaudeHistory（mock ../ipc/claudeHistory + TerminalRegistry，照
// claude-history-hook.test.tsx 模式）+ 真实 stores 种子 + 真实 Row/InputDialog/菜单。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import {
  render,
  cleanup,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import { ClaudeHistorySections } from "../features/claudeHistory/ClaudeHistorySections";
import { AgentStatusView } from "../features/agentStatus/AgentStatusView";
import {
  resetProjectStores,
  seedExplorerProject,
} from "./helpers/workspace-setup";
import type { HistorySession } from "../types/claudeHistory";

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
    mockScanHistory: vi.fn(),
    mockDeleteHistorySession: vi.fn(),
    mockRenameHistorySession: vi.fn(),
    mockAsk: vi.fn(),
    mockWriteText: vi.fn(),
    mockRestore: vi.fn(),
    mockUseAgentStatus: vi.fn(),
  };
});

// ── 模块级 mock ──

vi.mock("../ipc/claudeHistory", () => ({
  scanHistory: h.mockScanHistory,
  deleteHistorySession: h.mockDeleteHistorySession,
  renameHistorySession: h.mockRenameHistorySession,
}));

vi.mock("../ipc/dialog", () => ({
  ask: h.mockAsk,
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: h.mockWriteText,
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

// ── 辅助函数 ──

/** 最小 HistorySession 工厂 */
function makeSession(
  id: string,
  overrides: Partial<HistorySession> = {},
): HistorySession {
  return {
    sessionId: id,
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    ...overrides,
  };
}

/** 常规（可恢复）会话：cwd 存在且目录存在 */
function normalSession(id: string, cwd: string, overrides: Partial<HistorySession> = {}) {
  return makeSession(id, { cwd, cwdExists: true, ...overrides });
}

/** 渲染 ClaudeHistorySections 的默认 props（受控折叠态可经 rerender 调整） */
const defaultSectionProps = {
  expandedCurrent: false,
  expandedAll: false,
  onToggleCurrent: vi.fn(),
  onToggleAll: vi.fn(),
};

beforeEach(() => {
  resetProjectStores();
  h.listeners.clear();
  h.all.clear();
  h.mockGetAll.mockClear(); // 默认实现 () => new Map(all)，测试经 h.all 注入注册表内容
  h.mockSubscribe.mockClear();
  h.mockScanHistory.mockReset();
  h.mockScanHistory.mockResolvedValue([]);
  h.mockDeleteHistorySession.mockReset();
  h.mockDeleteHistorySession.mockResolvedValue(undefined);
  h.mockRenameHistorySession.mockReset();
  h.mockRenameHistorySession.mockResolvedValue(undefined);
  h.mockAsk.mockReset();
  h.mockAsk.mockResolvedValue(true);
  h.mockWriteText.mockReset();
  h.mockRestore.mockReset();
  h.mockRestore.mockResolvedValue(undefined);
  h.mockUseAgentStatus.mockReset();
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
      React.createElement(ClaudeHistorySections, defaultSectionProps),
    );

    // 搜索框 + 刷新按钮（FE-12）
    expect(
      container.querySelector('[data-e2e="agent-history-search"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-refresh"]'),
    ).toBeTruthy();
    // 两个区块头（FE-12）
    expect(
      container.querySelector('[data-e2e="agent-history-section-current"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-section-all"]'),
    ).toBeTruthy();
    expect(getByText("当前项目历史会话")).toBeTruthy();
    expect(getByText("全部项目历史会话")).toBeTruthy();

    // 搜索框位于两个区块之上（DOM 顺序：search 先于 section-current 先于 section-all）
    const search = container.querySelector('[data-e2e="agent-history-search"]')!;
    const current = container.querySelector(
      '[data-e2e="agent-history-section-current"]',
    )!;
    const all = container.querySelector('[data-e2e="agent-history-section-all"]')!;
    expect(search.compareDocumentPosition(current) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(current.compareDocumentPosition(all) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("默认两历史区收起：不渲染行、不触发 scan", () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src"),
    ]);

    const { container } = render(
      React.createElement(ClaudeHistorySections, defaultSectionProps),
    );

    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
    expect(h.mockScanHistory).not.toHaveBeenCalled();
  });

  it("点击刷新按钮触发 scan()", async () => {
    seedExplorerProject("C:/project");
    const { container } = render(
      React.createElement(ClaudeHistorySections, defaultSectionProps),
    );

    const refresh = container.querySelector(
      '[data-e2e="agent-history-refresh"]',
    ) as HTMLElement;
    fireEvent.click(refresh);

    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 展开触发 scan（仅首次）
// ═══════════════════════════════════════════════════════════════

describe("历史区首次展开触发 scan", () => {
  it("展开当前项目区 → scan 一次；收起再展开另一区 → 不重复 scan", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src"),
    ]);

    const { rerender } = render(
      React.createElement(ClaudeHistorySections, defaultSectionProps),
    );
    expect(h.mockScanHistory).not.toHaveBeenCalled();

    // 首次展开当前项目区 → scan 一次，行渲染
    rerender(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(
        document.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    // 收起当前区再展开全部区 → 不重复 scan（仅首次）
    rerender(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: false,
      }),
    );
    rerender(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
  });

  it("展开全部项目区同样触发 scan（数据同源，两区共享扫描结果）", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src"),
      normalSession("session-2", "D:/other"),
    ]);

    const { rerender } = render(
      React.createElement(ClaudeHistorySections, defaultSectionProps),
    );
    rerender(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(
        document.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(2);
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 搜索过滤
// ═══════════════════════════════════════════════════════════════

describe("搜索框过滤", () => {
  it("输入关键词过滤两区当前展开的列表（标题/首条 prompt，大小写不敏感）", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src", {
        title: "修复登录 bug",
        firstPrompt: "排查 token 过期问题",
      }),
      normalSession("session-2", "C:/project/src", {
        title: "重构界面",
        firstPrompt: "调整布局",
      }),
    ]);

    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(2);
    });

    // 标题匹配
    const search = container.querySelector(
      '[data-e2e="agent-history-search"]',
    ) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "登录" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(1);

    // prompt 匹配（大小写不敏感）
    fireEvent.change(search, { target: { value: "TOKEN" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(1);

    // 清空 → 全部恢复
    fireEvent.change(search, { target: { value: "" } });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);
  });
});

// ═══════════════════════════════════════════════════════════════
// 全部项目区分组折叠
// ═══════════════════════════════════════════════════════════════

describe("全部项目区分组折叠", () => {
  it("按 cwd 分组：组标题 basename + title 悬停完整路径 + 未知目录组 + 展开/收起", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("s1", "D:/a/projA", { mtimeMs: 4000 }),
      normalSession("s2", "D:/a/projA", { mtimeMs: 3000 }),
      normalSession("s3", "D:/b/projB", { mtimeMs: 2000 }),
      makeSession("s4", { cwd: null, mtimeMs: 1000 }),
    ]);

    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-group"]').length,
      ).toBe(3);
    });

    const groups = Array.from(
      container.querySelectorAll('[data-e2e="agent-history-group"]'),
    );
    // 组标题 = basename；title 属性 = 完整路径
    expect(groups[0].textContent).toContain("projA");
    expect(groups[0].getAttribute("title")).toBe("D:/a/projA");
    // 未知目录组标题「(未知目录)」，无 title
    expect(groups[2].textContent).toContain("(未知目录)");
    expect(groups[2].getAttribute("title")).toBeNull();

    // 组内行渲染
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(4);

    // 点击组标题收起 → 组内行隐藏，其他组不受影响
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(2);

    // 再点展开 → 恢复
    fireEvent.click(groups[0]);
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(4);
  });
});

// ═══════════════════════════════════════════════════════════════
// 空态四文案（FE-09）
// ═══════════════════════════════════════════════════════════════

describe("空态与提示文案", () => {
  it("当前项目无历史会话 →「该项目暂无历史会话」", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("s1", "D:/other", { title: "其他项目会话" }),
    ]);

    const { getByText, container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(getByText("该项目暂无历史会话")).toBeTruthy();
    });
    // 行不渲染（过滤后为空）
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
  });

  it("全部项目无任何会话 →「暂无历史会话」", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([]);

    const { getByText, container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(getByText("暂无历史会话")).toBeTruthy();
    });
    expect(
      container.querySelectorAll('[data-e2e="agent-history-group"]').length,
    ).toBe(0);
  });

  it("无活跃项目（rootPath null）→ 当前项目区显示「无活跃项目」", async () => {
    // 不种子 project → rootPath 为 null
    const { getByText } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    expect(getByText("无活跃项目")).toBeTruthy();
  });

  it("搜索无结果 →「无匹配的会话」提示", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("s1", "C:/project/src", { title: "修复登录" }),
    ]);

    const { container, getByText } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    const search = container.querySelector(
      '[data-e2e="agent-history-search"]',
    ) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "不存在的关键词" } });
    expect(getByText("无匹配的会话")).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 右键菜单可用性矩阵（普通/孤儿/无 cwd/⚡ × 4 操作）
// ═══════════════════════════════════════════════════════════════

describe("右键菜单可用性矩阵", () => {
  /** 渲染全部区并返回「按 cwd 分组后的第一行」元素 */
  async function renderAllAndGetFirstRow(sessions: HistorySession[]) {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue(sessions);
    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBeGreaterThan(0);
    });
    return container.querySelector('[data-e2e="agent-history-row"]') as HTMLElement;
  }

  /** 右键打开第一行的菜单，返回 { container, items } */
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

  it("普通行：四项操作全部可用（复制/分支恢复/删除/重命名）", async () => {
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    const { labels } = openMenu(row);

    expect(labels).toContain("复制恢复命令");
    expect(labels).toContain("分支恢复");
    expect(labels).toContain("删除");
    expect(labels).toContain("重命名");
  });

  it("复制恢复命令：写入剪贴板，格式 = cd '<cwd>' && claude --resume <id>", async () => {
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    openMenu(row);

    fireEvent.click(document.body.querySelector('[data-e2e="agent-history-menu"]')!.children[0]!);
    expect(h.mockWriteText).toHaveBeenCalledWith(
      "cd 'D:/a/projA' && claude --resume session-1",
    );
  });

  it("复制恢复命令：无 cwd 行 → 仅 claude --resume <id>", async () => {
    const row = await renderAllAndGetFirstRow([
      makeSession("session-1", { title: "无 cwd 会话" }),
    ]);
    openMenu(row);

    fireEvent.click(document.body.querySelector('[data-e2e="agent-history-menu"]')!.children[0]!);
    expect(h.mockWriteText).toHaveBeenCalledWith("claude --resume session-1");
  });

  it("分支恢复：普通行可用 → restoreHistorySession(session, { fork: true })", async () => {
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "普通会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    // items[0]=复制 items[1]=分支恢复 items[2]=删除 items[3]=重命名
    fireEvent.click(items[1]!);
    expect(h.mockRestore).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: "session-1" }),
      { fork: true },
    );
  });

  it("孤儿行：分支恢复禁用（点击无效果）", async () => {
    // cwd 非 null 但 cwdExists=false → orphan
    const row = await renderAllAndGetFirstRow([
      makeSession("session-1", { cwd: "D:/gone", cwdExists: false, title: "孤儿会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[1]!); // 分支恢复（禁用项无 onClick）
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("无 cwd 行：分支恢复禁用", async () => {
    const row = await renderAllAndGetFirstRow([
      makeSession("session-1", { cwd: null, title: "无 cwd 会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[1]!);
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("运行中行（⚡）：删除禁用", async () => {
    // TerminalRegistry 含 transcriptPath 指向 session-1 → activeIds 含 session-1
    h.all.set("panel-1", {
      claudeSession: { transcriptPath: "C:\\x\\session-1.jsonl" },
    });
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "运行中会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[2]!); // 删除（禁用项无 onClick）
    expect(h.mockAsk).not.toHaveBeenCalled();
    expect(h.mockDeleteHistorySession).not.toHaveBeenCalled();
  });

  it("删除：ask 确认 → deleteHistorySession → removeLocal 即时移除行", async () => {
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "待删会话" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[2]!);

    // ask 确认 → 删除 IPC
    await waitFor(() => {
      expect(h.mockAsk).toHaveBeenCalledWith(
        '确定删除会话"待删会话"？此操作不可撤销。',
        expect.any(Object),
      );
      expect(h.mockDeleteHistorySession).toHaveBeenCalledWith("session-1");
    });
    // removeLocal 即时刷新 → 行从列表移除
    await waitFor(() => {
      expect(
        document.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(0);
    });
  });

  it("删除：ask 取消 → 不删除", async () => {
    h.mockAsk.mockResolvedValue(false);
    const row = await renderAllAndGetFirstRow([
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

  it("重命名：打开 InputDialog → 提交后 renameHistorySession → updateLocalTitle 即时更新标题", async () => {
    const row = await renderAllAndGetFirstRow([
      normalSession("session-1", "D:/a/projA", { title: "旧标题" }),
    ]);
    openMenu(row);

    const items = document.body.querySelector(
      '[data-e2e="agent-history-menu"]',
    )!.children;
    fireEvent.click(items[3]!);

    // InputDialog 弹出（初始值 = 当前标题）
    const dialog = await waitFor(() =>
      document.querySelector('[data-e2e="agent-history-input-dialog"]'),
    );
    expect(dialog).toBeTruthy();

    const input = dialog!.querySelector("input") as HTMLInputElement;
    expect(input.value).toBe("旧标题");
    fireEvent.change(input, { target: { value: "新标题" } });
    const buttons = Array.from(dialog!.querySelectorAll("button"));
    fireEvent.click(buttons[buttons.length - 1]!); // 确认按钮

    // 重命名 IPC + 局部刷新
    await waitFor(() => {
      expect(h.mockRenameHistorySession).toHaveBeenCalledWith(
        "session-1",
        "新标题",
      );
    });
    await waitFor(() => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
      expect(rows.length).toBe(1);
      expect(rows[0].textContent).toContain("新标题");
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 双击分派三分支
// ═══════════════════════════════════════════════════════════════

describe("双击分派三分支", () => {
  it("普通行双击 → restoreHistorySession(session)", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src", { title: "普通会话" }),
    ]);
    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: "session-1" }),
    );
  });

  it("孤儿行双击 → 无操作", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      makeSession("session-1", {
        cwd: "C:/gone",
        cwdExists: false,
        title: "孤儿会话",
      }),
    ]);
    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).not.toHaveBeenCalled();
    expect(h.mockAsk).not.toHaveBeenCalled();
  });

  it("无 cwd 行双击 → 无操作", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      makeSession("session-1", { cwd: null, title: "无 cwd 会话" }),
    ]);
    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedAll: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    expect(h.mockRestore).not.toHaveBeenCalled();
  });

  it("运行中行双击 → ask「该会话已在运行中」→ 确认走 fork 恢复", async () => {
    seedExplorerProject("C:/project");
    h.all.set("panel-1", {
      claudeSession: { transcriptPath: "C:\\x\\session-1.jsonl" },
    });
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src", { title: "运行中会话" }),
    ]);
    const { container } = render(
      React.createElement(ClaudeHistorySections, {
        ...defaultSectionProps,
        expandedCurrent: true,
      }),
    );
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    fireEvent.doubleClick(
      container.querySelector('[data-e2e="agent-history-row"]')!,
    );
    await waitFor(() => {
      expect(h.mockAsk).toHaveBeenCalledWith("该会话已在运行中", {
        title: "会话运行中",
        kind: "warning",
        okLabel: "分支恢复",
      });
    });
    // ask 确认（默认 true）→ fork 恢复
    await waitFor(() => {
      expect(h.mockRestore).toHaveBeenCalledWith(
        expect.objectContaining({ sessionId: "session-1" }),
        { fork: true },
      );
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// AgentStatusView 三区集成（FE-08）
// ═══════════════════════════════════════════════════════════════

describe("AgentStatusView 三区集成", () => {
  const defaultAgentStatus = {
    state: { kind: "ready" as const },
    rows: [
      {
        panelId: "terminal-page1-0",
        pageId: "page1",
        projectId: "proj-1",
        title: "终端 page1",
        status: "attention",
        lastEventAt: Date.now(),
        usage: undefined,
      },
    ],
    currentProjectName: "测试项目",
  };

  beforeEach(() => {
    h.mockUseAgentStatus.mockReturnValue(defaultAgentStatus);
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
    expect(h.mockScanHistory).not.toHaveBeenCalled();
  });

  it("点击历史区标题展开 → 触发 scan()；再次收起展开不重复", async () => {
    seedExplorerProject("C:/project");
    h.mockScanHistory.mockResolvedValue([
      normalSession("session-1", "C:/project/src", { title: "历史会话" }),
    ]);
    const { getByText, container } = render(
      React.createElement(AgentStatusView, {
        switchToPage: vi.fn(),
        onDeletePage: vi.fn(),
      }),
    );

    fireEvent.click(getByText("当前项目历史会话"));
    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-e2e="agent-history-row"]').length,
      ).toBe(1);
    });

    // 收起再展开全部区 → 不重复 scan（仅首次）
    fireEvent.click(getByText("当前项目历史会话"));
    fireEvent.click(getByText("全部项目历史会话"));
    await waitFor(() => {
      expect(h.mockScanHistory).toHaveBeenCalledTimes(1);
    });
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
