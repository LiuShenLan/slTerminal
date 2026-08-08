// workspace-defaulttab.test.tsx — 生产 DefaultTab 渲染测试（WRK-05）
//
// 直接渲染 PageDockviewHost.tsx 导出的生产 DefaultTab 组件（非手写 Mock），
// 通过 fake PanelApi 驱动事件，验证：
// - tabIcon emoji/img 分支渲染（含 / \ http: data: → img，否则 span）
// - onDidParametersChange 事件结构（回调直接接收扁平 Parameters 对象，
//   event.tabIcon 而非 event.params.tabIcon——漂移即失败）
// - onDidTitleChange 标题更新、关闭按钮 api.close
//
// 事件结构说明（Workspace/CLAUDE.md「Dockview 事件结构注意事项」）：
// Dockview PanelApi.onDidParametersChange 类型为 Event<Parameters>，
// 回调直接接收 Parameters 对象，不是 { params: Parameters } 包裹结构。
// 生产 DefaultTab 若误写成 event.params.tabIcon 将恒为 undefined——本文件回归守卫。

import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { render, act, cleanup } from "@testing-library/react";
import { DefaultTab } from "../workspace/PageDockviewHost";

afterEach(() => {
  cleanup();
});

// ---- 辅助 ----

interface TabListeners {
  title: Array<(e: { title: string }) => void>;
  params: Array<(e: Record<string, unknown>) => void>;
}

/** 构造 fake PanelApi 并渲染生产 DefaultTab（StrictMode 双渲染：取最后实例） */
function renderTab(init: { title?: string; tabIcon?: string | null; tabLogo?: string | null }) {
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
  if (init.tabIcon !== undefined) params.tabIcon = init.tabIcon;
  if (init.tabLogo !== undefined) params.tabLogo = init.tabLogo;

  const result = render(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    React.createElement(DefaultTab, { api, containerApi: {}, params } as any),
  );

  // 取 DefaultTab 根 div（DOM 顺序：图标 → logo → 标题 → 关闭按钮）
  const root = result.container.firstChild as HTMLElement | null;

  // 每次现取（React 更新后 DOM 结构变化，快照数组会失效）
  const getChildren = () => (root ? Array.from(root.children) : []);

  const iconImg = () => root?.querySelector("img") ?? null;
  /** CLI 品牌 logo img（alt="CLI 图标"，与 URL tabIcon 的 alt="页签图标" 区分） */
  const statusImg = () => root?.querySelector('img[alt="CLI 图标"]') ?? null;
  // 标题 span 恒为最后一个 span（图标 span 在它前面）
  const titleSpan = () => {
    const spans = root?.querySelectorAll("span") ?? [];
    return (spans[spans.length - 1] as HTMLElement | undefined) ?? null;
  };
  const closeBtn = () => root?.querySelector("button") ?? null;

  return { ...result, api, listeners, root, getChildren, iconImg, statusImg, titleSpan, closeBtn };
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
    it("params.tabIcon 为 null → 不渲染图标（无 img/span 图标）", () => {
      const { iconImg, titleSpan } = renderTab({ tabIcon: null });
      expect(iconImg()).toBeNull();
      // 第一个子元素直接是标题 span（无图标 span 占位）
      expect(titleSpan()?.textContent).toBe("terminal-0");
    });

    it("params.tabIcon 含 / → 渲染 img（URL/文件路径分支）", () => {
      const { iconImg } = renderTab({ tabIcon: "/claude.png" });
      expect(iconImg()).toBeTruthy();
      expect(iconImg()?.getAttribute("src")).toBe("/claude.png");
      expect(iconImg()?.getAttribute("alt")).toBe("页签图标");
    });

    it("params.tabIcon 为 emoji → 渲染 span 图标", () => {
      const { iconImg, getChildren } = renderTab({ tabIcon: "⚡" });
      expect(iconImg()).toBeNull();
      const icon = getChildren()[0] as HTMLElement;
      expect(icon.tagName).toBe("SPAN");
      expect(icon.textContent).toBe("⚡");
    });

    it("params 无 tabIcon 字段（undefined）→ 不崩溃、无图标", () => {
      const { iconImg, titleSpan } = renderTab({});
      expect(iconImg()).toBeNull();
      expect(titleSpan()?.textContent).toBe("terminal-0");
    });

    it("初始标题显示 api.title", () => {
      const { titleSpan } = renderTab({ title: "claude" });
      expect(titleSpan()?.textContent).toBe("claude");
    });
  });

  describe("动态更新（onDidParametersChange 真实事件路径）", () => {
    it("tabIcon 从 null 变为图片路径 → img 出现", () => {
      const { listeners, iconImg } = renderTab({ tabIcon: null });
      expect(iconImg()).toBeNull();
      emitParamsChange(listeners, { tabIcon: "/claude.png" });
      expect(iconImg()?.getAttribute("src")).toBe("/claude.png");
    });

    it("tabIcon 从 null 变为 emoji → span 图标出现", () => {
      const { listeners, iconImg, getChildren } = renderTab({ tabIcon: null });
      emitParamsChange(listeners, { tabIcon: "⚡" });
      expect(iconImg()).toBeNull();
      expect((getChildren()[0] as HTMLElement).textContent).toBe("⚡");
    });

    it("tabIcon 从图片变为 emoji → img 移除、span 出现", () => {
      const { listeners, iconImg, getChildren } = renderTab({ tabIcon: "/claude.png" });
      expect(iconImg()).toBeTruthy();
      emitParamsChange(listeners, { tabIcon: "✅" });
      expect(iconImg()).toBeNull();
      expect((getChildren()[0] as HTMLElement).textContent).toBe("✅");
    });

    it("tabIcon 从非空变为 null → 图标移除", () => {
      const { listeners, iconImg, getChildren } = renderTab({ tabIcon: "/claude.png" });
      emitParamsChange(listeners, { tabIcon: null });
      expect(iconImg()).toBeNull();
      // 第一个子元素回退为标题 span
      expect((getChildren()[0] as HTMLElement).textContent).toBe("terminal-0");
    });

    it("回归守卫：event.params.tabIcon 包裹结构不生效（真实结构为扁平 event.tabIcon）", () => {
      // 若生产代码误写成 event.params.tabIcon，此用例将失败（事件恒 undefined）
      const { listeners, iconImg } = renderTab({ tabIcon: null });
      emitParamsChange(listeners, { params: { tabIcon: "/wrapped.png" } });
      expect(iconImg()).toBeNull();
      // 扁平结构才生效
      emitParamsChange(listeners, { tabIcon: "/flat.png" });
      expect(iconImg()?.getAttribute("src")).toBe("/flat.png");
    });

    it("event 为 undefined → 不崩溃（undefined ?? null → 图标清空）", () => {
      const { listeners, iconImg } = renderTab({ tabIcon: "⚡" });
      act(() => {
        listeners.params.forEach((cb) => cb(undefined as unknown as Record<string, unknown>));
      });
      expect(iconImg()).toBeNull();
    });
  });

  describe("标题更新（onDidTitleChange）", () => {
    it("setTitle 后标题更新（回调收到 TitleEvent { title }）", () => {
      const { listeners, titleSpan } = renderTab({ title: "terminal-0" });
      expect(titleSpan()?.textContent).toBe("terminal-0");
      emitTitleChange(listeners, "claude");
      expect(titleSpan()?.textContent).toBe("claude");
    });

    it("标题更新不影响图标", () => {
      const { listeners, iconImg, titleSpan } = renderTab({ tabIcon: "/claude.png" });
      emitTitleChange(listeners, "claude-v2");
      expect(iconImg()?.getAttribute("src")).toBe("/claude.png");
      expect(titleSpan()?.textContent).toBe("claude-v2");
    });
  });

  describe("渲染属性", () => {
    it("img 类型：width=16 height=16 flexShrink=0", () => {
      const { iconImg } = renderTab({ tabIcon: "/claude.png" });
      expect(iconImg()?.getAttribute("width")).toBe("16");
      expect(iconImg()?.getAttribute("height")).toBe("16");
      expect((iconImg() as HTMLElement).style.flexShrink).toBe("0");
    });

    it("span 图标：fontSize=14 lineHeight=1 flexShrink=0", () => {
      const { getChildren } = renderTab({ tabIcon: "⚡" });
      const icon = getChildren()[0] as HTMLElement;
      expect(icon.style.fontSize).toBe("14px");
      expect(icon.style.lineHeight).toBe("1");
      expect(icon.style.flexShrink).toBe("0");
    });

    it("DOM 顺序：图标 → 标题 → 关闭按钮", () => {
      const { iconImg, titleSpan, closeBtn } = renderTab({ tabIcon: "/claude.png" });
      expect(iconImg()).toBeTruthy();
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

  describe("edge cases", () => {
    it("tabIcon 含反斜杠 → img 分支（Windows 路径）", () => {
      const { iconImg } = renderTab({ tabIcon: "C:\\icons\\claude.png" });
      expect(iconImg()?.getAttribute("src")).toBe("C:\\icons\\claude.png");
    });

    it("tabIcon 为 http: URL → img 分支", () => {
      const { iconImg } = renderTab({ tabIcon: "https://example.com/icon.png" });
      expect(iconImg()?.getAttribute("src")).toBe("https://example.com/icon.png");
    });

    it("tabIcon 为 data: URL → img 分支", () => {
      const { iconImg } = renderTab({ tabIcon: "data:image/png;base64,AAAA" });
      expect(iconImg()?.getAttribute("src")).toBe("data:image/png;base64,AAAA");
    });

    it("tabIcon 为空字符串 → 不渲染图标（falsy）", () => {
      const { iconImg } = renderTab({ tabIcon: "" });
      expect(iconImg()).toBeNull();
    });
  });

  describe("CLI logo（tabLogo，仅随 emoji 显示）", () => {
    it("emoji + tabLogo → img[alt='CLI 图标'] 存在（src/16×16/flexShrink）", () => {
      const { statusImg } = renderTab({ tabIcon: "⚡", tabLogo: "/cli-icons/claude.png" });
      expect(statusImg()).toBeTruthy();
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
      expect(statusImg()?.getAttribute("width")).toBe("16");
      expect(statusImg()?.getAttribute("height")).toBe("16");
      expect((statusImg() as HTMLElement).style.flexShrink).toBe("0");
    });

    it("DOM 顺序：emoji span → logo img → 标题 → 关闭按钮", () => {
      const { getChildren, statusImg, titleSpan, closeBtn } = renderTab({
        tabIcon: "⚡",
        tabLogo: "/cli-icons/claude.png",
      });
      const icon = getChildren()[0] as HTMLElement;
      expect(icon.tagName).toBe("SPAN");
      expect(icon.textContent).toBe("⚡");
      expect(getChildren()[1]).toBe(statusImg());
      expect(titleSpan()?.textContent).toBe("terminal-0");
      expect(closeBtn()?.getAttribute("title")).toBe("关闭");
    });

    it("tabIcon 为 null + tabLogo → 无 img（仅随 emoji，双清双保险）", () => {
      const { statusImg, iconImg } = renderTab({
        tabIcon: null,
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusImg()).toBeNull();
      expect(iconImg()).toBeNull();
    });

    it("动态更新：emitParamsChange 携 tabLogo → logo img 出现（扁平参数回归）", () => {
      const { listeners, statusImg } = renderTab({ tabIcon: null });
      expect(statusImg()).toBeNull();
      emitParamsChange(listeners, { tabIcon: "⚡", tabLogo: "/cli-icons/claude.png" });
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
    });

    it("动态更新：tabLogo 移除 → logo img 消失（emoji 保留）", () => {
      const { listeners, statusImg, getChildren } = renderTab({
        tabIcon: "⚡",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(statusImg()).toBeTruthy();
      emitParamsChange(listeners, { tabIcon: "⚡", tabLogo: null });
      expect(statusImg()).toBeNull();
      // emoji span 仍保留
      expect((getChildren()[0] as HTMLElement).textContent).toBe("⚡");
    });

    it("URL tabIcon（img 分支）与 tabLogo 并存 → 两个 img 互不干扰", () => {
      const { iconImg, statusImg } = renderTab({
        tabIcon: "/claude.png",
        tabLogo: "/cli-icons/claude.png",
      });
      expect(iconImg()?.getAttribute("src")).toBe("/claude.png");
      expect(statusImg()?.getAttribute("src")).toBe("/cli-icons/claude.png");
    });
  });
});
