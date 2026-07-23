// explorer-selection.test.tsx — 文件浏览器选中模型测试
//
// 测试 FileTree 单击选中行为：选中回调、高亮渲染、单选切换、空白取消

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/react";
import React from "react";
import { FileTree } from "../features/explorer/FileTree";
import type { TreeNode } from "../features/explorer/useFileTree";

function makeFileNode(path: string, name: string, isDir = false): TreeNode {
  return {
    entry: { name, path, isDir, size: isDir ? undefined : 100, modified: Date.now() },
    expanded: false,
    children: [],
    loading: false,
  };
}

function renderFileTree(
  nodes: TreeNode[],
  props: Partial<{
    selectedPath: string | null;
    onSelect: (path: string | null) => void;
    onOpenFile: (path: string) => void;
    onToggleExpand: (path: string) => void;
  }> = {},
) {
  const defaultProps = {
    nodes,
    depth: 0,
    gitStatusMap: new Map<string, string>(),
    onToggleExpand: props.onToggleExpand ?? vi.fn(),
    onOpenFile: props.onOpenFile ?? vi.fn(),
    onOpenInTerminal: vi.fn(),
    onRename: vi.fn(),
    onDelete: vi.fn(),
    onNewFile: vi.fn(),
    onNewFolder: vi.fn(),
    selectedPath: props.selectedPath ?? null,
    onSelect: props.onSelect ?? vi.fn(),
    renamingPath: null,
    renameValue: "",
    onRenameStart: vi.fn(),
    onRenameCancel: vi.fn(),
  };
  return render(React.createElement(FileTree, defaultProps));
}

describe("FileTree 选中模型", () => {
  let onSelect: (path: string | null) => void;

  beforeEach(() => {
    onSelect = vi.fn();
  });

  afterEach(() => {
    cleanup();
  });

  it("单击文件行 → onSelect(filePath) 调用", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { onSelect });
    fireEvent.click(getByText("test.ts"));
    expect(onSelect).toHaveBeenCalledWith("/a/test.ts");
  });

  it("单击文件夹行 → onSelect(dirPath) 调用", () => {
    const nodes = [makeFileNode("/a/src", "src", true)];
    const { getByText } = renderFileTree(nodes, { onSelect });
    fireEvent.click(getByText("src"));
    expect(onSelect).toHaveBeenCalledWith("/a/src");
  });

  it("选中行渲染 EXPLORER_SELECTION_BG 背景", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { selectedPath: "/a/test.ts" });
    const row = getByText("test.ts").closest("div");
    expect(row).not.toBeNull();
    // jsdom 将 #094771 转为 rgb(9, 71, 113)
    expect(row!.style.background).toBe("rgb(9, 71, 113)");
  });

  it("未选中行不渲染选中背景", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { selectedPath: "/a/other.ts" });
    const row = getByText("test.ts").closest("div");
    expect(row).not.toBeNull();
    expect(row!.style.background).not.toBe("rgb(9, 71, 113)");
  });

  it("单击另一行 → onSelect(newPath) → 旧行取消高亮（单选）", () => {
    const nodes = [
      makeFileNode("/a/a.ts", "a.ts"),
      makeFileNode("/a/b.ts", "b.ts"),
    ];
    const { getByText } = renderFileTree(nodes, { selectedPath: "/a/a.ts", onSelect });
    fireEvent.click(getByText("b.ts"));
    expect(onSelect).toHaveBeenCalledWith("/a/b.ts");
  });

  it("单击 root 空白区域 → onSelect(null)", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { container } = renderFileTree(nodes, { onSelect });
    // 点击 wrapper div（depth=0 的包装层）
    const wrapper = container.firstChild as HTMLElement;
    fireEvent.click(wrapper);
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("selectedPath=null → 无行高亮", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { selectedPath: null });
    const row = getByText("test.ts").closest("div");
    expect(row!.style.background).not.toBe("rgb(9, 71, 113)");
  });

  it("双击文件 → onOpenFile 仍然调用（不破坏现有双击行为）", () => {
    const onOpenFile = vi.fn();
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { onOpenFile });
    fireEvent.doubleClick(getByText("test.ts"));
    expect(onOpenFile).toHaveBeenCalledWith("/a/test.ts");
  });

  it("单击目录 → onToggleExpand 仍然调用（不破坏现有展开折叠）", () => {
    const onToggleExpand = vi.fn();
    const nodes = [makeFileNode("/a/src", "src", true)];
    const { getByText } = renderFileTree(nodes, { onToggleExpand });
    fireEvent.click(getByText("src"));
    expect(onToggleExpand).toHaveBeenCalledWith("/a/src");
  });

  it("hover 时 selected 背景保持不变（不覆盖选中态）", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { selectedPath: "/a/test.ts" });
    const row = getByText("test.ts").closest("div")!;
    fireEvent.mouseEnter(row);
    // 选中态背景保持（不变成 hover 色）
    expect(row.style.background).toBe("rgb(9, 71, 113)");
  });

  it("selectedPath prop 变化 → 高亮跟随变化", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText, rerender } = render(
      React.createElement(FileTree, {
        nodes,
        depth: 0,
        gitStatusMap: new Map(),
        onToggleExpand: vi.fn(),
        onOpenFile: vi.fn(),
        onOpenInTerminal: vi.fn(),
        onRename: vi.fn(),
        onDelete: vi.fn(),
        onNewFile: vi.fn(),
        onNewFolder: vi.fn(),
        selectedPath: "/a/test.ts",
        onSelect: vi.fn(),
        renamingPath: null,
        renameValue: "",
        onRenameStart: vi.fn(),
        onRenameCancel: vi.fn(),
      }),
    );
    expect(getByText("test.ts").closest("div")!.style.background).toBe("rgb(9, 71, 113)");

    rerender(
      React.createElement(FileTree, {
        nodes,
        depth: 0,
        gitStatusMap: new Map(),
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
      }),
    );
    expect(getByText("test.ts").closest("div")!.style.background).not.toBe("rgb(9, 71, 113)");
  });

  it("嵌套子节点单击 → onSelect", () => {
    const child = makeFileNode("/a/src/index.ts", "index.ts");
    const parent: TreeNode = {
      entry: { name: "src", path: "/a/src", isDir: true, size: undefined, modified: Date.now() },
      expanded: true,
      children: [child],
      loading: false,
    };
    const { getByText } = renderFileTree([parent], { onSelect });
    fireEvent.click(getByText("index.ts"));
    expect(onSelect).toHaveBeenCalledWith("/a/src/index.ts");
  });

  it("右键菜单仍然可触发（不破坏现有右键行为）", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText, getAllByText } = renderFileTree(nodes);
    fireEvent.contextMenu(getByText("test.ts"));
    // 右键菜单应出现（含"打开"项）
    expect(getAllByText("打开").length).toBeGreaterThan(0);
  });

  it("同文件再次单击不取消选中（无 toggle 行为）", () => {
    const nodes = [makeFileNode("/a/test.ts", "test.ts")];
    const { getByText } = renderFileTree(nodes, { selectedPath: "/a/test.ts", onSelect });
    fireEvent.click(getByText("test.ts"));
    // 仍然调用 onSelect（不取消）
    expect(onSelect).toHaveBeenCalledWith("/a/test.ts");
  });
});
