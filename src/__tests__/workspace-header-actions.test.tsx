// workspace-header-actions.test.tsx — 组头 + 按钮 / Watermark / 右键菜单新建终端
// 行为测试（CP-004 单宿主页组协议适配）
//
// 验证：+ 钮/右键「新建终端」/Watermark 均经 addTerminalPanel 共享工厂——
// addPanel 显式 position.referenceGroup = 目标页组（页组 id 字符串形态）、
// 面板 id 页前缀协议（{pageId}:terminal-N）。直接测生产工厂与组件，
// 不渲染完整 Dockview 树。
//
// React StrictMode 双渲染导致 getByText/getByTitle 找到多个元素，
// 统一使用 container 取最后一个实例（两个实例行为相同）。
/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, within, screen } from "@testing-library/react";

// ─── Hoisted clipboard mock（「复制相对路径」用例断言写剪贴板）───
const mocks = vi.hoisted(() => {
  const mockWriteText = vi.fn();
  return {
    mockWriteText,
    resetClipboard() {
      mockWriteText.mockReset();
      mockWriteText.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/clipboard", () => ({
  writeText: mocks.mockWriteText,
  readText: vi.fn().mockResolvedValue(""),
}));

import { titleManager } from "../workspace/titleManager";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { useLayout } from "../stores/layout";
import { resetTerminalPanelSeq } from "../lib/panelId";
import { DIM_FG } from "../theme";
import {
  createRightHeader,
  createTabMenuItems,
} from "../workspace/Workspace";
import { createWatermark } from "../workspace/PageDockviewHost";
import { pageGroupId } from "../workspace/pageGroups";

// ---- 辅助 ----

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

/**
 * fake 宿主 api：addPanel spy + getGroup（addTerminalPanel 的页组存在性守卫）——
 * 目标页组（page-{pageId}）视为已挂载。
 */
function makeFakeHost(pageIds: string[], addPanelSpy: ReturnType<typeof vi.fn> = vi.fn()) {
  const pageGroupIds = new Set(pageIds.map((p) => pageGroupId(p)));
  const host = {
    addPanel: addPanelSpy,
    getGroup: (id: string) => (pageGroupIds.has(id) ? { id } : undefined),
  };
  return { host, addPanelSpy };
}

/** 渲染 Header 组件并返回 helpers（规避 StrictMode getByText 多元素问题） */
function renderHeader(pageId: string, _cwd: string, groupId: string, activePageId = pageId) {
  useLayout.setState({ activePageId: activePageId });
  const { host, addPanelSpy } = makeFakeHost([activePageId]);
  const Header = createRightHeader(() => host as any);
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

  return { ...result, addPanelSpy, mockGroup, clickPlus };
}

beforeEach(() => {
  titleManager.reset();
  TerminalRegistry._reset();
  resetTerminalPanelSeq();
  useLayout.setState({ activePageId: null });
  mocks.resetClipboard();
});

// ============================================================
// createRightHeader — + 按钮
// ============================================================

describe("createRightHeader", () => {
  it("R1: 渲染 + 按钮", () => {
    const { getAllByText } = renderHeader("p1", "/test", "group-alpha", "p1");
    expect(getAllByText("+")[0]).toBeTruthy();
  });

  it("R2: 按钮 title 为\"新建终端\"", () => {
    const { getAllByTitle } = renderHeader("p1", "/test", "group-alpha", "p1");
    expect(getAllByTitle("新建终端")[0]).toBeTruthy();
  });

  it("R9: + 按钮尺寸规格 22px/圆角 4/fg-3（TAB-04）", () => {
    const { getAllByText } = renderHeader("p1", "/test", "group-alpha", "p1");
    const btn = getAllByText("+").slice(-1)[0] as HTMLButtonElement;
    expect(btn.style.width).toBe("22px");
    expect(btn.style.height).toBe("22px");
    expect(btn.style.borderRadius).toBe("4px");
    expect(btn.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("R3: 点击 + 调用 addPanel（经 addTerminalPanel——getGroup 守卫后落活跃页组）", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/test", "group-alpha", "p1");
    clickPlus();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
  });

  it("R4/R5/R6: addPanel position.referenceGroup = 目标页组 id（页组协议字符串形态）", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/test", "page-p1", "p1");
    clickPlus();

    const options = addPanelSpy.mock.calls[0][0];
    // position 必须存在且指向页组 id（跨页组拖拽/落点确定性——组 id 协议形态）
    expect(options.position).not.toBeUndefined();
    expect(options.position.referenceGroup).toBe("page-p1");
  });

  it("R7: 组头属主页优先——非活跃页组 id 不可解析时兜底活跃页；页组 id 解析优先", () => {
    const mockGroupA = makeFakeGroup("group-left"); // 非页组 → 兜底活跃页
    const mockGroupB = makeFakeGroup(pageGroupId("p1")); // 页组 → 组属主页
    const spyA = vi.fn();
    const spyB = vi.fn();
    useLayout.setState({ activePageId: "p1" });
    const { host } = makeFakeHost(["p1"], spyA);
    const { host: hostB } = makeFakeHost(["p1"], spyB);

    const HeaderA = createRightHeader(() => host as any);
    const HeaderB = createRightHeader(() => hostB as any);

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

    const btnsA = within(ra.container).getAllByText("+");
    const btnsB = within(rb.container).getAllByText("+");
    fireEvent.click(btnsA[btnsA.length - 1]);
    fireEvent.click(btnsB[btnsB.length - 1]);

    expect(spyA.mock.calls[0][0].position.referenceGroup).toBe("page-p1");
    expect(spyB.mock.calls[0][0].position.referenceGroup).toBe("page-p1");
  });

  it("R8: addPanel 参数——页前缀协议 id + renderer always + 标题 terminal-N", () => {
    const { addPanelSpy, clickPlus } = renderHeader("p1", "/home/test", "page-p1", "p1");
    clickPlus();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.component).toBe("terminal");
    expect(options.renderer).toBe("always");
    expect(options.title).toMatch(/^terminal-/);
    // CP-004 页前缀协议：id/params.panelId = {pageId}:terminal-N
    expect(options.id).toBe("p1:terminal-0");
    expect(options.params.panelId).toBe("p1:terminal-0");
    expect(options.params.cwd).toBeUndefined();
  });

  it("R10: 目标页组未挂载（getGroup 缺失）→ 静默不 addPanel（addTerminalPanel 守卫）", () => {
    useLayout.setState({ activePageId: "p1" });
    const addPanelSpy = vi.fn();
    const host = { addPanel: addPanelSpy, getGroup: () => undefined };
    const Header = createRightHeader(() => host as any);
    const result = render(
      React.createElement(Header, {
        containerApi: { addPanel: addPanelSpy },
        group: makeFakeGroup("page-p1"),
        api: {}, panels: [], activePanel: undefined,
        isGroupActive: false, headerPosition: "top",
      } as any),
    );
    const btns = result.getAllByText("+");
    fireEvent.click(btns[btns.length - 1]);
    expect(addPanelSpy).not.toHaveBeenCalled();
  });
});

// ============================================================
// createTabMenuItems — 右键菜单"新建终端"
// ============================================================

describe("createTabMenuItems", () => {
  function callMenu(
    pageId: string,
    groupId: string,
    options?: {
      panelComponent?: string;
      apiSpy?: ReturnType<typeof vi.fn>;
      /** 文件页签参数（params.filePath 判文件型） */
      filePath?: string;
      /** 工厂第 4 参（项目根,「复制相对路径」基准） */
      rootPath?: string;
    },
  ) {
    const onRenameRequestSpy = vi.fn();
    const addPanelSpy = options?.apiSpy ?? vi.fn();
    const mockGroup = makeFakeGroup(groupId);
    // 单宿主形态：工厂收 (getApi, pageId|null, onRenameRequest, projectRootPath)，
    // 返回 (panel) => items 构建器；「新建终端」action 经 addTerminalPanel 落
    // 面板属主页组（panel.id 页前缀解析，兜底 menuPageId）
    const { host } = makeFakeHost([pageId], addPanelSpy);
    const getMenu = createTabMenuItems(
      () => host as any, pageId, onRenameRequestSpy, options?.rootPath,
    );
    // fake 面板：id 页前缀协议形态（panel 属主页 = pageId）；view.contentComponent
    // 判终端；api.group 供关闭族（组快照取自右键传入面板结构）
    const fakePanel = {
      id: `${pageId}:terminal-0`,
      title: "terminal-0",
      params: options?.filePath ? { filePath: options.filePath } : {},
      view: { contentComponent: options?.panelComponent ?? "terminal" },
      api: {
        setTitle: vi.fn(), updateParameters: vi.fn(),
        close: vi.fn(), group: mockGroup,
      },
    };

    const items = getMenu(fakePanel as any);

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
    const { newTerminalItem } = callMenu("p1", "page-p1");
    expect(newTerminalItem).toBeDefined();
  });

  it("C2: \"新建终端\" action 调用 addPanel", () => {
    const { newTerminalItem, addPanelSpy } = callMenu("p1", "page-p1");
    (newTerminalItem as any).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
  });

  it("C3/C4: addPanel position.referenceGroup = 面板属主页组 id（页组协议字符串）", () => {
    const { newTerminalItem, addPanelSpy } = callMenu("p1", "group-beta");
    (newTerminalItem as any).action();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).toBeDefined();
    expect(options.position.referenceGroup).toBe("page-p1");
    // 页前缀协议 id（面板属主页 p1——localId 独立计数从 0 起）
    expect(options.id).toBe("p1:terminal-0");
    expect(options.params.panelId).toBe("p1:terminal-0");
  });

  it("C5: 终端右键菜单完整结构（含重命名项）", () => {
    const { items } = callMenu("p1", "page-p1");
    // 终端结构：[新建终端, separator, 重命名, separator, 关闭, 关闭其他, 关闭全部]
    //（OV-04：库内建 close 字符串改自定义项——关闭类 = danger 危险项，UI-802）
    expect(items).toHaveLength(7);
    expect((items[0] as any).label).toBe("新建终端");
    expect(items[1]).toBe("separator");
    expect((items[2] as any).label).toBe("重命名");
    expect(items[3]).toBe("separator");
    expect((items[4] as any)).toMatchObject({ label: "关闭", danger: true });
    expect((items[5] as any)).toMatchObject({ label: "关闭其他", danger: true });
    expect((items[6] as any)).toMatchObject({ label: "关闭全部", danger: true });
  });

  it("C6: 非终端面板菜单无重命名项（结构保持 5 项）", () => {
    const { items, renameItem } = callMenu("p1", "page-p1", {
      panelComponent: "editor",
    });
    expect(renameItem).toBeUndefined();
    expect(items).toHaveLength(5);
    expect((items[2] as any)).toMatchObject({ label: "关闭", danger: true });
    expect((items[3] as any)).toMatchObject({ label: "关闭其他", danger: true });
    expect((items[4] as any)).toMatchObject({ label: "关闭全部", danger: true });
  });

  it("C7: 重命名项 action 调 onRenameRequest(panel)", () => {
    const { renameItem, onRenameRequestSpy, fakePanel } = callMenu("p1", "page-p1");
    (renameItem as any).action();
    expect(onRenameRequestSpy).toHaveBeenCalledTimes(1);
    expect(onRenameRequestSpy).toHaveBeenCalledWith(fakePanel);
  });

  it("C8: claude 运行中（agentSession 存在）→ 重命名项 disabled", () => {
    TerminalRegistry.register("p1:terminal-0", {} as any);
    TerminalRegistry.setAgentSession("p1:terminal-0", { sessionId: "s1" } as any);
    const { renameItem } = callMenu("p1", "page-p1");
    expect((renameItem as any).disabled).toBe(true);
  });

  it("C9: 无 agentSession → 重命名项可点", () => {
    TerminalRegistry.register("p1:terminal-0", {} as any);
    const { renameItem } = callMenu("p1", "page-p1");
    expect((renameItem as any).disabled).not.toBe(true);
  });

  it("C10: agentSession 为空对象（已退出残留）→ 重命名项可点", () => {
    TerminalRegistry.register("p1:terminal-0", {} as any);
    TerminalRegistry.setAgentSession("p1:terminal-0", null);
    const { renameItem } = callMenu("p1", "page-p1");
    expect((renameItem as any).disabled).not.toBe(true);
  });

  it("C11: 连续两次构建菜单后执行新建，terminal-N 编号不跳（FE-04）", () => {
    // FE-04 回归：localId 分配延迟到「新建终端」action 执行时（lib/panelId 每页
    // 计数）——右键弹菜单（构建菜单）不点击不消耗编号，两次构建后再点仍从
    // terminal-0 起；页前缀协议 id = p1:terminal-N
    const { host, addPanelSpy } = makeFakeHost(["p1"]);
    const getMenu = createTabMenuItems(() => host as any, "p1", vi.fn());
    const mockGroup = makeFakeGroup("page-p1");
    const fakePanel = {
      id: "p1:terminal-0",
      title: "terminal-0",
      params: {},
      view: { contentComponent: "terminal" },
      api: {
        setTitle: vi.fn(), updateParameters: vi.fn(),
        close: vi.fn(), group: mockGroup,
      },
    };
    const findNewTerminal = (items: ReturnType<typeof getMenu>) =>
      items.find((item) => typeof item === "object" && item.label === "新建终端") as any;

    // 连续两次构建菜单（右键弹菜单但不点击）
    const firstItems = getMenu(fakePanel as any);
    const secondItems = getMenu(fakePanel as any);

    // 执行第二次构建的菜单——编号不因两次构建而跳号
    findNewTerminal(secondItems).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(1);
    expect(addPanelSpy.mock.calls[0][0].id).toBe("p1:terminal-0");
    expect(addPanelSpy.mock.calls[0][0].params.panelId).toBe("p1:terminal-0");

    // 再执行第一次构建的菜单——编号连续递增不重复
    findNewTerminal(firstItems).action();
    expect(addPanelSpy).toHaveBeenCalledTimes(2);
    expect(addPanelSpy.mock.calls[1][0].id).toBe("p1:terminal-1");
  });

  it("C12: 文件页签右键菜单结构——头部「复制相对路径」+separator，其余项保持", () => {
    const { items } = callMenu("p1", "page-p1", {
      panelComponent: "editor",
      filePath: "C:/proj/src/a.ts",
      rootPath: "C:/proj",
    });
    // 文件结构：[复制相对路径, separator, 新建终端, separator, 关闭, 关闭其他, 关闭全部]
    expect(items).toHaveLength(7);
    expect((items[0] as any)).toMatchObject({ label: "复制相对路径" });
    expect(items[1]).toBe("separator");
    expect((items[2] as any).label).toBe("新建终端");
    expect(items[3]).toBe("separator");
    expect((items[4] as any)).toMatchObject({ label: "关闭", danger: true });
    expect((items[5] as any)).toMatchObject({ label: "关闭其他", danger: true });
    expect((items[6] as any)).toMatchObject({ label: "关闭全部", danger: true });
  });

  it("C13: 点击「复制相对路径」→ writeText(相对项目根路径)", () => {
    const { items } = callMenu("p1", "page-p1", {
      panelComponent: "editor",
      filePath: "C:/proj/src/a.ts",
      rootPath: "C:/proj",
    });
    (items[0] as any).action();
    expect(mocks.mockWriteText).toHaveBeenCalledTimes(1);
    expect(mocks.mockWriteText).toHaveBeenCalledWith("src/a.ts");
  });

  it("C14: 文件在项目根外 → 点击兜底 writeText(完整绝对路径)", () => {
    const { items } = callMenu("p1", "page-p1", {
      panelComponent: "gitshow",
      filePath: "C:/else/a.ts",
      rootPath: "C:/proj",
    });
    (items[0] as any).action();
    expect(mocks.mockWriteText).toHaveBeenCalledWith("C:/else/a.ts");
  });

  it("C15: 工厂未传项目根 → 点击兜底绝对路径，不抛异常", () => {
    const { items } = callMenu("p1", "page-p1", {
      panelComponent: "editor",
      filePath: "C:/proj/a.ts",
    });
    expect(() => {
      (items[0] as any).action();
    }).not.toThrow();
    expect(mocks.mockWriteText).toHaveBeenCalledWith("C:/proj/a.ts");
  });

  it("C16: writeText 被拒 → 仅 console.error，无未处理拒绝", async () => {
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.mockWriteText.mockRejectedValueOnce(new Error("clipboard denied"));
    const { items } = callMenu("p1", "page-p1", {
      panelComponent: "editor",
      filePath: "C:/proj/a.ts",
      rootPath: "C:/proj",
    });
    (items[0] as any).action();

    await vi.waitFor(() => {
      expect(consoleErrorSpy).toHaveBeenCalled();
    });
    expect(consoleErrorSpy.mock.calls[0][0]).toContain("复制相对路径失败");
    consoleErrorSpy.mockRestore();
  });
});

// ============================================================
// Watermark — 空页组「新建终端」（落活跃页组，显式页组 position）
// ============================================================

describe("Watermark 回归（页组协议）", () => {
  /** 渲染生产 createWatermark 组件（TQ-A-04：直测生产实现） */
  function renderRealWatermark(pageId: string) {
    useLayout.setState({ activePageId: pageId });
    const { host, addPanelSpy } = makeFakeHost([pageId]);
    const Watermark = createWatermark(() => host as any);
    render(<Watermark containerApi={{ addPanel: addPanelSpy } as never} />);
    // StrictMode 双渲染：取最后一个按钮实例
    const clickBtn = () => {
      const btns = screen.getAllByRole("button", { name: "新建终端" });
      fireEvent.click(btns[btns.length - 1]);
    };
    return { addPanelSpy, clickBtn };
  }

  it("W1: Watermark addPanel 显式落活跃页组（position.referenceGroup = page-{pageId}）", () => {
    const { addPanelSpy, clickBtn } = renderRealWatermark("p1");
    clickBtn();

    const options = addPanelSpy.mock.calls[0][0];
    expect(options.position).not.toBeUndefined();
    expect(options.position.referenceGroup).toBe("page-p1");
  });

  it("W2: Watermark 正常调用 addPanel——页前缀协议 id + 常驻 renderer", () => {
    const { addPanelSpy, clickBtn } = renderRealWatermark("p1");
    clickBtn();

    expect(addPanelSpy).toHaveBeenCalledTimes(1);
    const options = addPanelSpy.mock.calls[0][0];
    expect(options.component).toBe("terminal");
    expect(options.renderer).toBe("always");
    expect(options.id).toBe("p1:terminal-0");
    expect(options.params.panelId).toBe("p1:terminal-0");
  });

  it("W3: 无活跃页时点击 → 不 addPanel（无落点）", () => {
    useLayout.setState({ activePageId: null });
    const addPanelSpy = vi.fn();
    const host = { addPanel: addPanelSpy, getGroup: () => undefined };
    const Watermark = createWatermark(() => host as any);
    render(<Watermark containerApi={{ addPanel: addPanelSpy } as never} />);
    const btns = screen.getAllByRole("button", { name: "新建终端" });
    fireEvent.click(btns[btns.length - 1]);
    expect(addPanelSpy).not.toHaveBeenCalled();
  });
});
