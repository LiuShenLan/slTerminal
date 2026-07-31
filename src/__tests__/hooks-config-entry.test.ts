// hooks-config-entry.test.ts — global.openHooksConfig 入口命令测试
//
// 覆盖路径：
//   1. createGlobalShortcuts 返回两条命令，第二条为 global.openHooksConfig（元数据完整）
//   2. handler 首次触发 → addPanel（id = hooksConfig-{activePageId}，契约 C13-7 单例规则）
//   3. handler 重复触发 → getPanel 命中 → focus() 聚焦不新建
//   4. handler：无活跃页面 → 返回 false 透传
//   5. handler：无 DockviewApi → 返回 false 透传
//   6. handler：activePageId 变化 → 面板 id 跟随（同页单例）

import { describe, it, expect, vi, beforeEach } from "vitest";
import { createGlobalShortcuts } from "../features/shortcuts/globalCommands";
import { useLayout } from "../stores/layout";
import type { DockviewApi, IDockviewPanel } from "dockview-react";

/**
 * 构造 DockviewApi stub：getPanel/addPanel 共享一个面板 Map——
 * addPanel 后 getPanel 即可命中，模拟真实 Dockview 行为。
 */
function dockviewApiStub(): {
  api: DockviewApi;
  addPanel: ReturnType<typeof vi.fn>;
  focus: ReturnType<typeof vi.fn>;
} {
  const focus = vi.fn();
  const panels = new Map<string, IDockviewPanel>();
  const addPanel = vi.fn((params: { id: string }) => {
    panels.set(params.id, { focus } as unknown as IDockviewPanel);
  });
  const getPanel = vi.fn((id: string) => panels.get(id));
  return { api: { getPanel, addPanel } as unknown as DockviewApi, addPanel, focus };
}

describe("createGlobalShortcuts — global.openHooksConfig", () => {
  beforeEach(() => {
    // 重置布局 store（真实 Zustand store，Zustand setState 浅合并）
    useLayout.setState({ activePageId: null });
  });

  function openHooksCommand() {
    const cmds = createGlobalShortcuts(() => undefined);
    expect(cmds).toHaveLength(2);
    return cmds[1];
  }

  describe("命令结构", () => {
    it("工厂返回两条命令，第二条为 global.openHooksConfig", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds).toHaveLength(2);
      expect(cmds[0].id).toBe("global.closeTab");
      expect(cmds[1].id).toBe("global.openHooksConfig");
    });

    it("元数据完整（title/category/context/priority/defaultKey）", () => {
      const cmd = openHooksCommand();
      expect(cmd.title).toBe("打开 Hooks 配置");
      expect(cmd.category).toBe("global");
      expect(cmd.context).toBe("global");
      expect(cmd.priority).toBe(10);
      const ks = cmd.defaultKey!;
      expect(ks.ctrlKey).toBe(true);
      expect(ks.shiftKey).toBe(true);
      expect(ks.altKey).toBe(false);
      expect(ks.metaKey).toBe(false);
      expect(ks.code).toBe("KeyH");
    });
  });

  describe("handler 同页单例（契约 C13-7）", () => {
    it("首次触发 → addPanel（id = hooksConfig-{activePageId}），返回 true", () => {
      useLayout.setState({ activePageId: "page1" });
      const { api, addPanel } = dockviewApiStub();
      const cmds = createGlobalShortcuts(() => api);

      const result = cmds[1].handler(new KeyboardEvent("keydown"));

      expect(result).toBe(true);
      expect(addPanel).toHaveBeenCalledOnce();
      expect(addPanel).toHaveBeenCalledWith({
        id: "hooksConfig-page1",
        component: "hooksConfig",
        title: "Hooks 配置",
        params: { panelId: "hooksConfig-page1" },
      });
    });

    it("重复触发 → getPanel 命中 → focus() 聚焦不新建，返回 true", () => {
      useLayout.setState({ activePageId: "page1" });
      const { api, addPanel, focus } = dockviewApiStub();
      const cmds = createGlobalShortcuts(() => api);

      cmds[1].handler(new KeyboardEvent("keydown")); // 首次新建
      const result = cmds[1].handler(new KeyboardEvent("keydown")); // 重复触发

      expect(result).toBe(true);
      expect(addPanel).toHaveBeenCalledOnce(); // 不新建
      expect(focus).toHaveBeenCalledOnce(); // 聚焦已有面板
    });

    it("activePageId 变化 → 面板 id 跟随（每页独立单例）", () => {
      useLayout.setState({ activePageId: "page1" });
      const { api, addPanel, focus } = dockviewApiStub();
      const cmds = createGlobalShortcuts(() => api);

      cmds[1].handler(new KeyboardEvent("keydown"));
      expect(addPanel).toHaveBeenCalledWith({
        id: "hooksConfig-page1",
        component: "hooksConfig",
        title: "Hooks 配置",
        params: { panelId: "hooksConfig-page1" },
      });

      // 切换到 page2 → 新建 hooksConfig-page2；page1 面板保持不动
      useLayout.setState({ activePageId: "page2" });
      cmds[1].handler(new KeyboardEvent("keydown"));
      expect(addPanel).toHaveBeenCalledTimes(2);
      expect(addPanel).toHaveBeenLastCalledWith({
        id: "hooksConfig-page2",
        component: "hooksConfig",
        title: "Hooks 配置",
        params: { panelId: "hooksConfig-page2" },
      });
      expect(focus).not.toHaveBeenCalled();
    });
  });

  describe("handler 透传分支", () => {
    it("无活跃页面（activePageId 为 null）→ 返回 false，不调 getPanel/addPanel", () => {
      const { api, addPanel } = dockviewApiStub();
      const cmds = createGlobalShortcuts(() => api);

      const result = cmds[1].handler(new KeyboardEvent("keydown"));

      expect(result).toBe(false);
      expect(addPanel).not.toHaveBeenCalled();
    });

    it("无 DockviewApi → 返回 false 透传", () => {
      useLayout.setState({ activePageId: "page1" });
      const cmds = createGlobalShortcuts(() => undefined);

      const result = cmds[1].handler(new KeyboardEvent("keydown"));

      expect(result).toBe(false);
    });
  });
});
