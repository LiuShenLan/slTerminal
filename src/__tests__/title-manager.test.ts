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

    it("SaveAs 解决同名冲突——原冲突文件恢复 basename", () => {
      // 两个同名 index.ts 存在冲突
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/lib/index.ts");

      // 将 editor-2 另存为其他文件名，冲突解除
      const updates = tm.handleSaveAs(
        "page1",
        "editor-2",
        "D:/project/lib/unique.ts",
        ROOT,
      );

      expect(updates).toHaveLength(2);
      // editor-1 为唯一 index.ts，恢复 basename
      expect(updates).toContainEqual({ panelId: "editor-1", title: "index.ts" });
      // editor-2 新文件名唯一
      expect(updates).toContainEqual({ panelId: "editor-2", title: "unique.ts" });
    });

    it("SaveAs 导致三路同名冲突", () => {
      // 先有两个不同名的文件（无冲突）
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/lib/index.ts");
      // editor-3 原名为 unique.ts
      tm.registerEditor("page1", "editor-3", "D:/project/lib/unique.ts");

      // 将 editor-3 另存为第三个 index.ts，导致三路冲突
      const updates = tm.handleSaveAs(
        "page1",
        "editor-3",
        "D:/project/test/index.ts",
        ROOT,
      );

      expect(updates).toHaveLength(3);
      // 三个文件同名为 index.ts，全部显示相对路径
      expect(updates).toContainEqual({ panelId: "editor-1", title: "src/index.ts" });
      expect(updates).toContainEqual({ panelId: "editor-2", title: "lib/index.ts" });
      expect(updates).toContainEqual({ panelId: "editor-3", title: "test/index.ts" });
    });
  });

  // ---- suffix 标题后缀支持 ----

  describe("suffix 标题后缀支持", () => {
    const ROOT = "D:/project";

    // -- getFileEditorTitle with suffix --

    it("无冲突时 basename 拼接 suffix", () => {
      const title = tm.getFileEditorTitle(
        "page1", ROOT, "D:/project/src/a.ts", "(git diff)",
      );
      expect(title).toBe("a.ts(git diff)");
    });

    it("有同名冲突时 relPath 拼接 suffix", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/a.ts");
      const title = tm.getFileEditorTitle(
        "page1", ROOT, "D:/project/lib/a.ts", "(git diff)",
      );
      expect(title).toBe("lib/a.ts(git diff)");
    });

    // -- recomputeTitles with suffix --

    it("recomputeTitles 保留各 entry 的 suffix（普通编辑器 + git 页签同名混合）", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts"); // 无 suffix
      tm.registerEditor("page1", "diff-1", "D:/project/src/index.ts", "(git diff)"); // 有 suffix
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toHaveLength(2);
      expect(updates).toContainEqual({ panelId: "editor-1", title: "src/index.ts" });
      expect(updates).toContainEqual({ panelId: "diff-1", title: "src/index.ts(git diff)" });
    });

    it("recomputeTitles 无 suffix 条目不拼接后缀", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/unique.ts");
      const updates = tm.recomputeTitles("page1", ROOT);
      expect(updates).toEqual([{ panelId: "editor-1", title: "unique.ts" }]);
    });

    // -- findExistingEditor with suffix --

    it("findExistingEditor 带 suffix 匹配同 suffix 条目", () => {
      tm.registerEditor("page1", "diff-1", "D:/project/src/index.ts", "(git diff)");
      const found = tm.findExistingEditor("page1", "D:/project/src/index.ts", "(git diff)");
      expect(found).toBe("diff-1");
    });

    it("findExistingEditor 不带 suffix 不匹配带 suffix 条目", () => {
      tm.registerEditor("page1", "diff-1", "D:/project/src/index.ts", "(git diff)");
      const found = tm.findExistingEditor("page1", "D:/project/src/index.ts");
      expect(found).toBeNull();
    });

    it("findExistingEditor 带 suffix 不匹配无 suffix 条目", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts"); // 无 suffix
      const found = tm.findExistingEditor("page1", "D:/project/src/index.ts", "(git diff)");
      expect(found).toBeNull();
    });

    it("findExistingEditor 不同 suffix 不相互匹配", () => {
      tm.registerEditor("page1", "diff-1", "D:/project/src/index.ts", "(git diff)");
      const found = tm.findExistingEditor("page1", "D:/project/src/index.ts", "(git add)");
      expect(found).toBeNull();
    });
  });

  // ---- onDeletePage ----

  describe("onDeletePage", () => {
    it("删除后 registry 条目被清理，已注册编辑器不可查找", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/src/main.ts");
      tm.onDeletePage("page1");

      expect(tm.findExistingEditor("page1", "D:/project/src/index.ts")).toBeNull();
      expect(tm.findExistingEditor("page1", "D:/project/src/main.ts")).toBeNull();
    });

    it("删除后 counters 条目被清理，终端编号重置为 terminal-0", () => {
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-2");
      tm.onDeletePage("page1");

      // 重新使用同一 pageId，从 terminal-0 开始
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");
    });

    it("删除不存在的 pageId 不抛异常", () => {
      expect(() => tm.onDeletePage("nonexistent")).not.toThrow();
    });

    it("删除一个页面不影响其他页面的 registry 和 counters", () => {
      // page1 注册编辑器 + 消费终端编号
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");
      expect(tm.getTerminalTitle("page1")).toBe("terminal-1");

      // page2 独立注册
      tm.registerEditor("page2", "editor-2", "D:/project/lib/util.ts");
      expect(tm.getTerminalTitle("page2")).toBe("terminal-0");

      // 删除 page1
      tm.onDeletePage("page1");

      // page1 已清空
      expect(tm.findExistingEditor("page1", "D:/project/src/index.ts")).toBeNull();
      expect(tm.getTerminalTitle("page1")).toBe("terminal-0");

      // page2 完全不受影响
      expect(tm.findExistingEditor("page2", "D:/project/lib/util.ts")).toBe("editor-2");
      expect(tm.getTerminalTitle("page2")).toBe("terminal-1");
    });

    it("删除后 recomputeTitles 返回空数组", () => {
      tm.registerEditor("page1", "editor-1", "D:/project/src/index.ts");
      tm.registerEditor("page1", "editor-2", "D:/project/src/main.ts");
      tm.onDeletePage("page1");

      const updates = tm.recomputeTitles("page1", "D:/project");
      expect(updates).toEqual([]);
    });
  });
});
