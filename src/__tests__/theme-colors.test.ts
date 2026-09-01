// colors.test.ts — theme/colors.ts 配色 token 单元测试
//
// colors.ts 为 facade：值代理 schemeRegistry.getActive()（默认 linear），
// 测试断言值即 linear 方案值（值契约锚点 = src/theme/schemes/linear.ts；
// 删除死配置与新增 ON_ACCENT_FG 见 C1 清单）。
//
// 覆盖路径：
//   1. GIT_FILE_COLORS 7 个 token 均为合法 hex
//   2. GIT_GUTTER_COLORS 3 个 token 均为合法 hex
//   3. EXPLORER_COLORS 5 个 token
//   4. 通用 UI 色 28 个独立 token（24 既有 + accentFg/selectionHoverBg/titlebarBg/titlebarCloseHover 4 新增）
//   5. ROOT_CSS_VARS 键集合 6 键（--sl-bg-primary / --sl-fg-primary + FE-08 收编 4 键）
//   6. theme/index.ts 重导出所有 token

import { describe, it, expect } from "vitest";
import {
  GIT_FILE_COLORS,
  GIT_GUTTER_COLORS,
  EXPLORER_COLORS,
  SIDEBAR_COLORS,
  PANEL_BG,
  SIDEBAR_BG,
  SECONDARY_BG,
  APP_BG,
  APP_BG_PRIMARY,
  EDITOR_BG,
  SIDEBAR_FG,
  ERROR_FG,
  PLACEHOLDER_FG,
  BUTTON_FG,
  DIM_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ACCENT_FG,
  ACTIVE_SELECTION_BG,
  EXPLORER_SELECTION_BG,
  SELECTION_HOVER_BG,
  SEPARATOR_BG,
  CONTEXT_MENU_BORDER,
  SHADOW_MENU,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
  ON_ACCENT_FG,
  TITLEBAR_BG,
  TITLEBAR_CLOSE_HOVER_BG,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
  ROOT_CSS_VARS,
  AGENT_STATUS_USAGE_COLORS,
} from "../theme";

describe("theme/colors.ts 配色 token", () => {
  describe("GIT_FILE_COLORS（文件名 git 状态色）", () => {
    it("包含 7 个 token", () => {
      expect(Object.keys(GIT_FILE_COLORS)).toHaveLength(7);
    });

    const cases = [
      { key: "modified", expected: "#d6b25e" },
      { key: "added", expected: "#86bb7a" },
      { key: "untracked", expected: "#6fbfc4" },
      { key: "deleted", expected: "#d9706b" },
      { key: "renamed", expected: "#6e9ff2" },
      { key: "conflict", expected: "#d9706b" },
      { key: "ignored", expected: "#6b675f" },
    ];

    it.each(cases)(
      "$key 值与预期一致 ($expected)",
      ({ key, expected }: { key: string; expected: string }) => {
        // 断言真实导出值（STS-01）：token 漂移/拼错即红，而非仅验证测试内字面量自身
        expect(GIT_FILE_COLORS[key as keyof typeof GIT_FILE_COLORS]).toBe(expected);
      },
    );
  });

  describe("GIT_GUTTER_COLORS（行内 diff 边栏色）", () => {
    it("包含 3 个 token", () => {
      expect(Object.keys(GIT_GUTTER_COLORS)).toHaveLength(3);
    });

    const gutterCases = [
      { key: "modified", expected: "#d6b25e" },
      { key: "added", expected: "#86bb7a" },
      { key: "deleted", expected: "#d9706b" },
    ];

    it.each(gutterCases)(
      "$key 值与预期一致 ($expected)",
      ({ key, expected }: { key: string; expected: string }) => {
        expect(GIT_GUTTER_COLORS[key as keyof typeof GIT_GUTTER_COLORS]).toBe(expected);
      },
    );
  });

  describe("EXPLORER_COLORS（文件浏览器通用色）", () => {
    it("包含 5 个 token", () => {
      expect(Object.keys(EXPLORER_COLORS)).toHaveLength(5);
    });

    const explorerCases = [
      { key: "bg", expected: "#101012" },
      { key: "fg", expected: "#b3aea6" },
      { key: "hover", expected: "#222227" },
      { key: "arrowClosed", expected: "#8a857d" },
      { key: "arrowOpen", expected: "#8a857d" },
    ];

    it.each(explorerCases)(
      "$key 值与预期一致 ($expected)",
      ({ key, expected }: { key: string; expected: string }) => {
        expect(EXPLORER_COLORS[key as keyof typeof EXPLORER_COLORS]).toBe(expected);
      },
    );
  });

  describe("SIDEBAR_COLORS（侧栏树专用色，FE-24 提取）", () => {
    it("包含 8 个 token（含 treeGuide 树形引导线——问题 4 修复新增）", () => {
      expect(Object.keys(SIDEBAR_COLORS)).toHaveLength(8);
    });

    const sidebarCases = [
      { key: "bg", expected: "#101012" },
      { key: "fg", expected: "#b3aea6" },
      { key: "hover", expected: "#222227" },
      { key: "selected", expected: "rgba(110,159,242,0.13)" },
      { key: "border", expected: "rgba(255,255,255,0.055)" },
      { key: "contextMenuBorder", expected: "rgba(255,255,255,0.09)" },
    ];

    it.each(sidebarCases)(
      "$key 值与预期一致 ($expected)",
      ({ key, expected }: { key: string; expected: string }) => {
        expect(SIDEBAR_COLORS[key as keyof typeof SIDEBAR_COLORS]).toBe(expected);
      },
    );

    it("contextMenuShadow 为合法阴影字符串", () => {
      expect(SIDEBAR_COLORS.contextMenuShadow).toMatch(
        /^0 8px 32px rgba\(0,0,0,0\.35\)$/,
      );
    });
  });

  describe("通用 UI 色（独立 token）", () => {
    const uiTokenCases = [
      // 背景色
      { name: "PANEL_BG", value: PANEL_BG, expected: "#0a0a0b" },
      { name: "SIDEBAR_BG", value: SIDEBAR_BG, expected: "#1a1a1e" },
      { name: "SECONDARY_BG", value: SECONDARY_BG, expected: "#222227" },
      { name: "APP_BG", value: APP_BG, expected: "#0a0a0b" },
      { name: "APP_BG_PRIMARY", value: APP_BG_PRIMARY, expected: "#0a0a0b" },
      { name: "EDITOR_BG", value: EDITOR_BG, expected: "#0a0a0b" },
      // 前景/文字色
      { name: "SIDEBAR_FG", value: SIDEBAR_FG, expected: "#ece9e4" },
      { name: "ERROR_FG", value: ERROR_FG, expected: "#d9706b" },
      { name: "PLACEHOLDER_FG", value: PLACEHOLDER_FG, expected: "#6b675f" },
      { name: "BUTTON_FG", value: BUTTON_FG, expected: "#ece9e4" },
      { name: "DIM_FG", value: DIM_FG, expected: "#8a857d" },
      // 交互控件色
      { name: "INPUT_BG", value: INPUT_BG, expected: "#1a1a1e" },
      { name: "INPUT_BORDER", value: INPUT_BORDER, expected: "rgba(255,255,255,0.09)" },
      { name: "FOCUS_BORDER", value: FOCUS_BORDER, expected: "#6e9ff2" },
      { name: "ACCENT_FG", value: ACCENT_FG, expected: "#8fb4f5" },
      { name: "ACTIVE_SELECTION_BG", value: ACTIVE_SELECTION_BG, expected: "rgba(110,159,242,0.13)" },
      { name: "EXPLORER_SELECTION_BG", value: EXPLORER_SELECTION_BG, expected: "rgba(110,159,242,0.13)" },
      { name: "SELECTION_HOVER_BG", value: SELECTION_HOVER_BG, expected: "rgba(110,159,242,0.22)" },
      { name: "SEPARATOR_BG", value: SEPARATOR_BG, expected: "rgba(255,255,255,0.055)" },
      { name: "CONTEXT_MENU_BORDER", value: CONTEXT_MENU_BORDER, expected: "rgba(255,255,255,0.09)" },
      // HTML 面板色
      { name: "HTML_PANEL_LOADING_FG", value: HTML_PANEL_LOADING_FG, expected: "#8a857d" },
      { name: "HTML_PANEL_IFRAME_BG", value: HTML_PANEL_IFRAME_BG, expected: "#FFFFFF" },
      // 强调底色前景
      { name: "ON_ACCENT_FG", value: ON_ACCENT_FG, expected: "#0c1220" },
      // 标题栏
      { name: "TITLEBAR_BG", value: TITLEBAR_BG, expected: "#141416" },
      { name: "TITLEBAR_CLOSE_HOVER_BG", value: TITLEBAR_CLOSE_HOVER_BG, expected: "#c04747" },
      // 错误提示色
      { name: "ERROR_BANNER_BG", value: ERROR_BANNER_BG, expected: "rgba(217,112,107,0.12)" },
      { name: "ERROR_BANNER_BORDER", value: ERROR_BANNER_BORDER, expected: "#d9706b" },
      { name: "ERROR_BANNER_FG", value: ERROR_BANNER_FG, expected: "#ece9e4" },
    ];

    it("共 28 个 UI token", () => {
      expect(uiTokenCases).toHaveLength(28);
    });

    it.each(uiTokenCases)(
      "$name 为合法色值（hex 或 rgba）($expected)",
      ({ expected }: { expected: string }) => {
        // 附录 A 中 rgba 形态（inputBorder/selection 系列/separator 等）与 hex 并存，
        // 一律视为合法
        expect(expected).toMatch(/^(#[0-9A-Fa-f]{3,6}|rgba?\([\d\s.,%]+\))$/);
      },
    );

    it.each(uiTokenCases)(
      "$name 值与预期一致",
      ({ value, expected }: { value: string; expected: string }) => {
        expect(value).toBe(expected);
      },
    );
  });

  describe("SHADOW_MENU（上下文菜单阴影色，FE-24）", () => {
    it("值为 rgba(0,0,0,0.55)", () => {
      expect(SHADOW_MENU).toBe("rgba(0,0,0,0.55)");
    });

    it("为非空字符串", () => {
      expect(typeof SHADOW_MENU).toBe("string");
      expect(SHADOW_MENU.length).toBeGreaterThan(0);
    });
  });

  describe("AGENT_STATUS_USAGE_COLORS（Agent 用量条分段色，四档 ≥90/≥70/≥50）", () => {
    it("包含 low / medium / high / critical 四个 token", () => {
      const keys = Object.keys(AGENT_STATUS_USAGE_COLORS);
      expect(keys).toHaveLength(4);
      expect(keys).toContain("low");
      expect(keys).toContain("medium");
      expect(keys).toContain("high");
      expect(keys).toContain("critical");
    });

    const usageCases = [
      { key: "low", expected: "#86bb7a" },
      { key: "medium", expected: "#a9c686" },
      { key: "high", expected: "#d6b25e" },
      { key: "critical", expected: "#d9706b" },
    ];

    it.each(usageCases)(
      "$key 值与预期一致 ($expected)",
      ({ key, expected }: { key: string; expected: string }) => {
        expect(AGENT_STATUS_USAGE_COLORS[key as keyof typeof AGENT_STATUS_USAGE_COLORS]).toBe(expected);
      },
    );
  });

  describe("ROOT_CSS_VARS（CSS 变量桥接，FE-24 + FE-08 扩键）", () => {
    it("包含 6 键：--sl-bg-primary / --sl-fg-primary + 聚焦边框 + 滚动条三键", () => {
      const keys = Object.keys(ROOT_CSS_VARS);
      expect(keys).toContain("--sl-bg-primary");
      expect(keys).toContain("--sl-fg-primary");
      expect(keys).toContain("--sl-focus-border");
      expect(keys).toContain("--sl-scrollbar-slider");
      expect(keys).toContain("--sl-scrollbar-slider-hover");
      expect(keys).toContain("--sl-scrollbar-slider-active");
      expect(keys).toHaveLength(6);
    });

    it("--sl-bg-primary 引用 APP_BG_PRIMARY", () => {
      expect(ROOT_CSS_VARS["--sl-bg-primary"]).toBe(APP_BG_PRIMARY);
    });

    it("--sl-fg-primary 值为 #b3aea6", () => {
      expect(ROOT_CSS_VARS["--sl-fg-primary"]).toBe("#b3aea6");
    });

    it("--sl-focus-border 引用 FOCUS_BORDER（ui.focusBorder 同源）", () => {
      expect(ROOT_CSS_VARS["--sl-focus-border"]).toBe(FOCUS_BORDER);
      expect(ROOT_CSS_VARS["--sl-focus-border"]).toBe("#6e9ff2");
    });

    it("滚动条三键与 App.css 收编前硬编码逐字相同（rgba(255,255,255,0.10/0.20/0.28)）", () => {
      expect(ROOT_CSS_VARS["--sl-scrollbar-slider"]).toBe("rgba(255,255,255,0.10)");
      expect(ROOT_CSS_VARS["--sl-scrollbar-slider-hover"]).toBe("rgba(255,255,255,0.20)");
      expect(ROOT_CSS_VARS["--sl-scrollbar-slider-active"]).toBe("rgba(255,255,255,0.28)");
    });

    it("所有值均为非空字符串", () => {
      for (const value of Object.values(ROOT_CSS_VARS)) {
        expect(typeof value).toBe("string");
        expect(value.length).toBeGreaterThan(0);
      }
    });
  });
});
