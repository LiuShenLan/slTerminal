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

    it("规则 icon 字段为 Vite import 的真实 URL（非空字符串）", () => {
      const rule = tabTitleRegistry.match("claude");
      expect(rule).not.toBeNull();
      // Vite PNG import 返回 URL 字符串（dev: /src/assets/..., build: /assets/...）
      expect(typeof rule!.icon).toBe("string");
      expect(rule!.icon.length).toBeGreaterThan(0);
    });
  });

  describe("手动注册（_reset 后）", () => {
    afterEach(() => {
      tabTitleRegistry._reset();
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
