// default-layout-format.test.ts — makeEmptyLayout 格式验证
//
// 验证 NavTree.tsx 中 makeEmptyLayout 产出空布局对象，
// 新建页面不包含任何默认面板（terminal 等），由 Watermark 组件接管显示。
//
// WRK-11②：补"NavTree 实际使用 makeEmptyLayout"断言——渲染真实 NavTree
// （NAV-06 承接约定：SidebarTree 退役后 NavTree 承接新建项目/页面 CRUD），
// 行为断言新建项目/页面的 layout 为 makeEmptyLayout 产物（空对象）。
// 注：makeEmptyLayout 是 NavTree.tsx 同文件局部函数（handleAddProject/handleNewPage
// 直接调用），vi.mock 替换模块导出拦截不到局部函数引用——故以行为断言替代调用点 spy。

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted：dialog mock ───
const mocks = vi.hoisted(() => {
  const mockOpenDialog = vi.fn();
  return { mockOpenDialog };
});

vi.mock("../ipc/dialog", () => ({
  open: mocks.mockOpenDialog,
}));

import { makeEmptyLayout, NavTree } from "../features/navTree/NavTree";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

describe("makeEmptyLayout 空布局验证", () => {
  it("T1: 返回值为空对象", () => {
    const layout = makeEmptyLayout();
    expect(layout).toEqual({});
  });

  it("T2: 两次调用返回独立对象（修改一个不影响另一个）", () => {
    const a = makeEmptyLayout() as Record<string, unknown>;
    const b = makeEmptyLayout() as Record<string, unknown>;

    (a as Record<string, unknown>).mutated = true;
    expect(a.mutated).toBe(true);
    expect((b as Record<string, unknown>).mutated).toBeUndefined();
  });

  it("T3: JSON 序列化+反序列化往返后仍为 {}", () => {
    const layout = makeEmptyLayout();
    const json = JSON.stringify(layout);
    expect(json).toBe("{}");
    const restored = JSON.parse(json);
    expect(restored).toEqual({});
  });

  it("T4: Object.keys 长度为 0", () => {
    const layout = makeEmptyLayout();
    expect(Object.keys(layout)).toHaveLength(0);
  });

  it("T5: 返回值非 null", () => {
    const layout = makeEmptyLayout();
    expect(layout).not.toBeNull();
  });

  it("T6: 返回值为纯对象（constructor === Object）", () => {
    const layout = makeEmptyLayout();
    expect(layout.constructor).toBe(Object);
  });

  it("T7: 无原型链属性污染", () => {
    const layout = makeEmptyLayout();
    // 仅自有属性（非继承）
    expect(Object.prototype.hasOwnProperty.call(layout, "toString")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(layout, "__proto__")).toBe(false);
  });

  it("T8: typeof 为 'object'", () => {
    const layout = makeEmptyLayout();
    expect(typeof layout).toBe("object");
    // 排除数组和 null
    expect(Array.isArray(layout)).toBe(false);
    expect(layout).not.toBeNull();
  });
});

// ─── WRK-11②：NavTree 实际使用 makeEmptyLayout（真实组件 + 真实 stores）───

function renderNavTree() {
  return render(
    React.createElement(NavTree),
  );
}

describe("NavTree 实际使用 makeEmptyLayout（WRK-11②，NAV-06 承接）", () => {
  beforeEach(() => {
    cleanup();
    mocks.mockOpenDialog.mockReset();
    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: null });
  });

  it("T9: 新建项目流程产出 makeEmptyLayout 产物——page.layout 为空对象", async () => {
    mocks.mockOpenDialog.mockResolvedValueOnce("C:\\dev\\mock-proj");

    const { getAllByTitle } = renderNavTree();
    fireEvent.click(getAllByTitle("添加项目")[0]);

    await waitFor(() => {
      expect(Object.values(useProjects.getState().projects)).toHaveLength(1);
    }, { timeout: 3000 });

    // 行为断言：新页面 layout 为 makeEmptyLayout 产物（空对象）。
    // NavTree.handleAddProject 经 makeEmptyLayout() 构造 layout，
    // 真实组件 + 真实 makeEmptyLayout 全链路在此验证
    const proj = Object.values(useProjects.getState().projects)[0];
    expect(proj.pages[0].layout).toEqual({});
  });

  it("T10: 右键新建操作页面同样产出空布局（makeEmptyLayout 契约）", () => {
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "测试项目",
          rootPath: "C:\\test",
          pages: [
            { pageId: "page-1", name: "操作页面 1", layout: {}, cwd: "C:\\test", createdAt: 1, lastAccessedAt: 1 },
          ],
          activePageId: "page-1",
          version: 1,
        },
      },
      expandedNodes: { "proj-1": true },
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: "page-1" });

    const { getAllByText } = renderNavTree();
    fireEvent.contextMenu(getAllByText("测试项目")[0]);
    fireEvent.click(getAllByText("新建操作页面")[0]);

    const project = useProjects.getState().projects["proj-1"];
    expect(project.pages).toHaveLength(2);
    // 行为断言：新页面 layout 为空对象（makeEmptyLayout 契约产物，
    // handleNewPage 经 makeEmptyLayout() 构造）
    expect(project.pages[1].layout).toEqual({});
  });
});
