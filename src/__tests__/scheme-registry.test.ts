// SchemeRegistry 单元测试（spec §7.2，TST-02）
// 注册表单例测试模式照 tab-title-registry.test.ts 先例——测试经模块级单例
// schemeRegistry 驱动，beforeEach 调 _reset() 隔离（_reset 清空注册表 + active 复位 linear）。
// linear 四段完整性断言的键数/结构以 src/theme/schemes/linear.ts 实值为准。
import { describe, it, expect, beforeEach, vi } from "vitest";
import { schemeRegistry } from "../theme/schemeRegistry";
import { linear } from "../theme/schemes/linear";
import type { ColorScheme } from "../theme/schemes/types";

// 测试用方案工厂——基于 linear 派生（注册表行为测试只需 id/label 区分，色值不参与断言）
function makeScheme(id: string, label: string): ColorScheme {
  return { ...linear, id, label };
}

describe("SchemeRegistry", () => {
  // 每个 test case 清空注册表 + active 复位（照 tab-title-registry 先例）
  beforeEach(() => {
    schemeRegistry._reset();
  });

  describe("注册与查询", () => {
    it("register 后 get 返回注册的方案", () => {
      const scheme = makeScheme("custom", "Custom");
      schemeRegistry.register(scheme);
      expect(schemeRegistry.get("custom")).toBe(scheme);
    });

    it("get 未注册 id 返回 undefined", () => {
      schemeRegistry.register(linear);
      expect(schemeRegistry.get("not-exist")).toBeUndefined();
    });

    it("getAll 返回全部已注册方案且为注册序", () => {
      const a = makeScheme("a", "A");
      const b = makeScheme("b", "B");
      schemeRegistry.register(a);
      schemeRegistry.register(b);
      expect(schemeRegistry.getAll()).toEqual([a, b]);
    });

    it("getDefaultId 返回 linear（默认方案 id）", () => {
      expect(schemeRegistry.getDefaultId()).toBe("linear");
    });
  });

  describe("active 切换", () => {
    it("getActive 未 setActive 时默认返回 linear", () => {
      schemeRegistry.register(linear);
      expect(schemeRegistry.getActive()).toBe(linear);
    });

    it("setActive 已知 id 后 getActive 返回新方案", () => {
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(linear);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      expect(schemeRegistry.getActive()).toBe(custom);
    });

    it("setActive 已在多个方案间反复切换", () => {
      const a = makeScheme("a", "A");
      const b = makeScheme("b", "B");
      schemeRegistry.register(linear);
      schemeRegistry.register(a);
      schemeRegistry.register(b);
      schemeRegistry.setActive("a");
      expect(schemeRegistry.getActive()).toBe(a);
      schemeRegistry.setActive("b");
      expect(schemeRegistry.getActive()).toBe(b);
      schemeRegistry.setActive("linear");
      expect(schemeRegistry.getActive()).toBe(linear);
    });

    it("setActive 未知 id → 回退 linear + console.warn", () => {
      const warnSpy = vi.spyOn(console, "warn");
      schemeRegistry.register(linear);
      schemeRegistry.setActive("not-exist");
      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(warnSpy).toHaveBeenCalledWith(
        '[scheme] 未知配色方案 "not-exist"，回退到默认方案 linear',
      );
      expect(schemeRegistry.getActive()).toBe(linear);
      warnSpy.mockRestore();
    });

    it("setActive 未知 id 不注册该 id 的条目", () => {
      schemeRegistry.register(linear);
      schemeRegistry.setActive("ghost");
      expect(schemeRegistry.get("ghost")).toBeUndefined();
    });
  });

  describe("重复注册覆盖", () => {
    it("同 id 重复注册后 get 返回新方案（取最后一条）", () => {
      const v1 = makeScheme("custom", "v1");
      const v2 = makeScheme("custom", "v2");
      schemeRegistry.register(v1);
      schemeRegistry.register(v2);
      expect(schemeRegistry.get("custom")).toBe(v2);
      expect(schemeRegistry.getAll()).toEqual([v2]);
    });

    it("重复注册覆盖后 active 指向新对象（引用跟随更新）", () => {
      const v1 = makeScheme("custom", "v1");
      const v2 = makeScheme("custom", "v2");
      schemeRegistry.register(linear);
      schemeRegistry.register(v1);
      schemeRegistry.setActive("custom");
      schemeRegistry.register(v2);
      expect(schemeRegistry.getActive()).toBe(v2);
    });
  });

  describe("_reset 隔离", () => {
    it("_reset() 清空注册表——get 返回 undefined", () => {
      schemeRegistry.register(linear);
      schemeRegistry._reset();
      expect(schemeRegistry.get("linear")).toBeUndefined();
    });

    it("_reset() 后 active 复位默认——注册 linear 后 getActive 直接返回 linear", () => {
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(linear);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      schemeRegistry._reset();
      schemeRegistry.register(linear);
      expect(schemeRegistry.getActive()).toBe(linear);
    });

    it("_reset() 后可重新注册并正常切换", () => {
      schemeRegistry.register(linear);
      schemeRegistry._reset();
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(linear);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      expect(schemeRegistry.getActive()).toBe(custom);
    });
  });

  describe("linear 四段完整性", () => {
    it("ui 段——6 组键数 7/3/5/8/3/4 + 27 标量", () => {
      expect(linear.id).toBe("linear");
      expect(linear.label).toBe("Linear");
      // 6 组
      expect(Object.keys(linear.ui.gitFile).length).toBe(7);
      expect(Object.keys(linear.ui.gitGutter).length).toBe(3);
      expect(Object.keys(linear.ui.explorer).length).toBe(5);
      expect(Object.keys(linear.ui.sidebar).length).toBe(8);
      expect(Object.keys(linear.ui.errorBanner).length).toBe(3);
      expect(Object.keys(linear.ui.agentStatusUsage).length).toBe(4);
      // 27 标量（23 既有 + accentFg/selectionHoverBg/titlebarBg/titlebarCloseHover 4 新增，
      // 以 linear.ts ui 段实际键数为准——文档口径冲突时改文档不改断言，TQ-C-03）
      const scalarCount = (Object.keys(linear.ui) as Array<keyof typeof linear.ui>).filter(
        (k) => typeof linear.ui[k] === "string",
      ).length;
      expect(scalarCount).toBe(27);
      // 关键标量值对齐 linear.ts（附录 A 契约）
      expect(linear.ui.panelBg).toBe("#0a0a0b");
      expect(linear.ui.appBgPrimary).toBe("#0a0a0b");
      // 新增 4 标量落位
      expect(linear.ui.accentFg).toBe("#8fb4f5");
      expect(linear.ui.selectionHoverBg).toBe("rgba(110,159,242,0.22)");
      expect(linear.ui.titlebarBg).toBe("#141416");
      expect(linear.ui.titlebarCloseHover).toBe("#c04747");
    });

    it("terminal 段——25 键", () => {
      expect(Object.keys(linear.terminal).length).toBe(25);
      // 关键值对齐 linear.ts（与 panels/terminal/theme.ts 现状一致，附录 A 契约）
      expect(linear.terminal.background).toBe("#0a0a0b");
      expect(linear.terminal.foreground).toBe("#cfcac1");
      expect(linear.terminal.cursorAccent).toBe("#0a0a0b");
      expect(linear.terminal.brightWhite).toBe("#f0ede8");
    });

    it("editor 段——theme 透出非 undefined + overrides 结构完整（含 syntax 9 键 + plainText 3 键）", () => {
      expect(linear.editor.theme).toBeDefined();
      expect(linear.editor.overrides.background).toBe("#0a0a0b");
      expect(Object.keys(linear.editor.overrides.lint).length).toBe(7);
      expect(Object.keys(linear.editor.overrides.searchMatch).length).toBe(4);
      // 新增 syntax 子组 9 键（TH-01）
      expect(Object.keys(linear.editor.overrides.syntax).length).toBe(9);
      // 新增 plainText/lineNumber/lineNumberActive 3 键
      expect(linear.editor.overrides.plainText).toBe("#b3aea6");
      expect(linear.editor.overrides.lineNumber).toBe("#6b675f");
      expect(linear.editor.overrides.lineNumberActive).toBe("#b3aea6");
    });

    it("libraries 段——dockview 20 条 + allotment 2 键", () => {
      expect(Object.keys(linear.libraries.dockview).length).toBe(20);
      // 全部为 "--dv-" 前缀 CSS 变量
      for (const key of Object.keys(linear.libraries.dockview)) {
        expect(key.startsWith("--dv-")).toBe(true);
      }
      expect(linear.libraries.dockview["--dv-group-view-background-color"]).toBe("#0a0a0b");
      expect(Object.keys(linear.libraries.allotment)).toEqual([
        "separatorBorder",
        "focusBorder",
      ]);
    });
  });
});
