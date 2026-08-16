// workspace-defaulttab.test.tsx — 生产 DefaultTab 渲染测试（WRK-05）
//
// 直接渲染 PageDockviewHost.tsx 导出的生产 DefaultTab 组件（非手写 Mock），
// 通过 fake PanelApi 驱动事件，验证：
// - tabStatus → StatusDot 状态圆点渲染（IC-03：tabIcon emoji/img 分支已随
//   STATUS_EMOJI 删除，DefaultTab 改读 tabStatus 渲染 StatusDot；StatusDot
//   本体在此 mock 为可识别 span——本文件只守卫接线：status 值透传 + 渲染条件，
//   圆点颜色/形态归 StatusDot 自身测试）
// - onDidParametersChange 事件结构（回调直接接收扁平 Parameters 对象，
//   event.tabStatus 而非 event.params.tabStatus——漂移即失败）
// - onDidTitleChange 标题更新、关闭按钮 api.close
//
// 事件结构说明（Workspace/CLAUDE.md「Dockview 事件结构注意事项」）：
// Dockview PanelApi.onDidParametersChange 类型为 Event<Parameters>，
// 回调直接接收 Parameters 对象，不是 { params: Parameters } 包裹结构。
// 生产 DefaultTab 若误写成 event.params.tabStatus 将恒为 undefined——本文件回归守卫。

import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { render, act, cleanup } from "@testing-library/react";
import { DefaultTab } from "../workspace/PageDockviewHost";
import type { AgentStatus } from "../lib/agentStatus";

// StatusDot 由 icon-base agent 并行实现（IC-02）——本文件 mock 为可识别 span
// （data-testid="status-dot"，文本 = status 值）。注意：不能用 importOriginal
// 展开真实模块（文件由并行 agent 创建，此时可能尚不存在）；mock 整模块替换。
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
});

// ---- 辅助 ----

interface TabListeners {
  title: Array<(e: { title: string }) => void>;
  params: Array<(e: Record<string, unknown>) => void>;
}

/** 构造 fake PanelApi 并渲染生产 DefaultTab（StrictMode 双渲染：取最后实例） */
function renderTab(init: {
  title?: string;
  tabStatus?: AgentStatus | null;
  tabLogo?: string | null;
}) {
  const listeners: TabListeners = { title: [], params: [] };
  const api = {
    title: init.title ?? "terminal-0",
    component: "terminal",
    onDidTitleChange: vi.fn((cb: (e: { title: string }) => void) => {
      listeners.title.push(cb);
      return { dispose: vi.fn() };
    }),
    onDidParametersChange: vi.fn((cb: (e: Record<string, unknown>) => void) => {
      listeners.params.push(cb);
      return { dispose: vi.fn() };
    }),
    close: vi.fn(),
  };
  const params: Record<string, unknown> = {};
  if (init.tabStatus !== undefined) params.tabStatus = init.tabStatus;
  if (init.tabLogo !== undefined) params.tabLogo = init.tabLogo;

  const result = render(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    React.createElement(DefaultTab, { api, containerApi: {}, params } as any),
  );

  // 取 DefaultTab 根 div（DOM 顺序：状态圆点 → logo → 标题 → 关闭按钮）
  const root = result.container.firstChild as HTMLElement | null;

  // 每次现取（React 更新后 DOM 结构变化，快照数组会失效）
  const getChildren = () => (root ? Array.from(root.children) : []);

  /** StatusDot mock span（data-testid="status-dot"，文本 = status 值） */
  const statusDot = () => root?.querySelector('[data-testid="status-dot"]') ?? null;
  /** CLI 品牌 logo img（alt="CLI 图标"） */
  const statusImg = () => root?.querySelector('img[alt="CLI 图标"]') ?? null;
  // 标题 span 恒为最后一个 span（状态圆点 span 在它前面）
  const titleSpan = () => {
    const spans = root?.querySelectorAll("span") ?? [];
    return (spans[spans.length - 1] as HTMLElement | undefined) ?? null;
  };
  const closeBtn = () => root?.querySelector("button") ?? null;

  return { ...result, api, listeners, root, getChildren, statusDot, statusImg, titleSpan, closeBtn };
}

/** 模拟 Dockview 真实行为：updateParameters 后发射扁平 Parameters 对象 */
function emitParamsChange(listeners: TabListeners, params: Record<string, unknown>): void {
  act(() => {
    listeners.params.forEach((cb) => cb(params));
  });
}

/** 模拟 Dockview 真实行为：setTitle 后发射 TitleEvent { title } */
function emitTitleChange(listeners: TabListeners, title: string): void {
  act(() => {
    listeners.title.forEach((cb) => cb({ title }));
  });
}

describe("DefaultTab（生产组件）", () => {
  describe("初始状态", () => {
    it("params.tabStatus 为 null → 不渲染状态圆点", () => {
      const { statusDot, titleSpan } = renderTab({ tabStatus: null });
      expect(statusDot()).toBeNull();
      // 第一个子元素直接是标题 span（无圆点占位）
      expect(titleSpan()?.textContent).toBe("terminal-0");
    });

    it("params.tabStatus 为 working → 渲染 StatusDot（status 值透传）", () => {
      const { statusDot, getChildren } = renderTab({ tabStatus: "working" });
      const dot = statusDot() as HTMLElement;
      expect(dot).toBeTruthy();
      expect(dot.textContent).toBe("working");
      expect(getChildren()[0]).toBe(dot);
    });

    it("params 无 tabStatus 字段（undefined）→ 不崩溃、无圆点", () => {
      const { statusDot, titleSpan } = renderTab({});
      expect(statusDot()).toBeNull();
      expect(titleSpan()?.textContent).toBe("terminal-0");
    });

    it("初始标题显示 api.title", () => {
      const { titleSpan } = renderTab({ title: "claude" });
      expect(titleSpan()?.textContent).toBe("claude");
    });
  });

  describe("动态更新（onDidParametersChange 真实事件路径）", () => {
    it("tabStatus 从 null 变为 working → 圆点出现（status 透传）", () => {
      const { listeners, statusDot } = renderTab({ tabStatus: null });
      expect(statusDot()).toBeNull();
      emitParamsChange(listeners, { tabStatus: "working" });
      expect(statusDot()?.textContent).toBe("working");
    });

    it("tabStatus 从 working 变为 attention → 圆点 status 更新", () => {
      const { listeners, statusDot } = renderTab({ tabStatus: "working" });
      expect(statusDot()?.textContent).toBe("working");
      emitParamsChange(listeners, { tabStatus: "attention" });
      expect(statusDot()?.textContent).toBe("attention");
    });

    it("tabStatus 从非空变为 null → 圆点移除", () => {
      const { listeners, statusDot, getChildren } = renderTab({ tabStatus: "working" });
      expect(statusDot()).toBeTruthy();
      emitParamsChange(listeners, { tabStatus: null });
      expect(statusDot()).toBeNull();
      // 第一个子元素回退为标题 span
      expect((getChildren()[0] as HTMLElement).textContent).toBe("terminal-0");
    });

    it("回归守卫：event.params.tabStatus 包裹结构不生效（真实结构为扁平 event.tabStatus）", () => {
      // 若生产代码误写成 event.params.tabStatus，此用例将失败（事件恒 undefined）
      const { listeners, statusDot } = renderTab({ tabStatus: null });
      emitParamsChange(listeners, { params: { tabStatus: "working" } });
      expect(statusDot()).toBeNull();
      // 扁平结构才生效
      emitParamsChange(listeners, { tabStatus: "working" });
      expect(statusDot()?.textContent).toBe("working");
    });

    it("event 为 undefined → 不崩溃（undefined ?? null → 状态清空）", () => {
      const { listeners, statusDot } = renderTab({ tabStatus: "working" });
      act(() => {
        listeners.params.forEach((cb) => cb(undefined as unknown as Record<string, unknown>));
      });
      expect(statusDot()).toBeNull();
    });
  });

  describe("标题更新（onDidTitleChange）", () => {
    it("setTitle 后标题更新（回调收到 TitleEvent { title }）", () => {
      const { listeners, titleSpan } = renderTab({ title: "terminal-0" });
      expect(titleSpan()?.textContent).toBe("terminal-0");
      emitTitleChange(listeners, "claude");
      expect(titleSpan()?.textContent).toBe("claude");
    });

    it("标题更新不影响状态圆点与 logo", () => {
      const { listeners, statusDot, statusImg, titleSpan } = renderTab({
        tabStatus: "working",
        tabLogo: "/cli-icons/claude.png",
      });
      emitTitleChange(listeners, "claude-v2");
      expect(statusDot()?.textContent).toBe("working");
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
      expect(titleSpan()?.textContent).toBe("claude-v2");
    });
  });

  describe("渲染属性", () => {
    it("DOM 顺序：状态圆点 → 标题 → 关闭按钮", () => {
      const { statusDot, titleSpan, closeBtn } = renderTab({ tabStatus: "attention" });
      expect(statusDot()?.textContent).toBe("attention");
      expect(titleSpan()?.textContent).toBe("terminal-0");
      expect(closeBtn()?.textContent).toBe("×");
      expect(closeBtn()?.getAttribute("title")).toBe("关闭");
    });
  });

  describe("关闭按钮", () => {
    it("点击 × 调用 api.close", () => {
      const { api, closeBtn } = renderTab({});
      act(() => {
        closeBtn()?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      });
      expect(api.close).toHaveBeenCalledTimes(1);
    });
  });

  describe("CLI logo（tabLogo，跟随页签名显示——F9 行为修订）", () => {
    it("tabStatus + tabLogo → 圆点与 logo 并存（src/16×16/flexShrink）", () => {
      const { statusDot, statusImg } = renderTab({
        tabStatus: "working",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusDot()?.textContent).toBe("working");
      expect(statusImg()).toBeTruthy();
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
      expect(statusImg()?.getAttribute("width")).toBe("16");
      expect(statusImg()?.getAttribute("height")).toBe("16");
      expect((statusImg() as HTMLElement).style.flexShrink).toBe("0");
    });

    it("DOM 顺序：状态圆点 → logo img → 标题 → 关闭按钮", () => {
      const { getChildren, statusDot, statusImg, titleSpan, closeBtn } = renderTab({
        tabStatus: "working",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(getChildren()[0]).toBe(statusDot());
      expect(getChildren()[1]).toBe(statusImg());
      expect(titleSpan()?.textContent).toBe("terminal-0");
      expect(closeBtn()?.getAttribute("title")).toBe("关闭");
    });

    it("tabStatus 为 null + tabLogo → 仍渲染 logo（不依赖状态圆点）", () => {
      const { statusImg, getChildren, titleSpan } = renderTab({
        tabStatus: null,
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
      // 无状态圆点：logo 顶到标题前（位置语义不变）
      expect(getChildren()[0]).toBe(statusImg());
      expect(titleSpan()?.textContent).toBe("terminal-0");
    });

    it("tabLogo 为 null → 无 logo img（无会话）", () => {
      const { statusImg } = renderTab({ tabLogo: null });
      expect(statusImg()).toBeNull();
    });

    it("动态更新：仅 tabLogo 出现（tabStatus 缺席）→ logo img 渲染", () => {
      const { listeners, statusImg } = renderTab({ tabStatus: null });
      expect(statusImg()).toBeNull();
      emitParamsChange(listeners, { tabLogo: "/cli-icons/claude.png" });
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
    });

    it("动态更新：emitParamsChange 携 tabStatus + tabLogo → 圆点与 logo 同时出现（扁平参数回归）", () => {
      const { listeners, statusDot, statusImg } = renderTab({ tabStatus: null });
      expect(statusDot()).toBeNull();
      expect(statusImg()).toBeNull();
      emitParamsChange(listeners, {
        tabStatus: "working",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusDot()?.textContent).toBe("working");
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
    });

    it("动态更新：tabLogo 移除 → logo img 消失（圆点保留）", () => {
      const { listeners, statusDot, statusImg } = renderTab({
        tabStatus: "working",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusImg()).toBeTruthy();
      emitParamsChange(listeners, { tabStatus: "working", tabLogo: null });
      expect(statusImg()).toBeNull();
      // 状态圆点仍保留
      expect(statusDot()?.textContent).toBe("working");
    });
  });
});
