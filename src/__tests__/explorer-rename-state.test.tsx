// explorer-rename-state.test.tsx — 重命名状态上提测试
//
// 测试 renamingPath 从 FileTree 上提到 ExplorerPanel 后的行为正确性

import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
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

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    nodes: [makeFileNode("/a/test.ts", "test.ts")],
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
  };
}

describe("重命名状态上提", () => {
  it("右键 '重命名' → onRenameStart(path, name) 调用", () => {
    const onRenameStart = vi.fn();
    const props = baseProps({ onRenameStart });
    const { getByText, getAllByText } = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(getByText("test.ts"));
    const items = getAllByText("重命名");
    fireEvent.click(items[0]);
    expect(onRenameStart).toHaveBeenCalledWith("/a/test.ts", "test.ts");
  });

  it("renamingPath 非 null → FileTree 渲染 input", () => {
    const props = baseProps({ renamingPath: "/a/test.ts", renameValue: "test.ts" });
    const { container } = render(React.createElement(FileTree, props));
    const input = container.querySelector("input");
    expect(input).not.toBeNull();
    expect((input as HTMLInputElement).defaultValue).toBe("test.ts");
  });

  it("renamingPath 为 null → 不渲染 input", () => {
    const props = baseProps({ renamingPath: null });
    const { container } = render(React.createElement(FileTree, props));
    expect(container.querySelector("input")).toBeNull();
  });

  it("input Enter → onRename 调用（读取 ref 实际值）", () => {
    const onRename = vi.fn();
    const props = baseProps({ renamingPath: "/a/test.ts", renameValue: "test.ts", onRename });
    const { container } = render(React.createElement(FileTree, props));
    const input = container.querySelector("input")!;
    // 模拟用户编辑
    (input as HTMLInputElement).value = "renamed.ts";
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith("/a/test.ts", "renamed.ts");
  });

  it("input Escape → onRenameCancel 调用", () => {
    const onRenameCancel = vi.fn();
    const props = baseProps({ renamingPath: "/a/test.ts", renameValue: "test.ts", onRenameCancel });
    const { container } = render(React.createElement(FileTree, props));
    const input = container.querySelector("input")!;
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onRenameCancel).toHaveBeenCalled();
  });

  it("renameValue 初始值 = 文件名（由 ExplorerPanel 传入）", () => {
    const props = baseProps({ renamingPath: "/a/test.ts", renameValue: "test.ts" });
    const { container } = render(React.createElement(FileTree, props));
    const input = container.querySelector("input")! as HTMLInputElement;
    expect(input.defaultValue).toBe("test.ts");
  });

  it("新建文件 input 不受重命名状态影响", () => {
    // 同时存在 renamingPath 和新文件 input 测试
    const props = baseProps({ renamingPath: "/a/test.ts", renameValue: "test.ts" });
    const { container } = render(React.createElement(FileTree, props));
    // 只有一个 input（重命名的）
    expect(container.querySelectorAll("input")).toHaveLength(1);
  });

  it("文件夹右键 '重命名' → onRenameStart 传入文件夹名", () => {
    const onRenameStart = vi.fn();
    const dirNode: TreeNode = {
      entry: { name: "src", path: "/a/src", isDir: true, size: undefined, modified: Date.now() },
      expanded: false,
      children: [],
      loading: false,
    };
    const props = { ...baseProps({ onRenameStart }), nodes: [dirNode] };
    const { getByText, getAllByText } = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(getByText("src"));
    const items = getAllByText("重命名");
    fireEvent.click(items[0]);
    expect(onRenameStart).toHaveBeenCalledWith("/a/src", "src");
  });
});
