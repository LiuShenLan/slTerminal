// titleManager.test.ts — 标题管理器单元测试
//
// 测试 createTitleManager 的所有函数：
// 终端标题、编辑器标题、文件标题冲突、注册/注销、去重、save-as

import { describe, it, expect, beforeEach } from "vitest";
import { createTitleManager } from "../workspace/titleManager";

describe("createTitleManager", () => {
  let tm: ReturnType<typeof createTitleManager>;

  beforeEach(() => {
    tm = createTitleManager();
  });

  // ---- 终端标题 ----

  describe("getTerminalTitle", () => {
    it("单个终端页签标题为 terminal-0", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
    });

    it("多个终端页签顺序递增", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-2");
    });

    it("不同操作页面独立计数", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page2")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
      expect(tm.getTerminalTitle("page2")).toBe("terminal-1");
    });

    it("关闭不重算——序号不回退", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
      tm.unregisterEditor("page1", "terminal-0");
      // 关闭 terminal-0 后，下一个仍是 terminal-2
      expect(tm.getTerminalTitle("page1")).toBe("terminal-2");
    });
  });

  // ---- 空白编辑器标题 ----

  describe("getEditorTitle", () => {
    it("单个空白编辑器标题为 editor-0", () => {
      expect(tm.getEditorTitle("page1")).toBe("editor-0");
    });

    it("多个空白编辑器顺序递增", () => {
      expect(tm.getEditorTitle("page1")).toBe("editor-0");
      expect(tm.getEditorTitle("page1")).toBe("editor-1");
    });

    it("不同页面独立计数", () => {
      expect(tm.getEditorTitle("page1")).toBe("editor-0");
      expect(tm.getEditorTitle("page2")).toBe("editor-0");
    });

    it("与终端计数器独立", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getEditorTitle("page1")).toBe("editor-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
      expect(tm.getEditorTitle("page1")).toBe("editor-1");
    });
  });

  // ---- 文件编辑器标题 ----

  describe("getFileEditorTitle", () => {
    const ROOT = "D:/project";

    it("无冲突时返回文件名", () => {
      const title = tm.getFileEditorTitle("page1", ROOT, "D:/project/src/index.ts");
      expect(title).toBe("index.ts");
    });

    it("有同名文件（不同路径）时返回相对路径", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      const title = tm.getFileEditorTitle(
        "page1",
        ROOT,
        "D:/project/lib/index.ts",
      );
      expect(title).toBe("lib/index.ts");
    });

    it("不在项目树中时返回绝对路径", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      const title = tm.getFileEditorTitle(
        "page1",
        ROOT,
        "D:/other/index.ts",
      );
      expect(title).toBe("D:/other/index.ts");
    });

    it("检测冲突时排除自己", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      // 查询已存在的同一个文件路径 → 不视为冲突
      const title = tm.getFileEditorTitle(
        "page1",
        ROOT,
        "D:/project/src/index.ts",
      );
      expect(title).toBe("index.ts");
    });

    it("跨页面不检测冲突", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      // page2 独立，不应检测 page1 的文件
      const title = tm.getFileEditorTitle(
        "page2",
        ROOT,
        "D:/project/lib/index.ts",
      );
      expect(title).toBe("index.ts");
    });

    it("无已注册编辑器时返回文件名", () => {
      const title = tm.getFileEditorTitle("empty-page", ROOT, "/some/path/README.md");
      expect(title).toBe("README.md");
    });
  });

  // ---- 注册/注销 ----

  describe("registerEditor / unregisterEditor", () => {
    it("注册后可在 registry 中查找到", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/main.ts");
      const found = tm.findExistingEditor("page1", "D:/project/src/main.ts");
      expect(found).toBe("editor-1");
    });

    it("未注册的 panelId 返回 null", () => {
      const found = tm.findExistingEditor("page1", "D:/nonexistent.ts");
      expect(found).toBeNull();
    });

    it("注销后不再查找到", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/main.ts");
      tm.unregisterEditor("page1", "editor-1");
      const found = tm.findExistingEditor("page1", "D:/project/src/main.ts");
      expect(found).toBeNull();
    });

    it("注册空白编辑器（无 filePath）", () => {
      tm.registerEditor("page1", "editor-0");
      // 空白编辑器没有 filePath，findExistingEditor 不匹配
      const found = tm.findExistingEditor("page1", "");
      expect(found).toBeNull();
    });
  });

  // ---- 重复文件检测 ----

  describe("findExistingEditor", () => {
    it("同一文件路径命中", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      expect(tm.findExistingEditor("page1", "D:/project/src/index.ts")).toBe(
        "editor-1",
      );
    });

    it("Windows 反斜杠与正斜杠视为同一路径", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      expect(
        tm.findExistingEditor("page1", "D:\\project\\src\\index.ts"),
      ).toBe("editor-1");
    });

    it("不同路径未命中", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      expect(
        tm.findExistingEditor("page1", "D:/project/src/other.ts"),
      ).toBeNull();
    });

    it("跨页面独立检测", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      expect(
        tm.findExistingEditor("page2", "D:/project/src/index.ts"),
      ).toBeNull();
    });
  });

  // ---- recomputeTitles ----

  describe("recomputeTitles", () => {
    const ROOT = "D:/project";

    it("单个编辑器返回 basename", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toEqual([{ panelId: "editor-1", title: "index.ts" }]);
    });

    it("无冲突时所有编辑器返回 basename", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/src/README.md");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toHaveLength(2);
      expect(updates).toContainEqual({ panelId: "editor-1", title: "index.ts" });
      expect(updates).toContainEqual({
        panelId: "editor-2",
        title: "README.md",
      });
    });

    it("同名冲突时显示相对路径", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/lib/index.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toContainEqual({
        panelId: "editor-1",
        title: "src/index.ts",
      });
      expect(updates).toContainEqual({
        panelId: "editor-2",
        title: "lib/index.ts",
      });
    });

    it("不在项目树中的文件显示绝对路径（冲突时）", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/other/index.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toContainEqual({
        panelId: "editor-1",
        title: "src/index.ts",
      });
      expect(updates).toContainEqual({
        panelId: "editor-2",
        title: "D:/other/index.ts",
      });
    });

    it("关闭一个同名面板后剩余面板切回 basename", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/lib/index.ts");
      tm.unregisterEditor("page1", "editor-2");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toEqual([{ panelId: "editor-1", title: "index.ts" }]);
    });

    it("空注册表返回空数组", () => {
      const updates = tm.recomputeTitles("empty-page", ROOT);
      expect(updates).toEqual([]);
    });

    it("跳过空白编辑器（无 filePath）", () => {
      tm.registerEditor("page1", "editor-0");
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      // 空白编辑器不参与标题计算
      expect(updates).toEqual([{ panelId: "editor-1", title: "index.ts" }]);
    });

    it("三个同名文件正确计算冲突标题", () => {
      tm.registerEditor("page1", "e1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "e2", "D:/project/lib/index.ts");
      tm.registerEditor("page1", "e3", "D:/project/test/index.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toContainEqual({ panelId: "e1", title: "src/index.ts" });
      expect(updates).toContainEqual({ panelId: "e2", title: "lib/index.ts" });
      expect(updates).toContainEqual({ panelId: "e3", title: "test/index.ts" });
    });
  });

  // ---- handleSaveAs ----

  describe("handleSaveAs", () => {
    const ROOT = "D:/project";

    it("更新空白编辑器的 filePath 并计算标题", () => {
      tm.registerEditor("page1", "editor-0");
      const updates = tm.handleSaveAs(
        "page1",
        "editor-0",
        "D:/project/src/main.ts",
        ROOT,
      );
      expect(updates).toEqual([{ panelId: "editor-0", title: "main.ts" }]);
    });

    it("另存为后路径更新，标题随之变化", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/old.ts");
      const updates = tm.handleSaveAs(
        "page1",
        "editor-1",
        "D:/project/src/new.ts",
        ROOT,
      );
      expect(updates).toEqual([{ panelId: "editor-1", title: "new.ts" }]);
    });

    it("另存为后出现同名冲突，所有相关编辑器标题更新", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-0");
      const updates = tm.handleSaveAs(
        "page1",
        "editor-0",
        "D:/project/lib/index.ts",
        ROOT,
      );
      expect(updates).toContainEqual({
        panelId: "editor-0",
        title: "lib/index.ts",
      });
      expect(updates).toContainEqual({
        panelId: "editor-1",
        title: "src/index.ts",
      });
    });
  });
});
