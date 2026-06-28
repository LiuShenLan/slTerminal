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
});
