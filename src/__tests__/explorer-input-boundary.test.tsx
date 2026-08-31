// explorer-input-boundary.test.tsx — FileTree 内联输入框边界测试（EXP-06）
//
// 覆盖：
//   重命名输入框：Escape 取消 / 空名取消 / 重名仍提交 / 失焦提交 / 失焦空值取消
//   文件夹级新建文件/文件夹：Escape 取消 / blur 空值不创建 / Enter 创建

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/react";
import React from "react";
import { FileTree } from "../features/explorer/FileTree";
import type { TreeNode } from "../features/explorer/useFileTree";

function makeFileNode(path: string, name: string, isDir = false): TreeNode {
  return {
    entry: { name, path, isDir, size: isDir ? null : 100, modified: Date.now() },
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

/** 重命名模式下渲染（renamingPath 非 null） */
function renderRenaming(overrides: Record<string, unknown> = {}) {
  const props = baseProps({
    renamingPath: "/a/test.ts",
    renameValue: "test.ts",
    ...overrides,
  });
  const utils = render(React.createElement(FileTree, props));
  const input = utils.container.querySelector("input") as HTMLInputElement;
  return { ...utils, input };
}

afterEach(() => {
  cleanup();
});

// =====================================================================
// R 组：重命名输入框边界
// =====================================================================

describe("重命名输入框边界", () => {
  it("R1: Enter 空名（仅空白）→ 不调 onRename，调 onRenameCancel", () => {
    const onRename = vi.fn();
    const onRenameCancel = vi.fn();
    const { input } = renderRenaming({ onRename, onRenameCancel });

    (input as HTMLInputElement).value = "   ";
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
    expect(onRenameCancel).toHaveBeenCalledTimes(1);
  });

  it("R2: 失焦（blur）非空名 → onRename 提交", () => {
    const onRename = vi.fn();
    const { input } = renderRenaming({ onRename });

    (input as HTMLInputElement).value = "renamed.ts";
    fireEvent.blur(input);

    expect(onRename).toHaveBeenCalledWith("/a/test.ts", "renamed.ts");
    expect(onRename).toHaveBeenCalledTimes(1);
  });

  it("R3: 失焦（blur）空名 → 不调 onRename，调 onRenameCancel", () => {
    const onRename = vi.fn();
    const onRenameCancel = vi.fn();
    const { input } = renderRenaming({ onRename, onRenameCancel });

    (input as HTMLInputElement).value = "";
    fireEvent.blur(input);

    expect(onRename).not.toHaveBeenCalled();
    expect(onRenameCancel).toHaveBeenCalledTimes(1);
  });

  it("R4: 重名为当前同名（重名）→ 视为取消（同名短路，不发 IPC）", () => {
    // 防复发：修复前同名提交会触发后端 src==dst 覆盖分支误删源文件；
    // 现语义 = 名字未变即取消（与 Escape 同路径）
    const onRename = vi.fn();
    const onRenameCancel = vi.fn();
    const { input } = renderRenaming({ onRename, onRenameCancel });

    // 不改名直接 Enter（value 即原文件名）
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
    expect(onRenameCancel).toHaveBeenCalledTimes(1);
  });

  it("R5: Escape → 调 onRenameCancel，不调 onRename", () => {
    const onRename = vi.fn();
    const onRenameCancel = vi.fn();
    const { input } = renderRenaming({ onRename, onRenameCancel });

    (input as HTMLInputElement).value = "discard.ts";
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onRename).not.toHaveBeenCalled();
    expect(onRenameCancel).toHaveBeenCalledTimes(1);
  });
});

// =====================================================================
// F 组：文件夹级新建文件/文件夹输入框边界
// =====================================================================

describe("文件夹级新建输入框边界", () => {
  const dirNode = makeFileNode("/a/src", "src", true);

  /** 右键文件夹 → 点击菜单项 → 返回输入框 */
  function openFolderInput(
    menuLabel: "新建文件" | "新建文件夹",
    overrides: Record<string, unknown> = {},
  ) {
    const props = baseProps({ nodes: [dirNode], ...overrides });
    const utils = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(utils.getAllByText("src")[0]);
    const items = utils.getAllByText(menuLabel);
    fireEvent.click(items[0]);
    const inputs = document.querySelectorAll("input");
    const input = inputs[inputs.length - 1] as HTMLInputElement;
    return { ...utils, input };
  }

  it("F1: 文件夹级新建文件 Escape → 不创建、输入框消失", () => {
    const onNewFile = vi.fn();
    const { input } = openFolderInput("新建文件", { onNewFile });

    (input as HTMLInputElement).value = "skip.ts";
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onNewFile).not.toHaveBeenCalled();
    expect(document.querySelectorAll("input").length).toBe(0);
  });

  it("F2: 文件夹级新建文件 blur 空值 → 不创建、输入框消失", () => {
    const onNewFile = vi.fn();
    const props = baseProps({ nodes: [dirNode], onNewFile });
    const utils = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(utils.getAllByText("src")[0]);
    fireEvent.click(utils.getAllByText("新建文件")[0]);
    const inputs = document.querySelectorAll("input");
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;

    fireEvent.blur(newFileInput);

    expect(onNewFile).not.toHaveBeenCalled();
    expect(document.querySelectorAll("input").length).toBe(0);
  });

  it("F3: 文件夹级新建文件夹 Enter → onNewFolder 拼接父路径", () => {
    const onNewFolder = vi.fn();
    const props = baseProps({ nodes: [dirNode], onNewFolder });
    const utils = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(utils.getAllByText("src")[0]);
    fireEvent.click(utils.getAllByText("新建文件夹")[0]);
    const inputs = document.querySelectorAll("input");
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;

    fireEvent.change(newFolderInput, { target: { value: "components" } });
    fireEvent.keyDown(newFolderInput, { key: "Enter" });

    expect(onNewFolder).toHaveBeenCalledWith("/a/src/components");
    expect(onNewFolder).toHaveBeenCalledTimes(1);
    expect(document.querySelectorAll("input").length).toBe(0);
  });

  it("F4: 文件夹级新建文件夹 Escape → 不创建、输入框消失", () => {
    const onNewFolder = vi.fn();
    const props = baseProps({ nodes: [dirNode], onNewFolder });
    const utils = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(utils.getAllByText("src")[0]);
    fireEvent.click(utils.getAllByText("新建文件夹")[0]);
    const inputs = document.querySelectorAll("input");
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;

    fireEvent.change(newFolderInput, { target: { value: "components" } });
    fireEvent.keyDown(newFolderInput, { key: "Escape" });

    expect(onNewFolder).not.toHaveBeenCalled();
    expect(document.querySelectorAll("input").length).toBe(0);
  });

  it("F5: 文件夹级新建文件夹 blur 空值 → 不创建、输入框消失", () => {
    const onNewFolder = vi.fn();
    const props = baseProps({ nodes: [dirNode], onNewFolder });
    const utils = render(React.createElement(FileTree, props));
    fireEvent.contextMenu(utils.getAllByText("src")[0]);
    fireEvent.click(utils.getAllByText("新建文件夹")[0]);
    const inputs = document.querySelectorAll("input");
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;

    fireEvent.blur(newFolderInput);

    expect(onNewFolder).not.toHaveBeenCalled();
    expect(document.querySelectorAll("input").length).toBe(0);
  });
});
