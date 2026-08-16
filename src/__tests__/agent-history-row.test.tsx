// agent-history-row.test.tsx — HistorySessionRow L2 测试（FE-07 / FE-10）
//
// 覆盖：单行渲染（标题/相对时间/行高 30px）、prompt 预览 → 行容器原生 title tooltip、
// title null → sessionId 前 8 位、四态标记（status：working/attention/done/error/null——
// 问题 2）、孤儿标记、字号（标题 12px 粗体/时间 11px）、单击/双击/右键回调、选中态高亮、
// CLI logo 按 session.cliId 查 profile.iconSrc（MC-311：未注册 cliId → 无 logo 不报错）。
//
// NAV-08：双行改单行 30px（供导航树复用）——prompt 预览不再渲染为第二行文本。
// Stage 05：side-effect import profiles 注册真实 claude profile——logo 断言
// （iconSrc = /cli-icons/claude.png）经注册表查询（MC-311 委托）。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { HistorySessionRow } from "../features/agentHistory/HistorySessionRow";
import { formatRelativeTime } from "../features/agentHistory/historyModel";
import { EXPLORER_SELECTION_BG } from "../theme";
import "../features/cliProfiles/profiles";
import type { AgentHistorySession } from "../types/agentHistory";

// StatusDot 由 icon-base agent 并行实现（IC-02）——本文件 mock 为可识别 span
// （data-testid="status-dot"，文本 = status 值），只断言接线不依赖其内部 DOM
vi.mock("../lib/StatusDot", async () => {
  const { createElement } = await import("react");
  return {
    StatusDot: ({ status }: { status: string | null }) =>
      status == null
        ? null
        : createElement("span", { "data-testid": "status-dot" }, status),
  };
});

afterEach(cleanup);

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 输入补空格 → "rgba(r, g, b, a)"，照 activityBar.test.tsx 模式） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const n = parseInt(hex.slice(1), 16);
    return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
  }
  // 新方案 selection 类 token 为 rgba 形态，jsdom 输出 "rgba(r, g, b, a)"
  const m = hex.match(/^rgba\((\d+),(\d+),(\d+),([0-9.]+)\)$/);
  if (m) return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${m[4]})`;
  return hex;
}

/** 构造测试会话（默认值：5 分钟前、有标题、cwd 存在） */
function makeSession(
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    cwd: "D:\\data\\learn\\code\\slTerminal",
    title: "修复登录 bug 的会话",
    titleSource: "aiTitle",
    firstPrompt: "修复了 token 过期问题导致的重…",
    mtimeMs: Date.now() - 60_000 * 5,
    cwdExists: true,
    cliId: "claude",
    ...overrides,
  };
}

/** 渲染行组件，返回根元素与 mock 回调 */
function renderRow(
  session: AgentHistorySession,
  props: Partial<
    Pick<
      Parameters<typeof HistorySessionRow>[0],
      "status" | "orphan" | "noCwd" | "selected"
    >
  > = {},
) {
  const onSelect = vi.fn();
  const onDoubleClick = vi.fn();
  const onContextMenu = vi.fn();
  const utils = render(
    <HistorySessionRow
      session={session}
      status={props.status}
      orphan={props.orphan ?? false}
      noCwd={props.noCwd ?? false}
      selected={props.selected ?? false}
      onSelect={onSelect}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
    />,
  );
  const row = utils.container.querySelector(
    '[data-e2e="agent-history-row"]',
  ) as HTMLElement;
  return { ...utils, row, onSelect, onDoubleClick, onContextMenu };
}

describe("HistorySessionRow 渲染", () => {
  it("单行式：粗体标题（12px）+ 右侧相对时间（11px），行高 30px（NAV-08）", () => {
    const session = makeSession();
    const { getByText, container } = renderRow(session);

    const titleEl = getByText("修复登录 bug 的会话");
    // 标题 500 字重 12px（UI-205：字重仅 400/500）
    expect(titleEl.style.fontWeight).toBe("500");
    expect(titleEl.style.fontSize).toBe("12px");
    // 相对时间（与 historyModel 同源函数计算期望值）
    expect(
      getByText(formatRelativeTime(session.mtimeMs, Date.now())),
    ).toBeTruthy();
    // 单行结构：行根下不再有行1/行2 两级容器（直接是内容项）
    const row = container.querySelector(
      '[data-e2e="agent-history-row"]',
    ) as HTMLElement;
    expect(row.style.height).toBe("30px");
    expect(row.querySelector("div")).toBeNull();
  });

  it("prompt 预览不再渲染为第二行，改放行容器原生 title（悬停 tooltip）", () => {
    const session = makeSession();
    const { row, queryByText } = renderRow(session);

    // 预览文本不出现在 DOM 文本中
    expect(queryByText("修复了 token 过期问题导致的重…")).toBeNull();
    // 原生 title 属性 = firstPrompt（预览经 tooltip 呈现）
    expect(row.getAttribute("title")).toBe(session.firstPrompt);
  });

  it("firstPrompt 为 null → 行容器无 title 属性", () => {
    const session = makeSession({ firstPrompt: null });
    const { row } = renderRow(session);
    expect(row.hasAttribute("title")).toBe(false);
  });

  it("title 为 null 时显示 sessionId 前 8 位", () => {
    const session = makeSession({ title: null, titleSource: "none" });
    const { getByText, queryByText } = renderRow(session);

    expect(getByText("a1b2c3d4")).toBeTruthy();
    // 不显示 sessionId 全串
    expect(queryByText(session.sessionId)).toBeNull();
  });
});

describe("HistorySessionRow 状态标记（问题 2：四态同源）", () => {
  it("status=working → 状态圆点（StatusDot 透传 working）", () => {
    const session = makeSession();
    const { row } = renderRow(session, {
      status: "working",
    });

    const dot = row.querySelector('[data-testid="status-dot"]');
    expect(dot?.textContent).toBe("working");
    // 非孤儿行无孤儿标记（IC-08：孤儿标记 = IconClose，data-e2e="agent-history-orphan"）
    expect(row.querySelector('[data-e2e="agent-history-orphan"]')).toBeNull();
  });

  it("status=attention → 状态圆点（attention）", () => {
    const { row } = renderRow(makeSession(), { status: "attention" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("attention");
  });

  it("status=done → 状态圆点（done）", () => {
    const { row } = renderRow(makeSession(), { status: "done" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("done");
  });

  it("status=error → 状态圆点（error）", () => {
    const { row } = renderRow(makeSession(), { status: "error" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("error");
  });

  it("status=null / undefined → 无状态圆点，但 CLI logo 仍渲染（F9 行为修订：跟随会话名显示）", () => {
    const { row } = renderRow(makeSession(), { status: null });
    expect(row.querySelector('[data-testid="status-dot"]')).toBeNull();
    // logo 不依赖状态圆点：行存在（有会话名）即按 session.cliId 显示
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
  });

  it("status=working → 圆点后渲染 CLI logo（按 session.cliId 查 profile.iconSrc/14×14/位于圆点与标题间——MC-311）", () => {
    const { row } = renderRow(makeSession(), { status: "working" });
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    // logo 存在：src = claude profile.iconSrc（经 cliProfileRegistry 查 session.cliId）、14×14（NAV-08）
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("14");
    expect(logoImg?.getAttribute("height")).toBe("14");
    // 单行子元素序：状态圆点 span → logo img → 标题 → 时间
    const children = Array.from(row.children);
    expect(children[0].getAttribute("data-testid")).toBe("status-dot");
    expect(children[0].textContent).toBe("working");
    expect(children[1]).toBe(logoImg);
  });

  it("status=null → logo 顶到行首位（圆点缺席时 logo 在标题前，位置语义不变）", () => {
    const { row } = renderRow(makeSession(), { status: null });
    const children = Array.from(row.children);
    // 第一个子元素即 logo img（无圆点占位）
    expect(children[0].tagName).toBe("IMG");
    expect(children[0].getAttribute("alt")).toBe("CLI 图标");
    // 其后是标题 span
    expect(children[1].tagName).toBe("SPAN");
    expect(children[1].textContent).toBe("修复登录 bug 的会话");
  });

  it("未注册 cliId → 无 logo 不报错（MC-311 降级语义）", () => {
    const session = makeSession({ cliId: "unknown-cli" });
    const { row } = renderRow(session, { status: "working" });

    // 状态圆点仍渲染；logo 查询未命中 → 无 img（组件不抛错）
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("working");
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("orphan 孤儿行（status null）→ 孤儿标记 + 仍渲染 logo（F9 行为修订：行存在即显示）", () => {
    const session = makeSession({ cwdExists: false });
    const { row } = renderRow(session, { orphan: true });
    // IC-08：孤儿标记 = IconClose（data-e2e="agent-history-orphan"）
    expect(row.querySelector('[data-e2e="agent-history-orphan"]')).toBeTruthy();
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
  });

  it("orphan=true → 孤儿标记（与四态圆点并存渲染）", () => {
    const session = makeSession({ cwdExists: false });
    const { row } = renderRow(session, {
      status: "attention",
      orphan: true,
    });

    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("attention");
    expect(row.querySelector('[data-e2e="agent-history-orphan"]')).toBeTruthy();
  });

  it("status=working 且 orphan=true → 圆点渲染（四态标记不被孤儿标记掩盖，NAH-10）", () => {
    const session = makeSession({ cwdExists: false });
    const { row } = renderRow(session, {
      status: "working",
      orphan: true,
    });

    // 四态圆点优先展示（运行中会话即使 cwd 目录已删，行标记仍为 working）
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe("working");
    // 孤儿标记按 orphan 语义独立渲染（两标记非互斥——现状设计，与上方并存用例一致）
    expect(row.querySelector('[data-e2e="agent-history-orphan"]')).toBeTruthy();
  });

  it("noCwd=true（无 cwd 跳过孤儿判定）→ 无孤儿标记，无状态时不显示圆点", () => {
    const session = makeSession({ cwd: null, cwdExists: false });
    const { row } = renderRow(session, { noCwd: true });

    expect(row.querySelector('[data-e2e="agent-history-orphan"]')).toBeNull();
    expect(row.querySelector('[data-testid="status-dot"]')).toBeNull();
  });
});

describe("HistorySessionRow 交互回调", () => {
  it("单击 → onSelect(sessionId)", () => {
    const session = makeSession();
    const { row, onSelect } = renderRow(session);

    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(session.sessionId);
  });

  it("双击 → onDoubleClick(session)", () => {
    const session = makeSession();
    const { row, onDoubleClick } = renderRow(session);

    fireEvent.doubleClick(row);
    expect(onDoubleClick).toHaveBeenCalledTimes(1);
    expect(onDoubleClick).toHaveBeenCalledWith(session);
  });

  it("右键 → onContextMenu(session, { x, y }) 带点击坐标", () => {
    const session = makeSession();
    const { row, onContextMenu } = renderRow(session);

    fireEvent.contextMenu(row, { clientX: 123, clientY: 45 });
    expect(onContextMenu).toHaveBeenCalledTimes(1);
    expect(onContextMenu).toHaveBeenCalledWith(session, { x: 123, y: 45 });
  });
});

describe("HistorySessionRow 选中态", () => {
  it("selected=true → 背景 EXPLORER_SELECTION_BG", () => {
    const session = makeSession();
    const { row } = renderRow(session, { selected: true });

    expect(row.style.backgroundColor).toBe(hexToRgb(EXPLORER_SELECTION_BG));
  });

  it("selected=false → 背景透明", () => {
    const session = makeSession();
    const { row } = renderRow(session, { selected: false });

    expect(row.style.backgroundColor).not.toBe(EXPLORER_SELECTION_BG);
  });
});
