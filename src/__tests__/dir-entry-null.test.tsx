// dir-entry-null.test.tsx — null DirEntry 渲染测试（FE-12）
//
// DirEntry.size/modified 契约修正为 number | null（Rust serde 输出 null 而非省略/undefined）：
// - 目录条目 size/modified 恒为 null → FileTree 正常渲染不崩溃
// - 混合树（目录 null/null + 文件数值 + 展开子目录 null/null）递归渲染
// - mockEntry 测试工厂输出与契约一致（目录 → null/null，文件 → 1024/1）
// 说明：前端无 size/modified 排序消费（目录序由后端 fs_read_dir 返回），
// 本测试覆盖渲染侧的 null 语义。

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import React from "react";
import { FileTree } from "../features/explorer/FileTree";
import type { TreeNode } from "../features/explorer/useFileTree";
import { mockEntry } from "./helpers/vfs";

/** 渲染 FileTree（照 explorer 测试 renderFileTree 模式） */
function renderFileTree(nodes: TreeNode[]) {
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
    }),
  );
}

/** DirEntry → TreeNode（展开态可配） */
function toNode(entry: ReturnType<typeof mockEntry>, expanded = false): TreeNode {
  return { entry, expanded, children: [], loading: false };
}

describe("DirEntry null 语义（FE-12）", () => {
  afterEach(() => {
    cleanup();
  });

  it("目录条目 size/modified 为 null 时正常渲染（名称可见，不崩溃）", () => {
    const dir = mockEntry("src", true, "C:/proj/src");
    // 契约断言：目录恒为 null（非 undefined）
    expect(dir.size).toBeNull();
    expect(dir.modified).toBeNull();
    const { getByText } = renderFileTree([toNode(dir)]);
    expect(getByText("src")).toBeTruthy();
  });

  it("混合树：目录 null/null + 文件数值 + 展开子目录 null/null 递归渲染", () => {
    const file = { ...mockEntry("a.ts", false, "C:/proj/a.ts"), size: 100, modified: 123 } as ReturnType<typeof mockEntry>;
    const subDir = { ...mockEntry("nested", true, "C:/proj/src/nested") } as ReturnType<typeof mockEntry>;
    const dir = { ...mockEntry("src", true, "C:/proj/src") } as ReturnType<typeof mockEntry>;
    const nodes: TreeNode[] = [
      { ...toNode(dir, true), children: [toNode(file), toNode(subDir)] },
    ];
    const { getByText } = renderFileTree(nodes);
    expect(getByText("src")).toBeTruthy();
    expect(getByText("a.ts")).toBeTruthy();
    expect(getByText("nested")).toBeTruthy();
  });

  it("mockEntry 工厂：目录 → size/modified 为 null；文件 → 1024/1（与契约一致）", () => {
    const dir = mockEntry("d", true, "C:/p/d");
    const file = mockEntry("f", false, "C:/p/f");
    expect(dir.size).toBeNull();
    expect(dir.modified).toBeNull();
    expect(file.size).toBe(1024);
    expect(file.modified).toBe(1);
  });
});
