// workspace-header-actions.test.tsx — 分屏 + 按钮 & 右键菜单 addPanel 行为测试
//
// 验证：非聚焦分屏点击 + 按钮或右键"新建终端"时，新面板创建在点击的分屏
// 而非聚焦分屏。直接测试 createRightHeader/createGetContextMenu 工厂函数，
// 不渲染完整 Dockview 树。
//
// React StrictMode 双渲染导致 getByText/getByTitle 找到多个元素，
// 统一使用 container 取最后一个实例（两个实例行为相同）。
/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, within } from "@testing-library/react";
import { titleManager } from "../workspace/titleManager";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { DIM_FG } from "../theme";
import {
  createRightHeader,
  createGetContextMenu,
} from "../workspace/Workspace";

// ---- 辅助 ----

/** 生成每页递增的 panel ID（模拟 PageDockview.panelSeqRef） */
function makeNextPanelId(pageId: string) {
  let seq = 0;
  return () => `terminal-${pageId}-${seq++}`;
}

/** 创建 fake DockviewGroupPanel（满足类型，仅含 id 标识） */
function makeFakeGroup(id: string) {
  return { api: { id }, id } as unknown as Record<string, unknown>;
}

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 输入补空格 → "rgba(r, g, b, a)"） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  // 新方案 selection 类 token 为 rgba 形态，jsdom 输出 "rgba(r, g, b, a)"
  const m = hex.match(/^rgba\((\d+),(\d+),(\d+),([0-9.]+)\)$/);
  if (m) return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${m[4]})`;
  return hex;
}

/** 渲染 Header 组件并返回 helpers（规避 StrictMode getByText 多元素问题） */
function renderHeader(pageId: string, cwd: string, groupId: string) {
  const nextId = makeNextPanelId(pageId);
  const Header = createRightHeader(nextId, pageId, cwd);
  const addPanelSpy = vi.fn();
  const mockGroup = makeFakeGroup(groupId);

  const result = render(
    React.createElement(Header, {
      containerApi: { addPanel: addPanelSpy },
      group: mockGroup,
      api: {},
      panels: [],
      activePanel: undefined,
      isGroupActive: false,
      headerPosition: "top",
    } as any),
  );

  // StrictMode 双渲染：第一个元素 handler 已清理，取最后一个
  const clickPlus = () => {
    const btns = result.getAllByText("+");
    fireEvent.click(btns[btns.length - 1]);
  };

  return { ...result, addPanelSpy, mockGroup, clickPlus, nextId };
}

beforeEach(() => {
  titleManager.reset();
  TerminalRegistry._reset();
});

// ============================================================
// createRightHeader — + 按钮
// ============================================================

describe("createRightHeader", () => {
  it("R1: 渲染 + 按钮", () => {
    const { getAllByText } = renderHeader("p1", "/test", "group-alpha");
    expect(getAllByText("+")[0]).toBeTruthy();
  });

  it("R2: 按钮 title 为\"新建终端\"", () => {
    const { getAllByTitle } = renderHeader("p1", "/test", "group-alpha");
    expect(getAllByTitle("新建终端")[0]).toBeTruthy();
  });

  it("R9: + 按钮尺寸规格 22px/圆角 4/fg-3（TAB-04）", () => {
    const { getAllByText } = renderHeader("p1", "/test", "group-alpha");
    const btn = getAllByText("+").slice(-1)[0] as HTMLButtonElement;
    expect(btn.style.width).toBe("22px");
    expect(btn.style.height).toBe("22px");
    expect(btn.style.borderRadius).toBe("4px");
    expect(btn.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("R3: 点击 + 调用 addPanel", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/test", "group-alpha");
    clickPlus();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
  });

  it("R4: addPanel 包含 position.referenceGroup", () => {
    const { addPanelSpy, mockGroup, clickPlus } = renderHeader("p1", "/test", "group-alpha");
    clickPlus();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).toBeDefined();
    expect(options.position.referenceGroup).toBe(mockGroup);
  });

  it("R5: position.referenceGroup 指向传入的 group", () => {
    const { addPanelSpy, mockGroup, clickPlus } = renderHeader("p1", "/test", "group-alpha");
    clickPlus();

    expect(addPanelSpy.mock.calls[0][0].position.referenceGroup).toBe(mockGroup);
  });

  it("R6: 不传 position 的旧行为不再存在", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/test", "group-alpha");
    clickPlus();

    const options = addPanelSpy.mock.calls[0][0];
    // position 字段必须存在（非 undefined）
    expect(options.position).not.toBeUndefined();
    expect(options.position.referenceGroup).toBeDefined();
  });

  it("R7: 多分屏——不同 group 各自传正确的 referenceGroup", () => {
    const mockGroupA = makeFakeGroup("group-left");
    const mockGroupB = makeFakeGroup("group-right");
    const spyA = vi.fn();
    const spyB = vi.fn();

    // 两个独立 Header 实例，模拟左右分屏
    const HeaderA = createRightHeader(makeNextPanelId("p1"), "p1", "/test");
    const HeaderB = createRightHeader(makeNextPanelId("p1"), "p1", "/test");

    const ra = render(
      React.createElement(HeaderA, {
        containerApi: { addPanel: spyA }, group: mockGroupA,
        api: {}, panels: [],
        activePanel: undefined, isGroupActive: false, headerPosition: "top",
      } as any),
    );
    const rb = render(
      React.createElement(HeaderB, {
        containerApi: { addPanel: spyB }, group: mockGroupB,
        api: {}, panels: [],
        activePanel: undefined, isGroupActive: false, headerPosition: "top",
      } as any),
    );

    // 用 within 限域到各自 container，避免跨 render 查询污染
    const btnsA = within(ra.container).getAllByText("+");
    const btnsB = within(rb.container).getAllByText("+");
    fireEvent.click(btnsA[btnsA.length - 1]);
    fireEvent.click(btnsB[btnsB.length - 1]);

    expect(spyA).toHaveBeenCalledTimes(1);
    expect(spyB).toHaveBeenCalledTimes(1);
    expect(spyA.mock.calls[0][0].position.referenceGroup).toBe(mockGroupA);
    expect(spyB.mock.calls[0][0].position.referenceGroup).toBe(mockGroupB);
  });

  it("R8: addPanel 其余参数不变", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/home/test", "group-alpha");
    clickPlus();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.component).toBe("terminal");
    expect(options.renderer).toBe("always");
    expect(options.title).toMatch(/^terminal-/);
    expect(options.params.panelId).toMatch(/^terminal-p1-/);
    expect(options.params.cwd).toBe("/home/test");
  });
});

// ============================================================
// createGetContextMenu — 右键菜单"新建终端"
// ============================================================

describe("createGetContextMenu", () => {
  function callMenu(
    pageId: string,
    groupId: string,
    options?: { panelComponent?: string; apiSpy?: ReturnType<typeof vi.fn> },
  ) {
    const onRenameRequestSpy = vi.fn();
    const getMenu = createGetContextMenu(
      makeNextPanelId(pageId), pageId, onRenameRequestSpy,
    );
    const addPanelSpy = options?.apiSpy ?? vi.fn();
    const mockGroup = makeFakeGroup(groupId);
    // fake 面板：id 与 TerminalRegistry 种子键一致；view.contentComponent 判终端
    const fakePanel = {
      id: `terminal-${pageId}-0`,
      title: "terminal-0",
      params: {},
      view: { contentComponent: options?.panelComponent ?? "terminal" },
      api: { setTitle: vi.fn(), updateParameters: vi.fn() },
    };

    const items = getMenu({
      panel: fakePanel as any,
      group: mockGroup as any,
      api: { addPanel: addPanelSpy } as any,
      event: new MouseEvent("contextmenu"),
    });

    const newTerminalItem = items.find(
      (item) => typeof item === "object" && item.label === "新建终端",
    );
    expect(newTerminalItem).toBeDefined();

    const renameItem = items.find(
      (item) => typeof item === "object" && item.label === "重命名",
    );

    return { newTerminalItem, renameItem, addPanelSpy, mockGroup, items, onRenameRequestSpy, fakePanel };
  }

  it("C1: 菜单包含\"新建终端\"项", () => {
    const { newTerminalItem } = callMenu("p1", "group-alpha");
    expect(newTerminalItem).toBeDefined();
  });

  it("C2: \"新建终端\" action 调用 addPanel", () => {
    const { newTerminalItem, addPanelSpy } = callMenu("p1", "group-alpha");
    (newTerminalItem as any).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
  });

  it("C3: addPanel 含 position: { referenceGroup: params.group }", () => {
    const { newTerminalItem, addPanelSpy, mockGroup } = callMenu("p1", "group-beta");
    (newTerminalItem as any).action();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).toBeDefined();
    expect(options.position.referenceGroup).toBe(mockGroup);
  });

  it("C4: 不传 position 的旧行为不再存在", () => {
    const { newTerminalItem, addPanelSpy } = callMenu("p1", "group-beta");
    (newTerminalItem as any).action();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).not.toBeUndefined();
  });

  it("C5: 终端右键菜单完整结构（含重命名项）", () => {
    const { items } = callMenu("p1", "group-alpha");
    // 终端结构：[新建终端, separator, 重命名, separator, 关闭, 关闭其他, 关闭全部]
    //（OV-04：库内建 close 字符串改自定义项——关闭类 = danger 危险项，UI-802）
    expect(items).toHaveLength(7);
    expect((items[0] as any).label).toBe("新建终端");
    expect(items[1]).toBe("separator");
    expect((items[2] as any).label).toBe("重命名");
    expect(items[3]).toBe("separator");
    expect((items[4] as any).componentProps).toMatchObject({ label: "关闭", danger: true });
    expect((items[5] as any).componentProps).toMatchObject({ label: "关闭其他", danger: true });
    expect((items[6] as any).componentProps).toMatchObject({ label: "关闭全部", danger: true });
  });

  it("C6: 非终端面板菜单无重命名项（结构保持 5 项）", () => {
    const { items, renameItem } = callMenu("p1", "group-alpha", {
      panelComponent: "editor",
    });
    expect(renameItem).toBeUndefined();
    expect(items).toHaveLength(5);
    expect((items[2] as any).componentProps).toMatchObject({ label: "关闭", danger: true });
    expect((items[3] as any).componentProps).toMatchObject({ label: "关闭其他", danger: true });
    expect((items[4] as any).componentProps).toMatchObject({ label: "关闭全部", danger: true });
  });

  it("C7: 重命名项 action 调 onRenameRequest(panel)", () => {
    const { renameItem, onRenameRequestSpy, fakePanel } = callMenu("p1", "group-alpha");
    (renameItem as any).action();
    expect(onRenameRequestSpy).toHaveBeenCalledTimes(1);
    expect(onRenameRequestSpy).toHaveBeenCalledWith(fakePanel);
  });

  it("C8: claude 运行中（agentSession 存在）→ 重命名项 disabled", () => {
    TerminalRegistry.register("terminal-p1-0", {} as any);
    TerminalRegistry.setAgentSession("terminal-p1-0", { sessionId: "s1" } as any);
    const { renameItem } = callMenu("p1", "group-alpha");
    expect((renameItem as any).disabled).toBe(true);
  });

  it("C9: 无 agentSession → 重命名项可点", () => {
    TerminalRegistry.register("terminal-p1-0", {} as any);
    const { renameItem } = callMenu("p1", "group-alpha");
    expect((renameItem as any).disabled).not.toBe(true);
  });

  it("C10: agentSession 为空对象（已退出残留）→ 重命名项可点", () => {
    TerminalRegistry.register("terminal-p1-0", {} as any);
    TerminalRegistry.setAgentSession("terminal-p1-0", null);
    const { renameItem } = callMenu("p1", "group-alpha");
    expect((renameItem as any).disabled).not.toBe(true);
  });

  it("C11: 连续两次构建菜单后执行新建，terminal-N 编号不跳（FE-04）", () => {
    // FE-04 回归：nextPanelId() 延迟到「新建终端」action 执行时才分配——
    // 右键弹菜单（构建菜单）不点击不消耗编号，两次构建后再点仍从 terminal-p1-0 起
    const nextId = makeNextPanelId("p1");
    const getMenu = createGetContextMenu(nextId, "p1", vi.fn());
    const addPanelSpy = vi.fn();
    const mockGroup = makeFakeGroup("group-alpha");
    const fakePanel = {
      id: "terminal-p1-0",
      title: "terminal-0",
      params: {},
      view: { contentComponent: "terminal" },
      api: { setTitle: vi.fn(), updateParameters: vi.fn() },
    };
    const menuParams = {
      panel: fakePanel as any,
      group: mockGroup as any,
      api: { addPanel: addPanelSpy } as any,
      event: new MouseEvent("contextmenu"),
    };
    const findNewTerminal = (items: ReturnType<typeof getMenu>) =>
      items.find((item) => typeof item === "object" && item.label === "新建终端") as any;

    // 连续两次构建菜单（右键弹菜单但不点击）
    const firstItems = getMenu(menuParams);
    const secondItems = getMenu(menuParams);

    // 执行第二次构建的菜单——编号不因两次构建而跳号
    findNewTerminal(secondItems).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
    expect(addPanelSpy.mock.calls[0][0].id).toBe("terminal-p1-0");
    expect(addPanelSpy.mock.calls[0][0].params.panelId).toBe("terminal-p1-0");

    // 再执行第一次构建的菜单——编号连续递增不重复
    findNewTerminal(firstItems).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(2);
    expect(addPanelSpy.mock.calls[1][0].id).toBe("terminal-p1-1");
  });
});

// ============================================================
// Watermark 回归测试 — addPanel 不传 position
// ============================================================

describe("Watermark 回归", () => {
  function renderWatermark(pageId: string, cwd: string) {
    const nextId = makeNextPanelId(pageId);
    const addPanelSpy = vi.fn();

    // Watermark 等价于：createWatermark(nextId, pageId, cwd)
    const Watermark: React.FC<any> = ({ containerApi }) => (
      <div>
        <button
          onClick={() => {
            const id = nextId();
            const title = titleManager.getTerminalTitle(pageId);
            containerApi.addPanel({
              id, component: "terminal", title,
              params: { panelId: id, cwd }, renderer: "always",
            });
          }}
        >新建终端</button>
      </div>
    );

    const result = render(
      React.createElement(Watermark, {
        containerApi: { addPanel: addPanelSpy },
      } as any),
    );

    // StrictMode 双渲染：取最后一个元素
    const clickBtn = () => {
      const btns = result.getAllByText("新建终端");
      fireEvent.click(btns[btns.length - 1]);
    };

    return { ...result, addPanelSpy, clickBtn };
  }

  it("W1: Watermark addPanel 不传 position", () => {
    const { addPanelSpy, clickBtn } = renderWatermark("p1", "/test");
    clickBtn();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).toBeUndefined();
  });

  it("W2: Watermark 仍然正常调用 addPanel", () => {
    const { addPanelSpy, clickBtn } = renderWatermark("p1", "/home/user");
    clickBtn();

    expect(addPanelSpy).toHaveBeenCalledTimes(1);
    const options = addPanelSpy.mock.calls[0][0];
    expect(options.component).toBe("terminal");
    expect(options.renderer).toBe("always");
    expect(options.params.cwd).toBe("/home/user");
  });

  it("W3: Watermark 传入 group=undefined 时不崩溃", () => {
    const nextId = makeNextPanelId("p1");
    const addPanelSpy = vi.fn();
    const Watermark: React.FC<any> = ({ containerApi }) => (
      <div>
        <button
          onClick={() => {
            const id = nextId();
            containerApi.addPanel({
              id, component: "terminal",
              title: titleManager.getTerminalTitle("p1"),
              params: { panelId: id, cwd: "/test" }, renderer: "always",
            });
          }}
        >新建终端</button>
      </div>
    );

    const { getAllByText } = render(
      React.createElement(Watermark, {
        containerApi: { addPanel: addPanelSpy },
        group: undefined,
      } as any),
    );

    expect(() =>
      // StrictMode 双渲染：取最后一个元素
      fireEvent.click(getAllByText("新建终端").slice(-1)[0]),
    ).not.toThrow();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
  });
});
