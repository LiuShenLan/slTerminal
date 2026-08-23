// SideViewRegistry 单元测试
//
// 照 tab-title-registry.test.ts 模式：每个 test case 使用全新实例，
// 覆盖 register/get/getAll、重复注册覆盖、_reset 隔离、单例校验。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup } from "@testing-library/react";
import {
  SideViewRegistry,
  sideViewRegistry,
} from "../features/sideViews/sideViewRegistry";
// side-effect import：触发三条视图注册（nav/explorer/commit）——本文件唯一注册源
import "../features/sideViews/sideViewDefs";
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
        icon: StubComponent,
        component: StubComponent,
      });
      registry.register({
        id: "explorer",
        title: "文件浏览器",
        icon: StubComponent,
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
        icon: StubComponent,
        component: StubComponent,
      });

      const def = registry.get("projects");
      expect(def).toBeDefined();
      expect(def!.id).toBe("projects");
      expect(def!.title).toBe("项目列表");
      expect(def!.icon).toBe(StubComponent);
      expect(def!.component).toBe(StubComponent);
    });

    it("get 未注册 id 返回 undefined", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: StubComponent,
        component: StubComponent,
      });

      expect(registry.get("explorer")).toBeUndefined();
    });

    it("同 id 重复注册覆盖旧定义且不重复计数", () => {
      registry.register({
        id: "projects",
        title: "旧标题",
        icon: StubComponent,
        component: StubComponent,
      });
      registry.register({
        id: "projects",
        title: "新标题",
        icon: StubComponent,
        component: StubComponent,
      });

      const def = registry.get("projects");
      expect(def).toBeDefined();
      expect(def!.title).toBe("新标题");
      expect(def!.icon).toBe(StubComponent);
      // 同 id 覆盖不重复计数
      expect(registry.getAll()).toHaveLength(1);
    });
  });

  describe("生命周期", () => {
    it("_reset() 清空所有定义——get 任何 id 返回 undefined", () => {
      registry.register({
        id: "projects",
        title: "项目列表",
        icon: StubComponent,
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
        icon: StubComponent,
        component: StubComponent,
      });
      registry._reset();
      registry.register({
        id: "explorer",
        title: "文件浏览器",
        icon: StubComponent,
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

// ── sideViewDefs 常量守卫（TQ-COV-10）──
//
// sideViewDefs.ts 是 side-effect 注册文件（NAV-05 三槽）——import 即注册，
// 且只执行一次（模块缓存）：静态 import 后注册态恒在，直至 afterEach _reset。
// 守卫注册序精确契约：id 集合恒为 ["nav", "explorer", "commit"]，
// 新增视图必须经此文件追加注册（防止散落注册/重复注册/顺序漂移）。
// 断言针对全局单例（真实注册目标）；afterEach _reset 保证用例隔离
// （注册表家族通用契约，硬约束 #13）。

describe("sideViewDefs 注册守卫（TQ-COV-10）", () => {
  afterEach(() => {
    sideViewRegistry._reset();
  });

  it("import 触发注册三条视图：id 集合精确为 [nav, explorer, commit]（注册序），组件可挂载渲染", () => {
    const all = sideViewRegistry.getAll();
    const ids = all.map((def) => def.id);

    // 注册序精确契约（NAV-05 三槽）
    expect(ids).toEqual(["nav", "explorer", "commit"]);
    // title 契约文案
    expect(all[0]).toMatchObject({ id: "nav", title: "导航树" });
    expect(all[1]).toMatchObject({ id: "explorer", title: "文件浏览器" });
    expect(all[2]).toMatchObject({ id: "commit", title: "Commit" });
    // 图标与视图组件必须为可渲染组件（React component type）
    for (const def of all) {
      expect(typeof def.icon).toBe("function");
      expect(typeof def.component).toBe("function");
    }
    // 「配置」钮不入注册表（NAV-05：config 由 ActivityBar 底部固定渲染）
    expect(sideViewRegistry.get("config")).toBeUndefined();
    expect(all.some((def) => def.id === "config")).toBe(false);

    // 组件 smoke：三视图组件函数真实执行（TQ-COV-10——sideViewDefs 函数 100% 目标）。
    // 无活跃项目/无历史数据环境：NavTree 空态、ExplorerPanel/CommitView no-root 态
    // （useAgentHistory 扫描失败被 hook 内 catch，空历史渲染，不炸）
    for (const def of all) {
      const { container } = render(
        React.createElement(def.component, {} as never),
      );
      expect(container.querySelector("*")).toBeTruthy();
      cleanup();
    }
  });
});
