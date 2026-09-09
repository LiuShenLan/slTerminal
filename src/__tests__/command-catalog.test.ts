// commandCatalog.test.ts — 命令目录单元测试
//
// 覆盖：10 条命令齐全 + 元数据正确、id 唯一、defaultKey 合法且对自身 context 非保留
//       （terminal.interrupt 为 CP-020 显式豁免——保留键守卫改豁免形态）、
//       COMMAND_META_BY_ID 查找、commandFromMeta 合并 handler + 未知 id 抛错。

import { describe, it, expect, vi } from "vitest";
import { COMMAND_CATALOG, COMMAND_META_BY_ID, commandFromMeta } from "../features/shortcuts/commandCatalog";
import { formatKeystroke, isValidKeystrokeString } from "../features/shortcuts/keystroke";
import { isReserved } from "../features/shortcuts/reserved";
import { UPLINK_MSG_TYPES } from "../panels/docViewer/previewMessages";

const EXPECTED_IDS = [
  "global.closeTab",
  "terminal.copy",
  "terminal.paste",
  "terminal.newline",
  "terminal.interrupt",
  "editor.save",
  "editor.toggleWordWrap",
  "explorer.delete",
  "explorer.open",
  "explorer.rename",
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

  it("每条 defaultKey 对自身 context 非保留；terminal.interrupt 为 CP-020 显式豁免", () => {
    for (const m of COMMAND_CATALOG) {
      if (m.id === "terminal.interrupt") {
        // CP-020 显式豁免：Ctrl+KeyC 是保留键（isReserved 仍拦用户覆盖——无法改绑/解绑），
        // 但命令代码默认键绑定它——interrupt 必须先于 \x03 占住 Ctrl+C 才能置状态再透传
        expect(isReserved(m.defaultKey!, m.context)).toBe(true);
        continue;
      }
      expect(isReserved(m.defaultKey!, m.context)).toBe(false);
    }
  });

  it("元数据字段完整（title/category/context/priority）", () => {
    for (const m of COMMAND_CATALOG) {
      expect(typeof m.title).toBe("string");
      expect(m.title.length).toBeGreaterThan(0);
      expect(["global", "terminal", "editor", "explorer"]).toContain(m.category);
      expect(typeof m.context).toBe("string");
      expect(typeof m.priority).toBe("number");
    }
  });

  // S10-②（CP-013）：原 SEC-04（D16）「预览上行含 global 键转发重放」通道随
  // webview 迁移整体退役（键盘不跨窗口）——守卫意图迁移为「预览消息通道不含
  // 命令重放」：上行类型集合 = 渲染态白名单（previewMessages 单点），含 key/
  // command 类即红；global 命令集仍保持最小低风险（其注册语义不受影响）
  it("预览消息通道不含命令重放（上行 = 渲染态白名单，CP-013/CP-044）", () => {
    // 恰好为渲染态集合 + TE-08 E2E 字体探针（slterm_font_probe——仅 VITE_E2E
    // 构建注入；守卫详值断言在 doc-viewer-preview-messages.test.ts）
    expect([...UPLINK_MSG_TYPES].sort()).toEqual(
      ["slterm_zoom", "slterm_scroll", "slterm_nav", "slterm_font_probe"].sort(),
    );
    // 显式锁死：无 key/command 类消息（改名字复活重放通道即红）
    for (const t of UPLINK_MSG_TYPES) {
      expect(t).not.toMatch(/key|command/i);
    }
    // global 命令集仍最小（防回潮性扩充）
    const globals = COMMAND_CATALOG.filter((m) => m.context === "global").map((m) => m.id);
    expect(globals).toEqual(["global.closeTab"]);
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
  it.each(EXPECTED_IDS)("合并 handler 与元数据：%s", (id) => {
    const handler = vi.fn(() => true);
    const meta = COMMAND_META_BY_ID.get(id);
    expect(meta).toBeDefined();
    const cmd = commandFromMeta(id, handler);
    // 统一断言 id/context/defaultKey/handler 四要素（STS-08：全 10 条参数化遍历）
    expect(cmd.id).toBe(id);
    expect(cmd.context).toBe(meta!.context);
    expect(cmd.defaultKey).toEqual(meta!.defaultKey);
    expect(cmd.handler).toBe(handler);
  });

  it("未知 id 抛错", () => {
    expect(() => commandFromMeta("does.not.exist", () => true)).toThrow(/未知命令 id/);
  });
});
