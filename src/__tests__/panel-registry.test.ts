import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import React from "react";
import {
  panelRegistry,
  terminalTabConfig,
  PANEL_TYPES,
  PANEL_HTML_VIEWER,
  isValidPanelType,
  withPanelBoundary,
} from "../panelRegistry";

describe("panelRegistry", () => {
  // 1. panelRegistry 包含 terminal、editor、htmlviewer、gitshow、diff、hooksConfig 六个键
  it("包含 terminal、editor、htmlviewer、gitshow、diff、hooksConfig 六个键", () => {
    const keys = Object.keys(panelRegistry);
    expect(keys).toHaveLength(6);
    expect(keys).toContain("terminal");
    expect(keys).toContain("editor");
    expect(keys).toContain("htmlviewer");
    expect(keys).toContain("gitshow");
    expect(keys).toContain("diff");
    expect(keys).toContain("hooksConfig");
  });

  // 2. terminal 注册项的 component 是 TerminalPanel
  it("terminal 注册项为函数组件", () => {
    const entry = panelRegistry.terminal;
    expect(typeof entry).toBe("function");
  });

  // 3. editor 注册项的 component 是 EditorPanel
  it("editor 注册项为函数组件", () => {
    const entry = panelRegistry.editor;
    expect(typeof entry).toBe("function");
  });

  // 新增：htmlviewer 注册项为函数组件
  it("htmlviewer 注册项为函数组件", () => {
    const entry = panelRegistry.htmlviewer;
    expect(typeof entry).toBe("function");
  });

  // 新增：gitshow 注册项为函数组件
  it("gitshow 注册项为函数组件", () => {
    const entry = panelRegistry.gitshow;
    expect(typeof entry).toBe("function");
  });

  // 新增：diff 注册项为函数组件
  it("diff 注册项为函数组件", () => {
    const entry = panelRegistry.diff;
    expect(typeof entry).toBe("function");
  });

  // 新增：hooksConfig 注册项为函数组件
  it("hooksConfig 注册项为函数组件", () => {
    const entry = panelRegistry.hooksConfig;
    expect(typeof entry).toBe("function");
  });
});

describe("terminalTabConfig", () => {
  // 4. terminal 的 tabComponent 使用 terminalTabConfig
  it("terminalTabConfig 存在", () => {
    expect(terminalTabConfig).toBeDefined();
  });

  // 5. terminalTabConfig.renderer === "always"（跨页面存活关键配置）
  it('renderer 为 "always"（跨页面存活关键配置）', () => {
    expect(terminalTabConfig.renderer).toBe("always");
  });

  it("terminalTabConfig 只有一个属性 renderer", () => {
    expect(Object.keys(terminalTabConfig)).toEqual(["renderer"]);
  });
});

describe("PANEL_TYPES", () => {
  // 6. PANEL_TYPES 包含 ["terminal", "editor", "htmlviewer", "gitshow", "diff", "hooksConfig"]
  it('包含 ["terminal", "editor", "htmlviewer", "gitshow", "diff", "hooksConfig"]', () => {
    expect(PANEL_TYPES).toEqual([
      "terminal",
      "editor",
      "htmlviewer",
      "gitshow",
      "diff",
      "hooksConfig",
    ]);
  });

  it("长度为 6", () => {
    expect(PANEL_TYPES).toHaveLength(6);
  });

  it("as const 只读，元素类型为字面量", () => {
    // TypeScript 编译期保证，运行时验证值一致
    expect(PANEL_TYPES[0]).toBe("terminal");
    expect(PANEL_TYPES[1]).toBe("editor");
    expect(PANEL_TYPES[2]).toBe("htmlviewer");
    expect(PANEL_TYPES[3]).toBe("gitshow");
    expect(PANEL_TYPES[4]).toBe("diff");
    expect(PANEL_TYPES[5]).toBe("hooksConfig");
  });
});

describe("isValidPanelType", () => {
  // 7. isValidPanelType("terminal") → true
  it('"terminal" 返回 true', () => {
    expect(isValidPanelType("terminal")).toBe(true);
  });

  // 8. isValidPanelType("editor") → true
  it('"editor" 返回 true', () => {
    expect(isValidPanelType("editor")).toBe(true);
  });

  // 新增：isValidPanelType("htmlviewer") → true
  it('"htmlviewer" 返回 true', () => {
    expect(isValidPanelType("htmlviewer")).toBe(true);
  });

  // 新增：isValidPanelType("gitshow") → true
  it('"gitshow" 返回 true', () => {
    expect(isValidPanelType("gitshow")).toBe(true);
  });

  // 新增：isValidPanelType("diff") → true
  it('"diff" 返回 true', () => {
    expect(isValidPanelType("diff")).toBe(true);
  });

  // 新增：isValidPanelType("hooksConfig") → true
  it('"hooksConfig" 返回 true', () => {
    expect(isValidPanelType("hooksConfig")).toBe(true);
  });

  // 9. isValidPanelType("unknown") → false
  it('"unknown" 返回 false', () => {
    expect(isValidPanelType("unknown")).toBe(false);
  });

  // 10. isValidPanelType("") → false
  it('空字符串返回 false', () => {
    expect(isValidPanelType("")).toBe(false);
  });

  // 11. isValidPanelType 对大小写敏感（"Terminal" → false）
  it('大小写敏感，"Terminal" 返回 false', () => {
    expect(isValidPanelType("Terminal")).toBe(false);
  });

  it('大小写敏感，"EDITOR" 返回 false', () => {
    expect(isValidPanelType("EDITOR")).toBe(false);
  });

  it("通过 type predicate 后 TypeScript 将类型收窄为 PanelType", () => {
    // 运行时验证：合法类型入参通过且类型收窄生效
    const val = "terminal";
    if (isValidPanelType(val)) {
      // val 在此作用域内类型收窄为 PanelType
      expect(PANEL_TYPES.includes(val)).toBe(true);
    } else {
      // 不应进入此分支
      expect.unreachable("合法类型应通过校验");
    }
  });
});

describe("PANEL_HTML_VIEWER", () => {
  it('值为 "htmlviewer"', () => {
    expect(PANEL_HTML_VIEWER).toBe("htmlviewer");
  });
});

// ─── FE-22: 面板级错误边界（withPanelBoundary）───

interface TestPanelProps {
  params: { panelId: string };
}

/** 会抛错的子面板组件，用于验证边界隔离 */
const ThrowingPanel: React.FC<TestPanelProps> = () => {
  throw new Error("FE-22 模拟面板渲染错误");
};

/** 正常子面板组件（同页兄弟面板） */
const NormalPanel: React.FC<TestPanelProps> = (props) =>
  React.createElement(
    "div",
    { "data-testid": `normal-${props.params.panelId}` },
    `正常面板 ${props.params.panelId}`,
  );

describe("FE-22 面板级错误边界（withPanelBoundary）", () => {
  beforeEach(() => {
    // 渲染错误边界会经 componentDidCatch 打 console.error——静默避免噪音
    vi.spyOn(console, "error").mockImplementation(() => {});
    delete (window as unknown as Record<string, unknown>).__sltermError;
  });

  it("1. 抛错面板降级为 inline 占位，同页其他面板存活", () => {
    const WrappedThrow = withPanelBoundary(ThrowingPanel);
    const WrappedNormal = withPanelBoundary(NormalPanel);
    const { container } = render(
      React.createElement(
        React.Fragment,
        null,
        React.createElement(WrappedThrow, { params: { panelId: "bad" } }),
        React.createElement(WrappedNormal, { params: { panelId: "good" } }),
      ),
    );
    // 抛错面板显示 inline 降级占位（错误被边界吞掉，不扩大为整页崩溃）
    expect(container.textContent).toContain("页面渲染出错");
    // 同页兄弟面板不受影响——内容仍在
    expect(container.textContent).toContain("正常面板 good");
    expect(container.querySelector('[data-testid="normal-good"]')).not.toBeNull();
  });

  it("2. 无错误时正常透传 children", () => {
    const WrappedNormal = withPanelBoundary(NormalPanel);
    const { container } = render(
      React.createElement(WrappedNormal, { params: { panelId: "a" } }),
    );
    expect(container.textContent).toContain("正常面板 a");
    expect(container.textContent).not.toContain("页面渲染出错");
  });

  it("3. 抛错后 window.__sltermError 被记录（componentDidCatch 副作用）", () => {
    const WrappedThrow = withPanelBoundary(ThrowingPanel);
    render(React.createElement(WrappedThrow, { params: { panelId: "bad" } }));
    expect((window as unknown as Record<string, unknown>).__sltermError).toBeDefined();
  });

  it("4. panelRegistry 六个注册项均经 withPanelBoundary 包裹（displayName 前缀 Boundary）", () => {
    for (const [type, Comp] of Object.entries(panelRegistry)) {
      expect((Comp as { displayName?: string }).displayName, `类型 ${type}`).toMatch(
        /^Boundary\(/,
      );
    }
  });
});
