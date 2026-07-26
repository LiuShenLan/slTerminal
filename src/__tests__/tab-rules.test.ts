// tabRules 规则注册验证
import { describe, it, expect, afterEach } from "vitest";

// 导入 tabRules 触发 side-effect 注册，导入注册表验证注册结果
import "../panels/terminal/tabRules";
import { tabTitleRegistry } from "../panels/terminal/TabTitleRegistry";

describe("tabRules", () => {
  describe("side-effect import 验证", () => {
    // 此 describe 在 _reset() 之前执行，直接验证 tabRules.ts
    // 模块加载时的副作用生效——不依赖手动 register。

    it('import tabRules.ts 后 match("claude") 返回非空规则', () => {
      // tabRules.ts 在顶层 import 时已执行 register，
      // 此处直接 match 验证副作用生效
      const rule = tabTitleRegistry.match("claude");
      expect(rule).not.toBeNull();
      expect(rule!.command).toBe("claude");
      expect(rule!.title).toBe("claude");
    });

    it("规则不再携带 icon 字段（已由 P1-F3 hook 事件驱动设置）", () => {
      const rule = tabTitleRegistry.match("claude");
      expect(rule).not.toBeNull();
      // P1-F3-05: icon 已移除——claude 页签图标由 hook 事件驱动设置
      expect(rule!.icon).toBeUndefined();
    });
  });

  describe("手动注册（_reset 后）", () => {
    afterEach(() => {
      tabTitleRegistry._reset();
    });

    it("无 icon 注册 → match 返回 icon=undefined", () => {
      tabTitleRegistry.register({ command: "claude", title: "claude" });
      const rule = tabTitleRegistry.match("claude");
      expect(rule).not.toBeNull();
      expect(rule!.title).toBe("claude");
      expect(rule!.icon).toBeUndefined();
    });

    it('"claude update" 不匹配', () => {
      tabTitleRegistry.register({ command: "claude", title: "claude", icon: "/test.png" });
      expect(tabTitleRegistry.match("claude update")).toBeNull();
    });

    it("命令区分大小写", () => {
      tabTitleRegistry.register({ command: "claude", title: "claude", icon: "/test.png" });
      expect(tabTitleRegistry.match("Claude")).toBeNull();
    });

    it("同名命令重复注册后覆盖旧规则", () => {
      tabTitleRegistry.register({ command: "claude", title: "claude-v1", icon: "/old.png" });
      tabTitleRegistry.register({ command: "claude", title: "claude-v2", icon: "/new.png" });
      const rule = tabTitleRegistry.match("claude");
      expect(rule!.title).toBe("claude-v2");
      expect(rule!.icon).toBe("/new.png");
    });
  });
});
