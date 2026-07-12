// commandCatalog.test.ts — 命令目录单元测试
//
// 覆盖：5 条命令齐全 + 元数据正确、id 唯一、defaultKey 合法且对自身 context 非保留、
//       COMMAND_META_BY_ID 查找、commandFromMeta 合并 handler + 未知 id 抛错。

import { describe, it, expect, vi } from "vitest";
import { COMMAND_CATALOG, COMMAND_META_BY_ID, commandFromMeta } from "../features/shortcuts/commandCatalog";
import { formatKeystroke, isValidKeystrokeString } from "../features/shortcuts/keystroke";
import { isReserved } from "../features/shortcuts/reserved";

const EXPECTED_IDS = [
  "global.closeTab",
  "terminal.copy",
  "terminal.paste",
  "terminal.newline",
  "editor.save",
  "editor.toggleWordWrap",
];

describe("COMMAND_CATALOG", () => {
  it("包含全部预期命令", () => {
    const ids = COMMAND_CATALOG.map((m) => m.id);
    for (const id of EXPECTED_IDS) {
      expect(ids).toContain(id);
    }
    expect(COMMAND_CATALOG).toHaveLength(EXPECTED_IDS.length);
  });

  it("id 全局唯一", () => {
    const ids = COMMAND_CATALOG.map((m) => m.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("每条 defaultKey 均为合法 keystroke", () => {
    for (const m of COMMAND_CATALOG) {
      expect(m.defaultKey).not.toBeNull();
      expect(isValidKeystrokeString(formatKeystroke(m.defaultKey!))).toBe(true);
    }
  });

  it("每条 defaultKey 对自身 context 非保留（不与锁定键冲突）", () => {
    for (const m of COMMAND_CATALOG) {
      expect(isReserved(m.defaultKey!, m.context)).toBe(false);
    }
  });

  it("元数据字段完整（title/category/context/priority）", () => {
    for (const m of COMMAND_CATALOG) {
      expect(typeof m.title).toBe("string");
      expect(m.title.length).toBeGreaterThan(0);
      expect(["global", "terminal", "editor"]).toContain(m.category);
      expect(typeof m.context).toBe("string");
      expect(typeof m.priority).toBe("number");
    }
  });
});

describe("COMMAND_META_BY_ID", () => {
  it("按 id 查得元数据", () => {
    const meta = COMMAND_META_BY_ID.get("terminal.copy");
    expect(meta).toBeDefined();
    expect(meta!.defaultKey).toEqual({
      ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, code: "KeyC",
    });
  });

  it("未知 id 返回 undefined", () => {
    expect(COMMAND_META_BY_ID.get("nope")).toBeUndefined();
  });
});

describe("commandFromMeta", () => {
  it("合并 handler 与元数据", () => {
    const handler = vi.fn(() => true);
    const cmd = commandFromMeta("editor.save", handler);
    expect(cmd.id).toBe("editor.save");
    expect(cmd.context).toBe("editor");
    expect(cmd.defaultKey).toEqual({
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyS",
    });
    expect(cmd.handler).toBe(handler);
  });

  it("未知 id 抛错", () => {
    expect(() => commandFromMeta("does.not.exist", () => true)).toThrow(/未知命令 id/);
  });

  it("合并 editor.toggleWordWrap handler 与元数据", () => {
    const handler = vi.fn(() => true);
    const cmd = commandFromMeta("editor.toggleWordWrap", handler);
    expect(cmd.id).toBe("editor.toggleWordWrap");
    expect(cmd.context).toBe("editor");
    expect(cmd.defaultKey).toEqual({
      ctrlKey: false, shiftKey: false, altKey: true, metaKey: false, code: "KeyZ",
    });
    expect(cmd.handler).toBe(handler);
  });
});
