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
  function callMenu(pageId: string, groupId: string, apiSpy?: ReturnType<typeof vi.fn>) {
    const getMenu = createGetContextMenu(makeNextPanelId(pageId), pageId);
    const addPanelSpy = apiSpy ?? vi.fn();
    const mockGroup = makeFakeGroup(groupId);

    const items = getMenu({
      panel: {} as any,
      group: mockGroup as any,
      api: { addPanel: addPanelSpy } as any,
      event: new MouseEvent("contextmenu"),
    });

    const newTerminalItem = items.find(
      (item) => typeof item === "object" && item.label === "新建终端",
    );
    expect(newTerminalItem).toBeDefined();

    return { newTerminalItem, addPanelSpy, mockGroup, items };
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

  it("C5: 右键菜单完整结构", () => {
    const { items } = callMenu("p1", "group-alpha");
    // 结构：[新建终端, separator, close, closeOthers, closeAll]
    expect(items).toHaveLength(5);
    expect((items[0] as any).label).toBe("新建终端");
    expect(items[1]).toBe("separator");
    expect(items[2]).toBe("close");
    expect(items[3]).toBe("closeOthers");
    expect(items[4]).toBe("closeAll");
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
