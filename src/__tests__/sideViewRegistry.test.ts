// SideViewRegistry 单元测试
//
// 照 tab-title-registry.test.ts 模式：每个 test case 使用全新实例，
// 覆盖 register/get/getAll、重复注册覆盖、_reset 隔离、单例校验。

import { describe, it, expect, beforeEach } from "vitest";
import {
  SideViewRegistry,
  sideViewRegistry,
} from "../features/sideViews/sideViewRegistry";
/** 测试用 stub 组件——不渲染任何内容 */
function StubComponent(): null {
  return null;
}

describe("SideViewRegistry", () => {
  let registry: SideViewRegistry;

  // 每个 test case 使用全新实例，避免单例状态污染
  beforeEach(() => {
    registry = new SideViewRegistry();
  });

  describe("注册与查询", () => {
    it("register 后 getAll 按注册序返回全部定义", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: "📋",
        component: StubComponent,
      });
      registry.register({
        id: "explorer",
        title: "文件浏览器",
        icon: "📁",
        component: StubComponent,
      });

      const all = registry.getAll();
      expect(all).toHaveLength(2);
      expect(all[0].id).toBe("projects");
      expect(all[0].title).toBe("项目列表");
      expect(all[1].id).toBe("explorer");
    });

    it("get 已注册 id 返回对应定义", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: "📋",
        component: StubComponent,
      });

      const def = registry.get("projects");
      expect(def).toBeDefined();
      expect(def!.id).toBe("projects");
      expect(def!.title).toBe("项目列表");
      expect(def!.icon).toBe("📋");
      expect(def!.component).toBe(StubComponent);
    });

    it("get 未注册 id 返回 undefined", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: "📋",
        component: StubComponent,
      });

      expect(registry.get("explorer")).toBeUndefined();
    });

    it("同 id 重复注册覆盖旧定义且不重复计数", () => {
      registry.register({
        id: "projects",
        title: "旧标题",
        icon: "📋",
        component: StubComponent,
      });
      registry.register({
        id: "projects",
        title: "新标题",
        icon: "🆕",
        component: StubComponent,
      });

      const def = registry.get("projects");
      expect(def).toBeDefined();
      expect(def!.title).toBe("新标题");
      expect(def!.icon).toBe("🆕");
      // 同 id 覆盖不重复计数
      expect(registry.getAll()).toHaveLength(1);
    });
  });

  describe("生命周期", () => {
    it("_reset() 清空所有定义——get 任何 id 返回 undefined", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: "📋",
        component: StubComponent,
      });
      registry._reset();

      expect(registry.get("projects")).toBeUndefined();
      expect(registry.getAll()).toHaveLength(0);
    });

    it("_reset() 后可重新 register 新定义", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: "📋",
        component: StubComponent,
      });
      registry._reset();
      registry.register({
        id: "explorer",
        title: "文件浏览器",
        icon: "📁",
        component: StubComponent,
      });

      expect(registry.get("projects")).toBeUndefined();
      expect(registry.get("explorer")).toBeDefined();
      expect(registry.get("explorer")!.id).toBe("explorer");
      expect(registry.getAll()).toHaveLength(1);
    });
  });

  describe("单例", () => {
    it("全局单例存在且为 SideViewRegistry 实例", () => {
      expect(sideViewRegistry).toBeDefined();
      expect(sideViewRegistry).toBeInstanceOf(SideViewRegistry);
    });
  });
});
