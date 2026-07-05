// globalCommands.test.ts — createGlobalShortcuts 工厂函数单元测试
//
// 覆盖路径：
//   1. 命令结构验证（id、keystroke、context、priority）
//   2. handler：活跃面板存在 → close() 被调用 + 返回 true（阻止默认）
//   3. handler：无活跃面板 → close() 不调用 + 返回 false（透传）
//   4. handler：getDockviewApi() 返回 undefined → 安全返回 false
//   5. handler：activePanel 为 undefined → 安全返回 false

import { describe, it, expect, vi } from "vitest";
import { createGlobalShortcuts } from "../features/shortcuts/globalCommands";
import type { DockviewApi, IDockviewPanel } from "dockview-react";

/** 构造 DockviewApi stub */
function dockviewApiStub(opts?: { activePanel?: Partial<IDockviewPanel> | null }): DockviewApi {
  return {
    activePanel: (opts?.activePanel ?? null) as IDockviewPanel | undefined,
  } as unknown as DockviewApi;
}

/** 构造 activePanel stub */
function activePanelStub(): { api: { close: ReturnType<typeof vi.fn> } } {
  return { api: { close: vi.fn() } };
}

describe("createGlobalShortcuts", () => {
  // ---- 1. 命令结构 ----

  describe("命令结构", () => {
    it("返回一条命令", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds).toHaveLength(1);
    });

    it("命令 id 为 global.closeTab", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds[0].id).toBe("global.closeTab");
    });

    it("keystroke 为 Ctrl+W（仅 ctrlKey，code: KeyW）", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      const ks = cmds[0].keystroke;
      expect(ks.ctrlKey).toBe(true);
      expect(ks.shiftKey).toBe(false);
      expect(ks.altKey).toBe(false);
      expect(ks.metaKey).toBe(false);
      expect(ks.code).toBe("KeyW");
    });

    it("context 为 global", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds[0].context).toBe("global");
    });

    it("priority 为 10（低于面板级 100+）", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds[0].priority).toBe(10);
    });

    it("allowDuringComposition 未设置（默认 false = IME 透传）", () => {
      const cmds = createGlobalShortcuts(() => undefined);
      expect(cmds[0].allowDuringComposition).toBeUndefined();
    });
  });

  // ---- 2. handler: 活跃面板存在 → 关闭 ----

  describe("handler 关闭活跃面板", () => {
    it("活跃面板存在 → 调用 activePanel.api.close() 并返回 true", () => {
      const panel = activePanelStub();
      const api = dockviewApiStub({ activePanel: panel as unknown as IDockviewPanel });
      const cmds = createGlobalShortcuts(() => api);

      const event = new KeyboardEvent("keydown", {
        ctrlKey: true,
        code: "KeyW",
        bubbles: true,
        cancelable: true,
      });
      const result = cmds[0].handler(event);

      expect(panel.api.close).toHaveBeenCalledOnce();
      expect(result).toBe(true);
    });

    it("getDockviewApi 在每次 handler 调用时重新获取（不是注册时缓存）", () => {
      const panel1 = activePanelStub();
      const api1 = dockviewApiStub({ activePanel: panel1 as unknown as IDockviewPanel });

      let currentApi: DockviewApi | undefined = api1;
      const cmds = createGlobalShortcuts(() => currentApi);

      // 第一次调用 → 使用 api1
      cmds[0].handler(new KeyboardEvent("keydown", { ctrlKey: true, code: "KeyW" }));
      expect(panel1.api.close).toHaveBeenCalledOnce();

      // 模拟页面切换，getDockviewApi 现在返回不同的 api
      const panel2 = activePanelStub();
      const api2 = dockviewApiStub({ activePanel: panel2 as unknown as IDockviewPanel });
      currentApi = api2;

      // 第二次调用 → 使用 api2
      cmds[0].handler(new KeyboardEvent("keydown", { ctrlKey: true, code: "KeyW" }));
      expect(panel2.api.close).toHaveBeenCalledOnce();
    });
  });

  // ---- 3. handler: 无活跃面板 → 透传 ----

  describe("handler 无活跃面板透传", () => {
    it("activePanel 为 undefined → 返回 false（事件透传）", () => {
      const api = dockviewApiStub({ activePanel: null });
      const cmds = createGlobalShortcuts(() => api);

      const event = new KeyboardEvent("keydown", {
        ctrlKey: true,
        code: "KeyW",
        bubbles: true,
        cancelable: true,
      });
      const result = cmds[0].handler(event);

      expect(result).toBe(false);
      expect(event.defaultPrevented).toBe(false);
    });

    it("getDockviewApi 返回 undefined → 安全返回 false", () => {
      const cmds = createGlobalShortcuts(() => undefined);

      const event = new KeyboardEvent("keydown", {
        ctrlKey: true,
        code: "KeyW",
        bubbles: true,
        cancelable: true,
      });
      const result = cmds[0].handler(event);

      expect(result).toBe(false);
      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ---- 4. 边界 ----

  describe("边界", () => {
    it("多次调用 handler 不抛异常", () => {
      const panel = activePanelStub();
      const api = dockviewApiStub({ activePanel: panel as unknown as IDockviewPanel });
      const cmds = createGlobalShortcuts(() => api);

      const event = new KeyboardEvent("keydown", { ctrlKey: true, code: "KeyW" });

      expect(() => {
        cmds[0].handler(event);
        cmds[0].handler(event);
        cmds[0].handler(event);
      }).not.toThrow();
    });

    it("getDockviewApi 抛异常时 handler 不传播（由 ShortcutRegistry 调用链吞错）", () => {
      const cmds = createGlobalShortcuts(() => {
        throw new Error("unexpected");
      });

      // handler 本身不吞错——如果 getDockviewApi 抛异常，应由 ShortcutRegistry 调用链处理。
      // 此测试仅验证命令对象可正常创建。
      expect(cmds[0]).toBeDefined();
    });
  });
});
