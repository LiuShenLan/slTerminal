// SchemeRegistry 单元测试（spec §7.2，TST-02）
// 注册表单例测试模式照 tab-title-registry.test.ts 先例——测试经模块级单例
// schemeRegistry 驱动，beforeEach 调 _reset() 隔离（_reset 清空注册表 + active 复位 darcula）。
// darcula 四段完整性断言的键数/结构以 src/theme/schemes/darcula.ts 实值为准。
import { describe, it, expect, beforeEach, vi } from "vitest";
import { schemeRegistry } from "../theme/schemeRegistry";
import { darcula } from "../theme/schemes/darcula";
import type { ColorScheme } from "../theme/schemes/types";

// 测试用方案工厂——基于 darcula 派生（注册表行为测试只需 id/label 区分，色值不参与断言）
function makeScheme(id: string, label: string): ColorScheme {
  return { ...darcula, id, label };
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
      schemeRegistry.register(darcula);
      expect(schemeRegistry.get("not-exist")).toBeUndefined();
    });

    it("getAll 返回全部已注册方案且为注册序", () => {
      const a = makeScheme("a", "A");
      const b = makeScheme("b", "B");
      schemeRegistry.register(a);
      schemeRegistry.register(b);
      expect(schemeRegistry.getAll()).toEqual([a, b]);
    });

    it("getDefaultId 返回 darcula（默认方案 id）", () => {
      expect(schemeRegistry.getDefaultId()).toBe("darcula");
    });
  });

  describe("active 切换", () => {
    it("getActive 未 setActive 时默认返回 darcula", () => {
      schemeRegistry.register(darcula);
      expect(schemeRegistry.getActive()).toBe(darcula);
    });

    it("setActive 已知 id 后 getActive 返回新方案", () => {
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(darcula);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      expect(schemeRegistry.getActive()).toBe(custom);
    });

    it("setActive 已在多个方案间反复切换", () => {
      const a = makeScheme("a", "A");
      const b = makeScheme("b", "B");
      schemeRegistry.register(darcula);
      schemeRegistry.register(a);
      schemeRegistry.register(b);
      schemeRegistry.setActive("a");
      expect(schemeRegistry.getActive()).toBe(a);
      schemeRegistry.setActive("b");
      expect(schemeRegistry.getActive()).toBe(b);
      schemeRegistry.setActive("darcula");
      expect(schemeRegistry.getActive()).toBe(darcula);
    });

    it("setActive 未知 id → 回退 darcula + console.warn", () => {
      const warnSpy = vi.spyOn(console, "warn");
      schemeRegistry.register(darcula);
      schemeRegistry.setActive("not-exist");
      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(warnSpy).toHaveBeenCalledWith(
        '[scheme] 未知配色方案 "not-exist"，回退到默认方案 darcula',
      );
      expect(schemeRegistry.getActive()).toBe(darcula);
      warnSpy.mockRestore();
    });

    it("setActive 未知 id 不注册该 id 的条目", () => {
      schemeRegistry.register(darcula);
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
      schemeRegistry.register(darcula);
      schemeRegistry.register(v1);
      schemeRegistry.setActive("custom");
      schemeRegistry.register(v2);
      expect(schemeRegistry.getActive()).toBe(v2);
    });
  });

  describe("_reset 隔离", () => {
    it("_reset() 清空注册表——get 返回 undefined", () => {
      schemeRegistry.register(darcula);
      schemeRegistry._reset();
      expect(schemeRegistry.get("darcula")).toBeUndefined();
    });

    it("_reset() 后 active 复位默认——注册 darcula 后 getActive 直接返回 darcula", () => {
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(darcula);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      schemeRegistry._reset();
      schemeRegistry.register(darcula);
      expect(schemeRegistry.getActive()).toBe(darcula);
    });

    it("_reset() 后可重新注册并正常切换", () => {
      schemeRegistry.register(darcula);
      schemeRegistry._reset();
      const custom = makeScheme("custom", "Custom");
      schemeRegistry.register(darcula);
      schemeRegistry.register(custom);
      schemeRegistry.setActive("custom");
      expect(schemeRegistry.getActive()).toBe(custom);
    });
  });

  describe("darcula 四段完整性", () => {
    it("ui 段——6 组键数 7/3/5/8/3/3 + 23 标量", () => {
      expect(darcula.id).toBe("darcula");
      expect(darcula.label).toBe("Darcula");
      // 6 组
      expect(Object.keys(darcula.ui.gitFile).length).toBe(7);
      expect(Object.keys(darcula.ui.gitGutter).length).toBe(3);
      expect(Object.keys(darcula.ui.explorer).length).toBe(5);
      expect(Object.keys(darcula.ui.sidebar).length).toBe(8);
      expect(Object.keys(darcula.ui.errorBanner).length).toBe(3);
      expect(Object.keys(darcula.ui.agentStatusUsage).length).toBe(3);
      // 23 标量（标量均为 string，组为嵌套对象）
      const scalarCount = (Object.keys(darcula.ui) as Array<keyof typeof darcula.ui>).filter(
        (k) => typeof darcula.ui[k] === "string",
      ).length;
      expect(scalarCount).toBe(23);
      // 关键标量值对齐 darcula.ts
      expect(darcula.ui.panelBg).toBe("#1E1E1E");
      expect(darcula.ui.appBgPrimary).toBe("#1e1e2e");
    });

    it("terminal 段——25 键", () => {
      expect(Object.keys(darcula.terminal).length).toBe(25);
      // 关键值对齐 darcula.ts（与 panels/terminal/theme.ts 现状一致）
      expect(darcula.terminal.background).toBe("#1E1E1E");
      expect(darcula.terminal.foreground).toBe("#D4D4D4");
      expect(darcula.terminal.cursorAccent).toBe("#1E1E1E");
      expect(darcula.terminal.brightWhite).toBe("#FFFFFF");
    });

    it("editor 段——theme 透出非 undefined + overrides 结构完整", () => {
      expect(darcula.editor.theme).toBeDefined();
      expect(darcula.editor.overrides.background).toBe("#282C34");
      expect(Object.keys(darcula.editor.overrides.lint).length).toBe(7);
      expect(Object.keys(darcula.editor.overrides.searchMatch).length).toBe(4);
    });

    it("libraries 段——dockview 20 条 + allotment 2 键", () => {
      expect(Object.keys(darcula.libraries.dockview).length).toBe(20);
      // 全部为 "--dv-" 前缀 CSS 变量
      for (const key of Object.keys(darcula.libraries.dockview)) {
        expect(key.startsWith("--dv-")).toBe(true);
      }
      expect(darcula.libraries.dockview["--dv-group-view-background-color"]).toBe("#1E1E1E");
      expect(Object.keys(darcula.libraries.allotment)).toEqual([
        "separatorBorder",
        "focusBorder",
      ]);
    });
  });
});
