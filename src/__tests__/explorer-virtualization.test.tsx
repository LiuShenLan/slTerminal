// explorer-virtualization.test.tsx — FileTree 虚拟化（FE-30）窗口化渲染测试
//
// 覆盖：
// - 大目录（1000 节点）仅渲染滚动窗口内行（行数远小于节点总数）
// - 容器高度未测得（jsdom 无布局 clientHeight=0）→ 全量渲染兜底
// - 滚动后窗口平移（顶部行卸载、中部行进入窗口）
// - 展开大目录后行数仍窗口化（嵌套深度参与扁平化）
// - 窗口化下选中模型 / 右键菜单 / 重命名输入框行为保持
// - FE-40 程序式选中视口外行 → 滚动跟随（scrollTop 定位到该行）
//
// 实现说明：jsdom 无真实布局，滚动容器 clientHeight 恒 0——测试经
// HTMLElement.prototype.clientHeight 覆盖模拟视口高度（beforeEach mock / afterEach 恢复，
// 仅影响本文件环境）；scrollTop 为 jsdom 实例可写属性，直接赋值后 fireEvent.scroll。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, fireEvent, cleanup, screen } from "@testing-library/react";
import React from "react";
import { FileTree } from "../features/explorer/FileTree";
import type { TreeNode } from "../features/explorer/useFileTree";

/** 构造单节点 */
function makeFileNode(path: string, name: string, isDir = false): TreeNode {
  return {
    entry: { name, path, isDir, size: isDir ? null : 100, modified: Date.now() },
    expanded: false,
    children: [],
    loading: false,
  };
}

/** 构造 N 个根级文件节点（不展开，扁平 N 行） */
function makeTree(n: number): TreeNode[] {
  return Array.from({ length: n }, (_, i) =>
    makeFileNode(`/p/f${i}.ts`, `f${i}.ts`),
  );
}

/** 渲染 FileTree（独立组件，全默认 props + overrides） */
function renderFileTree(
  nodes: TreeNode[],
  overrides: Partial<Parameters<typeof FileTree>[0]> = {},
) {
  return render(
    React.createElement(FileTree, {
      nodes,
      depth: 0,
      gitStatusMap: new Map<string, string>(),
      onToggleExpand: vi.fn(),
      onOpenFile: vi.fn(),
      onOpenInTerminal: vi.fn(),
      onRename: vi.fn(),
      onDelete: vi.fn(),
      onNewFile: vi.fn(),
      onNewFolder: vi.fn(),
      selectedPath: null,
      onSelect: vi.fn(),
      renamingPath: null,
      renameValue: "",
      onRenameStart: vi.fn(),
      onRenameCancel: vi.fn(),
      ...overrides,
    }),
  );
}

/** 当前渲染的节点行数（按文件名 span 文本正则计数，不依赖行 DOM 结构） */
function renderedRowCount(): number {
  return screen.getAllByText(/^f\d+\.ts$/).length;
}

/**
 * 模拟滚动容器视口高度：覆盖 HTMLElement.prototype.clientHeight。
 * 返回恢复函数（delete 覆盖后回落到 jsdom 原 getter——clientHeight 定义在 Element 原型链上）。
 */
function mockClientHeight(h: number): () => void {
  const proto = HTMLElement.prototype as unknown as { clientHeight: number };
  const protoAny = proto as unknown as Record<string, unknown>;
  const desc = Object.getOwnPropertyDescriptor(proto, "clientHeight");
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => h });
  return () => {
    if (desc) Object.defineProperty(proto, "clientHeight", desc);
    else delete protoAny.clientHeight;
  };
}

/** 定位滚动容器（虚拟化列表的 overflowY 容器） */
function findScroller(container: HTMLElement): HTMLElement {
  const el = container.querySelector('[style*="overflow-y"]');
  if (!el) throw new Error("未找到滚动容器");
  return el as HTMLElement;
}

describe("FileTree 虚拟化（FE-30）", () => {
  afterEach(() => {
    cleanup();
  });

  it("1000 节点树窗口化：渲染行数远小于 1000", () => {
    const restore = mockClientHeight(300);
    try {
      renderFileTree(makeTree(1000));
      const count = renderedRowCount();
      // 视口 300px / 24px ≈ 12.5 行 + overscan 16 → 期望 20~40 行
      expect(count).toBeGreaterThan(0);
      expect(count).toBeLessThan(100);
    } finally {
      restore();
    }
  });

  it("容器高度未测得（clientHeight=0）→ 全量渲染兜底", () => {
    // 不 mock 高度：jsdom 默认 clientHeight=0，窗口退化全量
    renderFileTree(makeTree(120));
    expect(renderedRowCount()).toBe(120);
  });

  it("滚动后窗口平移：顶部行卸载、中部行进入窗口", () => {
    const restore = mockClientHeight(300);
    try {
      const { container } = renderFileTree(makeTree(1000));
      // 初始窗口含顶部节点
      expect(screen.queryByText("f0.ts")).not.toBeNull();
      expect(renderedRowCount()).toBeLessThan(100);

      // 滚动到中部：scrollTop=5000 → 起始行 ≈ 5000/24 ≈ 208
      const scroller = findScroller(container);
      scroller.scrollTop = 5000;
      fireEvent.scroll(scroller);

      // 顶部行已卸载（窗口外），中部行进入窗口
      expect(screen.queryByText("f0.ts")).toBeNull();
      expect(screen.queryByText("f208.ts")).not.toBeNull();
      // 窗口大小不变（仍远小于总数）
      expect(renderedRowCount()).toBeLessThan(100);
    } finally {
      restore();
    }
  });

  it("展开含 1000 子节点的大目录 → 渲染行数仍窗口化", () => {
    const restore = mockClientHeight(300);
    try {
      const children = Array.from({ length: 1000 }, (_, i) =>
        makeFileNode(`/p/big/f${i}.ts`, `f${i}.ts`),
      );
      const root: TreeNode = {
        entry: { name: "big", path: "/p/big", isDir: true, size: null, modified: Date.now() },
        expanded: true,
        children,
        loading: false,
      };
      renderFileTree([root]);
      // 扁平化后共 1001 行，窗口仍只渲染 ~29 行
      expect(renderedRowCount()).toBeGreaterThan(0);
      expect(renderedRowCount()).toBeLessThan(100);
    } finally {
      restore();
    }
  });

  it("窗口化下选中模型保持：点击可见行 → onSelect(path)", () => {
    const restore = mockClientHeight(300);
    try {
      const onSelect = vi.fn();
      renderFileTree(makeTree(1000), { onSelect });
      // 窗口内的节点（f0.ts）可点击选中；窗口外节点不渲染
      fireEvent.click(screen.getByText("f0.ts"));
      expect(onSelect).toHaveBeenCalledWith("/p/f0.ts");
    } finally {
      restore();
    }
  });

  it("窗口化下右键菜单保持：可见行 contextMenu → 文件菜单出现", () => {
    const restore = mockClientHeight(300);
    try {
      renderFileTree(makeTree(1000));
      fireEvent.contextMenu(screen.getByText("f0.ts"));
      // 文件菜单含「打开」（不是根级菜单）
      expect(screen.getAllByText("打开").length).toBeGreaterThan(0);
    } finally {
      restore();
    }
  });

  it("窗口化下重命名输入框保持：窗口内节点重命名 → 渲染输入框（替换行）", () => {
    const restore = mockClientHeight(300);
    try {
      renderFileTree(makeTree(1000), {
        renamingPath: "/p/f0.ts",
        renameValue: "f0.ts",
      });
      const inputs = document.querySelectorAll("input");
      expect(inputs.length).toBe(1);
      expect((inputs[0] as HTMLInputElement).value).toBe("f0.ts");
    } finally {
      restore();
    }
  });

  it("FE-40 程序式选中视口外行 → 滚动跟随：scrollTop 定位到该行", () => {
    const restore = mockClientHeight(300);
    try {
      const props: Parameters<typeof FileTree>[0] = {
        nodes: makeTree(1000),
        depth: 0,
        gitStatusMap: new Map<string, string>(),
        onToggleExpand: vi.fn(),
        onOpenFile: vi.fn(),
        onOpenInTerminal: vi.fn(),
        onRename: vi.fn(),
        onDelete: vi.fn(),
        onNewFile: vi.fn(),
        onNewFolder: vi.fn(),
        selectedPath: null,
        onSelect: vi.fn(),
        renamingPath: null,
        renameValue: "",
        onRenameStart: vi.fn(),
        onRenameCancel: vi.fn(),
      };
      const { container, rerender } = render(
        React.createElement(FileTree, props),
      );
      const scroller = findScroller(container);
      // 初始无选中 → 不滚动
      expect(scroller.scrollTop).toBe(0);

      // 程序式选中视口外行（f500.ts 行索引 500，视口 300px ≈ 12.5 行）→
      // FE-40 effect 滚动跟随：scrollTop = 行索引 × ROW_HEIGHT
      rerender(
        React.createElement(FileTree, {
          ...props,
          selectedPath: "/p/f500.ts",
        }),
      );
      expect(scroller.scrollTop).toBe(500 * 24);
    } finally {
      restore();
    }
  });
});
