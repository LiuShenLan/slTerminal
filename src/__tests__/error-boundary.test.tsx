// error-boundary.test.tsx — ErrorBoundary 类组件测试
//
// 验证 getDerivedStateFromError + componentDidCatch + render 错误态 + 正常透传。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import React from "react";
import { ErrorBoundary } from "../lib";

// mock ipc window
vi.mock("../ipc/window", () => ({
  registerCloseHandler: vi.fn(() => () => {}),
}));

// mock workspace
vi.mock("../workspace", () => ({
  Workspace: vi.fn(() => null),
}));

// mock stores
vi.mock("../stores/projects", () => ({
  loadAllProjects: vi.fn().mockResolvedValue(undefined),
  markPersistenceReady: vi.fn(),
  saveAllProjects: vi.fn().mockResolvedValue(undefined),
  cancelPendingSave: vi.fn(),
  useProjects: Object.assign(vi.fn(() => ({})), {
    getState: vi.fn(() => ({} as unknown)),
    setState: vi.fn(),
  }),
}));

vi.mock("../stores/layout", () => ({
  useLayout: Object.assign(vi.fn(() => ({ activePageId: null })), {
    getState: vi.fn(() => ({ activePageId: null, setActivePage: vi.fn() })),
    setState: vi.fn(),
  }),
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({})),
}));

/** 会抛错的子组件，用于测试错误边界 */
const ThrowError: React.FC = () => {
  throw new Error("模拟渲染错误");
};

describe("ErrorBoundary", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    delete (window as unknown as Record<string, unknown>).__sltermError;
  });

  it("1. 正常 children 透传（无错误时不拦截）", () => {
    const { container } = render(
      <ErrorBoundary>
        <div>正常内容</div>
      </ErrorBoundary>,
    );
    expect(container.textContent).toContain("正常内容");
    expect(container.textContent).not.toContain("应用渲染错误");
  });

  it("2. 子组件抛错 → 渲染 <h2>应用渲染错误</h2> + message + stack", () => {
    const { container } = render(
      <ErrorBoundary>
        <ThrowError />
      </ErrorBoundary>,
    );
    expect(container.textContent).toContain("应用渲染错误");
    expect(container.textContent).toContain("模拟渲染错误");
    // UI-201：fullscreen variant 根容器为全局字体栈（不得回退裸 monospace）
    expect((container.firstElementChild as HTMLElement).style.fontFamily).toBe(
      '"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace',
    );
    // 两个 <pre>：message 和 stack
    const pres = container.querySelectorAll("pre");
    expect(pres.length).toBe(2);
    expect(pres[0].textContent).toContain("模拟渲染错误");
    expect(pres[1].textContent).toContain("Error: 模拟渲染错误");
  });

  it("3. 子组件抛错 → window.__sltermError 被赋值，console.error 被调用", () => {
    render(
      <ErrorBoundary>
        <ThrowError />
      </ErrorBoundary>,
    );
    expect((window as unknown as Record<string, unknown>).__sltermError).toBeDefined();
    expect(console.error).toHaveBeenCalled();
  });

  it("4. variant=inline 子组件抛错 → 页面级占位 UI（不渲染全屏标题）", () => {
    const { container } = render(
      <ErrorBoundary variant="inline">
        <ThrowError />
      </ErrorBoundary>,
    );
    // inline 模式标题 + 说明文案（对应 ErrorBoundary.tsx inline 分支）
    expect(container.textContent).toContain("页面渲染出错");
    expect(container.textContent).toContain("该操作页面因渲染错误无法显示，其他页面不受影响。");
    // 不渲染 fullscreen 分支标题
    expect(container.textContent).not.toContain("应用渲染错误");
    // UI-201：inline variant 根容器为全局字体栈（不得回退裸 monospace）
    expect((container.firstElementChild as HTMLElement).style.fontFamily).toBe(
      '"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace',
    );
    // componentDidCatch 不受 variant 影响——window.__sltermError 仍被赋值
    expect((window as unknown as Record<string, unknown>).__sltermError).toBeDefined();
    // 错误详情 <pre> 含 message + stack（details 折叠不影响 DOM 存在）
    const pres = container.querySelectorAll("pre");
    expect(pres.length).toBe(1);
    expect(pres[0].textContent).toContain("模拟渲染错误");
    expect(pres[0].textContent).toContain("Error: 模拟渲染错误");
  });

  it("5. variant=inline 无错误时正常透传 children", () => {
    const { container } = render(
      <ErrorBoundary variant="inline">
        <div>内联正常内容</div>
      </ErrorBoundary>,
    );
    expect(container.textContent).toContain("内联正常内容");
    expect(container.textContent).not.toContain("页面渲染出错");
  });
});
