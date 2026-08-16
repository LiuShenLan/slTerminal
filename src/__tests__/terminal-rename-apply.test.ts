// terminal-rename-apply.test.ts — applyRename 纯函数直测
//
// applyRename（PageDockviewHost.tsx 导出）：重命名动作 = updateParameters 写
// customTitle + setTitle + 显式 onLayoutChange(saveLayout(api)) 触发持久化
// （setTitle/updateParameters 均不触发 onDidLayoutChange，故须显式保存）。
/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, vi } from "vitest";
import { applyRename } from "../workspace/PageDockviewHost";

describe("applyRename", () => {
  function makePanel(params?: Record<string, unknown> | undefined, title = "terminal-0") {
    return {
      id: "terminal-p1-0",
      title,
      params,
      api: { setTitle: vi.fn(), updateParameters: vi.fn() },
    };
  }

  it("updateParameters 展开保留原键并写入 customTitle", () => {
    const panel = makePanel({ panelId: "terminal-p1-0", cwd: "D:/repo", tabStatus: "attention" });
    applyRename({ toJSON: () => ({ mock: "layout" }) } as any, panel as any, "我的终端", vi.fn());
    expect(panel.api.updateParameters).toHaveBeenCalledWith({
      panelId: "terminal-p1-0",
      cwd: "D:/repo",
      tabStatus: "attention",
      customTitle: "我的终端",
    });
  });

  it("panel.params 为 undefined 时不崩（?? {} 分支）", () => {
    const panel = makePanel(undefined);
    expect(() =>
      applyRename({ toJSON: () => ({}) } as any, panel as any, "x", vi.fn()),
    ).not.toThrow();
    expect(panel.api.updateParameters).toHaveBeenCalledWith({ customTitle: "x" });
  });

  it("setTitle(newTitle) 调用", () => {
    const panel = makePanel();
    applyRename({ toJSON: () => ({}) } as any, panel as any, "改名", vi.fn());
    expect(panel.api.setTitle).toHaveBeenCalledWith("改名");
  });

  it("onLayoutChange 收到 saveLayout(api) 结果（toJSON 值）", () => {
    const api = { toJSON: () => ({ panels: { p1: { title: "t" } } }) };
    const onLayoutChange = vi.fn();
    applyRename(api as any, makePanel() as any, "新名", onLayoutChange);
    expect(onLayoutChange).toHaveBeenCalledTimes(1);
    expect(onLayoutChange).toHaveBeenCalledWith({ panels: { p1: { title: "t" } } });
  });

  it("原 params 对象不被修改（展开复制语义）", () => {
    const originalParams = { panelId: "terminal-p1-0" };
    const panel = makePanel(originalParams);
    applyRename({ toJSON: () => ({}) } as any, panel as any, "新名", vi.fn());
    expect(originalParams).toEqual({ panelId: "terminal-p1-0" });
    expect(originalParams).not.toHaveProperty("customTitle");
  });
});
