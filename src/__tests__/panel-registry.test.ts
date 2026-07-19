import { describe, it, expect } from "vitest";
import {
  panelRegistry,
  terminalTabConfig,
  PANEL_TYPES,
  PANEL_HTML_VIEWER,
  FILE_PANEL_TYPES,
  isValidPanelType,
} from "../panelRegistry";

describe("panelRegistry", () => {
  // 1. panelRegistry 包含 terminal、editor、htmlviewer、gitshow 四个键
  it("包含 terminal、editor、htmlviewer、gitshow 四个键", () => {
    const keys = Object.keys(panelRegistry);
    expect(keys).toHaveLength(4);
    expect(keys).toContain("terminal");
    expect(keys).toContain("editor");
    expect(keys).toContain("htmlviewer");
    expect(keys).toContain("gitshow");
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
  // 6. PANEL_TYPES 包含 ["terminal", "editor", "htmlviewer", "gitshow", "diff"]
  it('包含 ["terminal", "editor", "htmlviewer", "gitshow", "diff"]', () => {
    expect(PANEL_TYPES).toEqual(["terminal", "editor", "htmlviewer", "gitshow", "diff"]);
  });

  it("长度为 5", () => {
    expect(PANEL_TYPES).toHaveLength(5);
  });

  it("as const 只读，元素类型为字面量", () => {
    // TypeScript 编译期保证，运行时验证值一致
    expect(PANEL_TYPES[0]).toBe("terminal");
    expect(PANEL_TYPES[1]).toBe("editor");
    expect(PANEL_TYPES[2]).toBe("htmlviewer");
    expect(PANEL_TYPES[3]).toBe("gitshow");
    expect(PANEL_TYPES[4]).toBe("diff");
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

describe("FILE_PANEL_TYPES", () => {
  it('包含 "editor"', () => {
    expect(FILE_PANEL_TYPES.has("editor")).toBe(true);
  });

  it('包含 "htmlviewer"', () => {
    expect(FILE_PANEL_TYPES.has("htmlviewer")).toBe(true);
  });

  it('包含 "gitshow"', () => {
    expect(FILE_PANEL_TYPES.has("gitshow")).toBe(true);
  });

  it('包含 "diff"', () => {
    expect(FILE_PANEL_TYPES.has("diff")).toBe(true);
  });

  it('不包含 "terminal"', () => {
    expect(FILE_PANEL_TYPES.has("terminal")).toBe(false);
  });

  it("size 为 4", () => {
    expect(FILE_PANEL_TYPES.size).toBe(4);
  });
});

describe("PANEL_HTML_VIEWER", () => {
  it('值为 "htmlviewer"', () => {
    expect(PANEL_HTML_VIEWER).toBe("htmlviewer");
  });
});
