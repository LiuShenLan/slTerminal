// TabTitleRegistry 单元测试
import { describe, it, expect, beforeEach } from "vitest";
import { TabTitleRegistry, tabTitleRegistry } from "../panels/terminal/TabTitleRegistry";

describe("TabTitleRegistry", () => {
  let registry: TabTitleRegistry;

  // 每个 test case 使用全新实例
  beforeEach(() => {
    registry = new TabTitleRegistry();
  });

  describe("注册与匹配", () => {
    it("register 后 match 精确命令返回对应规则", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      const rule = registry.match("claude");
      expect(rule).not.toBeNull();
      expect(rule!.command).toBe("claude");
      expect(rule!.title).toBe("claude");
      expect(rule!.icon).toBe("/icon.png");
    });

    it("match 未注册命令返回 null", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      expect(registry.match("npm")).toBeNull();
    });

    it("match 空字符串返回 null", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      expect(registry.match("")).toBeNull();
    });

    it("命令区分大小写（Claude !== claude）", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      expect(registry.match("Claude")).toBeNull();
      expect(registry.match("CLAUDE")).toBeNull();
    });

    it("同名命令重复注册后覆盖旧规则（取最后一条）", () => {
      registry.register({ command: "claude", title: "claude-v1", icon: "/old.png" });
      registry.register({ command: "claude", title: "claude-v2", icon: "/new.png" });
      const rule = registry.match("claude");
      expect(rule!.title).toBe("claude-v2");
      expect(rule!.icon).toBe("/new.png");
    });
  });

  describe("生命周期", () => {
    it("_reset() 清空所有规则——match 任何命令返回 null", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      registry._reset();
      expect(registry.match("claude")).toBeNull();
    });

    it("_reset() 后可重新 register 新规则", () => {
      registry.register({ command: "claude", title: "claude", icon: "/icon.png" });
      registry._reset();
      registry.register({ command: "git", title: "git", icon: "/git.png" });
      expect(registry.match("claude")).toBeNull();
      expect(registry.match("git")).not.toBeNull();
    });
  });

  describe("单例", () => {
    it("全局单例存在且为 TabTitleRegistry 实例", () => {
      // 模块顶层 import 的 tabTitleRegistry 验证单例模式
      expect(tabTitleRegistry).toBeDefined();
      expect(tabTitleRegistry).toBeInstanceOf(TabTitleRegistry);
    });
  });
});
