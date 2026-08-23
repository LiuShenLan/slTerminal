// nav-history-row.test.tsx — NavHistoryRow L2 测试（FE-25 迁移：原 agent-history-row.test.tsx）
//
// 来源：HistorySessionRow 退役（FE-25）——原行级测试用例改写为 NavHistoryRow 面向。
// 保留独立语义（未被 nav-tree-history.test.tsx 覆盖者）：
//   - 单行渲染精确契约：行高 30px（SESSION_ROW_HEIGHT）/ 标题 12.5px / 相对时间 11px
//   - prompt 预览 → 行容器原生 title tooltip（含 firstPrompt null → 无 title 属性）
//   - title null → sessionId 前 8 位
//   - 四态透传（working/attention/done/error——集成层只断言「有圆点」未断言值）
//     + status null/undefined → 恒 done 灰档（NAV-10 契约）
//   - 行 logo 按 session.cliId 查 profile.iconSrc（含未注册 cliId 无 logo 不报错，MC-311）
//   - 双击/右键回调接线（含右键坐标参数）
// 删除语义（NavHistoryRow 无对应 prop，且集成层已覆盖）：
//   - 选中态高亮（无 selected——历史行无选中概念，选中态高亮仅属 NavTree 页面/会话行）
//   - 单击 onSelect（无该回调）
//   - 孤儿标记 orphan（无该 prop——孤儿/无 cwd 判定在 NavTree 双击分派与菜单禁用层，
//     nav-tree-history.test.tsx 覆盖）
//
// Mock 策略：StatusDot mock 为可识别 span（照原文件，只断言接线）；
// side-effect import profiles 注册真实 claude profile——logo 断言
// （iconSrc = /cli-icons/claude.png）经注册表查询（MC-311 委托）。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { NavHistoryRow } from "../features/navTree/NavHistoryRow";
import { formatRelativeTime } from "../features/agentHistory/historyModel";
import { SESSION_ROW_HEIGHT } from "../features/navTree/navStyles";
import type { AgentStatus } from "../lib/agentStatus";
import "../features/cliProfiles/profiles";
import type { AgentHistorySession } from "../types/agentHistory";
import { resetCliProfileRegistry } from "./helpers/mockCliProfile";

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

afterEach(() => {
  cleanup();
  // 清空 import profiles 的注册残留并恢复 claude 基线（TQ-B-14）——
  // 单例 _reset 后 side-effect 注册失效，logo 断言依赖的 claude profile 须显式补回
  resetCliProfileRegistry();
});

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

/** 渲染行组件，返回根元素与 mock 回调（status 缺省 undefined = 无运行状态） */
function renderRow(
  session: AgentHistorySession,
  props: { status?: AgentStatus | null } = {},
) {
  const onDoubleClick = vi.fn();
  const onContextMenu = vi.fn();
  const utils = render(
    <NavHistoryRow
      session={session}
      status={props.status}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
    />,
  );
  const row = utils.container.querySelector(
    '[data-e2e="nav-row-session"]',
  ) as HTMLElement;
  return { ...utils, row, onDoubleClick, onContextMenu };
}

describe("NavHistoryRow 渲染", () => {
  it("单行式：标题 + 右侧相对时间（11px），行高 30px（NAV-03 单行化）", () => {
    const session = makeSession();
    const { getByText, row } = renderRow(session);

    // 标题渲染 + 字号继承自行容器 12.5px（NAV-03 单行规范，nameStyle 无内联字号）
    expect(getByText("修复登录 bug 的会话")).toBeTruthy();
    expect(row.style.fontSize).toBe("12.5px");
    // 相对时间（与 historyModel 同源函数计算期望值），时间 span 11px fg-4
    const timeEl = getByText(formatRelativeTime(session.mtimeMs, Date.now()));
    expect(timeEl.style.fontSize).toBe("11px");
    // 单行结构：行高 30px（SESSION_ROW_HEIGHT 契约）
    expect(row.style.height).toBe(`${SESSION_ROW_HEIGHT}px`);
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

describe("NavHistoryRow 状态标记（NAV-10：圆点恒渲染）", () => {
  it("status=working → 状态圆点（StatusDot 透传 working）", () => {
    const { row } = renderRow(makeSession(), { status: "working" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "working",
    );
  });

  it("status=attention → 状态圆点（attention）", () => {
    const { row } = renderRow(makeSession(), { status: "attention" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "attention",
    );
  });

  it("status=done → 状态圆点（done）", () => {
    const { row } = renderRow(makeSession(), { status: "done" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "done",
    );
  });

  it("status=error → 状态圆点（error）", () => {
    const { row } = renderRow(makeSession(), { status: "error" });
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "error",
    );
  });

  it("status=null / undefined → 恒渲染 done 灰档（mockup .dot.idle 契约），logo 不受影响", () => {
    const { row } = renderRow(makeSession(), { status: null });
    // NAV-10：无运行状态 → done 灰档（StatusDot 恒渲染）
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "done",
    );
    // F9：行存在即按 session.cliId 显示 logo，不依赖状态
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
  });

  it("单行子元素序：状态圆点 → CLI logo → 标题 → 相对时间（NAV-03 结构契约）", () => {
    const session = makeSession();
    const { row } = renderRow(session, { status: "working" });

    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    const children = Array.from(row.children);
    expect(children[0].getAttribute("data-testid")).toBe("status-dot");
    expect(children[0].textContent).toBe("working");
    expect(children[1]).toBe(logoImg);
    // 最后一项为相对时间 span
    const last = children[children.length - 1] as HTMLElement;
    expect(last.textContent).toBe(
      formatRelativeTime(session.mtimeMs, Date.now()),
    );
  });
});

describe("NavHistoryRow 行 logo（MC-311）", () => {
  it("按 session.cliId 查 profile.iconSrc：claude → /cli-icons/claude.png、14×14（NAV-03）", () => {
    const { row } = renderRow(makeSession());
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("14");
    expect(logoImg?.getAttribute("height")).toBe("14");
  });

  it("未注册 cliId → 无 logo 不报错（MC-311 降级语义）", () => {
    const session = makeSession({ cliId: "unknown-cli" });
    const { row } = renderRow(session, { status: "working" });

    // 状态圆点仍渲染；logo 查询未命中 → 无 img（组件不抛错）
    expect(row.querySelector('[data-testid="status-dot"]')?.textContent).toBe(
      "working",
    );
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });
});

describe("NavHistoryRow 交互回调", () => {
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
