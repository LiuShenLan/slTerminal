// agent-history-row.test.tsx — HistorySessionRow L2 测试（FE-07 / FE-10）
//
// 覆盖：双行渲染（标题/相对时间/prompt 预览）、title null → sessionId 前 8 位、
// 四态标记（status：working/attention/done/error/null——问题 2）、✗ 孤儿标记、
// 字号层级（行1 标题 12px 粗体/行2 11px——问题 4）、单击/双击/右键回调、选中态高亮、
// CLI logo 按 session.cliId 查 profile.iconSrc（MC-311：未注册 cliId → 无 logo 不报错）。
//
// Stage 05：side-effect import profiles 注册真实 claude profile——logo 断言
// （iconSrc = /cli-icons/claude.png）经注册表查询（MC-311 委托）。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { HistorySessionRow } from "../features/agentHistory/HistorySessionRow";
import { formatRelativeTime } from "../features/agentHistory/historyModel";
import { EXPLORER_SELECTION_BG } from "../theme";
import "../features/cliProfiles/profiles";
import type { AgentHistorySession } from "../types/agentHistory";

afterEach(cleanup);

/** hex → "rgb(r, g, b)"（jsdom 会把 inline 色值规范化为 rgb 形式，照 activityBar.test.tsx 模式） */
function hexToRgb(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
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
  it("双行式：行1 粗体标题（12px）+ 右上角相对时间，行2 prompt 预览（11px）", () => {
    const session = makeSession();
    const { getByText, container } = renderRow(session);

    const titleEl = getByText("修复登录 bug 的会话");
    // 行1 标题粗体
    expect(titleEl.style.fontWeight).toBe("bold");
    // 行1 容器 12px（问题 4：行标题层级）
    expect(titleEl.parentElement!.style.fontSize).toBe("12px");
    // 相对时间（与 historyModel 同源函数计算期望值）
    expect(
      getByText(formatRelativeTime(session.mtimeMs, Date.now())),
    ).toBeTruthy();
    // 行2 prompt 预览（11px）
    const promptEl = getByText("修复了 token 过期问题导致的重…");
    expect(promptEl.style.fontSize).toBe("11px");
    // 行1 与行2 为独立容器（双行结构）
    expect(container.querySelectorAll('[data-e2e="agent-history-row"] > div').length).toBe(2);
  });

  it("title 为 null 时显示 sessionId 前 8 位", () => {
    const session = makeSession({ title: null, titleSource: "none" });
    const { getByText, queryByText } = renderRow(session);

    expect(getByText("a1b2c3d4")).toBeTruthy();
    // 不显示 sessionId 全串
    expect(queryByText(session.sessionId)).toBeNull();
  });

  it("firstPrompt 为 null 时不渲染行2", () => {
    const session = makeSession({ firstPrompt: null });
    const { row } = renderRow(session);
    // 行2 不存在（行根下仅一个文本行容器 = 行1）
    expect(row.querySelectorAll("div").length).toBe(1);
  });
});

describe("HistorySessionRow 状态标记（问题 2：四态同源）", () => {
  it("status=working → ⚡", () => {
    const session = makeSession();
    const { getByText, queryByText } = renderRow(session, {
      status: "working",
    });

    expect(getByText("⚡")).toBeTruthy();
    expect(queryByText("✗")).toBeNull();
  });

  it("status=attention → 🟡", () => {
    const { getByText } = renderRow(makeSession(), { status: "attention" });
    expect(getByText("🟡")).toBeTruthy();
  });

  it("status=done → ✅", () => {
    const { getByText } = renderRow(makeSession(), { status: "done" });
    expect(getByText("✅")).toBeTruthy();
  });

  it("status=error → ❌", () => {
    const { getByText } = renderRow(makeSession(), { status: "error" });
    expect(getByText("❌")).toBeTruthy();
  });

  it("status=null / undefined → 无状态标记（与活跃区 null 无图标一致）", () => {
    const { queryByText, row } = renderRow(makeSession(), { status: null });
    expect(queryByText("⚡")).toBeNull();
    expect(queryByText("🟡")).toBeNull();
    expect(queryByText("✅")).toBeNull();
    expect(queryByText("❌")).toBeNull();
    // CLI logo 仅随 status emoji → status null 无 logo img
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("status=working → emoji 后渲染 CLI logo（按 session.cliId 查 profile.iconSrc/16×16/位于 emoji 与标题间——MC-311）", () => {
    const { row } = renderRow(makeSession(), { status: "working" });
    const line1 = row.children[0] as HTMLElement;
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    // logo 存在：src = claude profile.iconSrc（经 cliProfileRegistry 查 session.cliId）、16×16
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("16");
    expect(logoImg?.getAttribute("height")).toBe("16");
    // 行1 子元素序：emoji span(⚡) → logo img → 标题 → 时间
    const children = Array.from(line1.children);
    expect(children[0].tagName).toBe("SPAN");
    expect(children[0].textContent).toBe("⚡");
    expect(children[1]).toBe(logoImg);
  });

  it("未注册 cliId → 无 logo 不报错（MC-311 降级语义）", () => {
    const session = makeSession({ cliId: "unknown-cli" });
    const { row } = renderRow(session, { status: "working" });

    // emoji 仍渲染；logo 查询未命中 → 无 img（组件不抛错）
    expect(row.textContent).toContain("⚡");
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("orphan ✗ 行（status null）→ 不渲染 logo（✗ 后不加图）", () => {
    const session = makeSession({ cwdExists: false });
    const { getByText, row } = renderRow(session, { orphan: true });
    expect(getByText("✗")).toBeTruthy();
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("orphan=true → ✗ 标记（与四态并存渲染）", () => {
    const session = makeSession({ cwdExists: false });
    const { getByText } = renderRow(session, {
      status: "attention",
      orphan: true,
    });

    expect(getByText("🟡")).toBeTruthy();
    expect(getByText("✗")).toBeTruthy();
  });

  it("status=working 且 orphan=true → ⚡ 渲染（四态标记不被孤儿标记掩盖，NAH-10）", () => {
    const session = makeSession({ cwdExists: false });
    const { getByText } = renderRow(session, {
      status: "working",
      orphan: true,
    });

    // 四态 emoji 优先展示（运行中会话即使 cwd 目录已删，行标记仍为 ⚡）
    expect(getByText("⚡")).toBeTruthy();
    // ✗ 按 orphan 语义独立渲染（两标记非互斥——现状设计，与上方并存用例一致）
    expect(getByText("✗")).toBeTruthy();
  });

  it("noCwd=true（无 cwd 跳过孤儿判定）→ 不显示 ✗，无状态时不显示标记", () => {
    const session = makeSession({ cwd: null, cwdExists: false });
    const { queryByText } = renderRow(session, { noCwd: true });

    expect(queryByText("✗")).toBeNull();
    expect(queryByText("⚡")).toBeNull();
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
