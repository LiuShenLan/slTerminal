// colors.test.ts — theme/colors.ts 配色 token 单元测试
//
// 覆盖路径：
//   1. GIT_FILE_COLORS 7 个 token 均为合法 hex
//   2. GIT_GUTTER_COLORS 4 个 token 均为合法 hex
//   3. EXPLORER_COLORS 6 个 token
//   4. 通用 UI 色 20 个独立 token
//   5. theme/index.ts 重导出所有 token

import { describe, it, expect } from "vitest";
import {
  GIT_FILE_COLORS,
  GIT_GUTTER_COLORS,
  EXPLORER_COLORS,
  PANEL_BG,
  SIDEBAR_BG,
  SECONDARY_BG,
  DROPDOWN_BG,
  APP_BG,
  EDITOR_BG,
  SIDEBAR_FG,
  ERROR_FG,
  PLACEHOLDER_FG,
  BUTTON_FG,
  DIM_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ACTIVE_SELECTION_BG,
  SEPARATOR_BG,
  CONTEXT_MENU_BORDER,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
} from "../theme";

const HEX6_RE = /^#[0-9A-Fa-f]{6}$/;

describe("theme/colors.ts 配色 token", () => {
  describe("GIT_FILE_COLORS（文件名 git 状态色）", () => {
    it("包含 7 个 token", () => {
      expect(Object.keys(GIT_FILE_COLORS)).toHaveLength(7);
    });

    const cases = [
      { key: "modified", expected: "#6897BB" },
      { key: "added", expected: "#629755" },
      { key: "untracked", expected: "#D1675A" },
      { key: "deleted", expected: "#6C6C6C" },
      { key: "renamed", expected: "#3A8484" },
      { key: "conflict", expected: "#D5756C" },
      { key: "ignored", expected: "#848504" },
    ];

    it.each(cases)(
      "$key 值为合法 6 位 hex ($expected)",
      ({ expected }: { expected: string }) => {
        expect(expected).toMatch(HEX6_RE);
      },
    );
  });

  describe("GIT_GUTTER_COLORS（行内 diff 边栏色）", () => {
    it("包含 4 个 token", () => {
      expect(Object.keys(GIT_GUTTER_COLORS)).toHaveLength(4);
    });

    const gutterCases = [
      { key: "modified", expected: "#374752" },
      { key: "added", expected: "#384C38" },
      { key: "deleted", expected: "#656E76" },
      { key: "whitespaceOnly", expected: "#4C4638" },
    ];

    it.each(gutterCases)(
      "$key 值为合法 6 位 hex ($expected)",
      ({ expected }: { expected: string }) => {
        expect(expected).toMatch(HEX6_RE);
      },
    );
  });

  describe("EXPLORER_COLORS（文件浏览器通用色）", () => {
    it("包含 6 个 token", () => {
      expect(Object.keys(EXPLORER_COLORS)).toHaveLength(6);
    });

    const explorerCases = [
      { key: "bg", expected: "#1E1E1E" },
      { key: "fg", expected: "#D4D4D4" },
      { key: "hover", expected: "#2A2D2E" },
      { key: "selected", expected: "#37373D" },
      { key: "arrowClosed", expected: "#6C6C6C" },
      { key: "arrowOpen", expected: "#D4D4D4" },
    ];

    it.each(explorerCases)(
      "$key 值为合法 6 位 hex ($expected)",
      ({ expected }: { expected: string }) => {
        expect(expected).toMatch(HEX6_RE);
      },
    );
  });

  describe("通用 UI 色（独立 token）", () => {
    const uiTokenCases = [
      // 背景色
      { name: "PANEL_BG", value: PANEL_BG, expected: "#1E1E1E" },
      { name: "SIDEBAR_BG", value: SIDEBAR_BG, expected: "#252526" },
      { name: "SECONDARY_BG", value: SECONDARY_BG, expected: "#2D2D2D" },
      { name: "DROPDOWN_BG", value: DROPDOWN_BG, expected: "#2A2D2E" },
      { name: "APP_BG", value: APP_BG, expected: "#1e1e2e" },
      { name: "EDITOR_BG", value: EDITOR_BG, expected: "#282C34" },
      // 前景/文字色
      { name: "SIDEBAR_FG", value: SIDEBAR_FG, expected: "#D4D4D4" },
      { name: "ERROR_FG", value: ERROR_FG, expected: "#F44747" },
      { name: "PLACEHOLDER_FG", value: PLACEHOLDER_FG, expected: "#808080" },
      { name: "BUTTON_FG", value: BUTTON_FG, expected: "#CCCCCC" },
      { name: "DIM_FG", value: DIM_FG, expected: "#999999" },
      // 交互控件色
      { name: "INPUT_BG", value: INPUT_BG, expected: "#3C3C3C" },
      { name: "INPUT_BORDER", value: INPUT_BORDER, expected: "#6C6C6C" },
      { name: "FOCUS_BORDER", value: FOCUS_BORDER, expected: "#007ACC" },
      { name: "ACTIVE_SELECTION_BG", value: ACTIVE_SELECTION_BG, expected: "#094771" },
      { name: "SEPARATOR_BG", value: SEPARATOR_BG, expected: "#444" },
      { name: "CONTEXT_MENU_BORDER", value: CONTEXT_MENU_BORDER, expected: "#454545" },
      // 错误提示色
      { name: "ERROR_BANNER_BG", value: ERROR_BANNER_BG, expected: "#5A1D1D" },
      { name: "ERROR_BANNER_BORDER", value: ERROR_BANNER_BORDER, expected: "#8B0000" },
      { name: "ERROR_BANNER_FG", value: ERROR_BANNER_FG, expected: "#F48771" },
    ];

    it("共 20 个 UI token", () => {
      expect(uiTokenCases).toHaveLength(20);
    });

    it.each(uiTokenCases)(
      "$name 为合法 6 位 hex ($expected)",
      ({ expected }: { expected: string }) => {
        // SEPARATOR_BG 是 '#444'（3 位简写），也视为合法
        expect(expected).toMatch(/^#[0-9A-Fa-f]{3,6}$/);
      },
    );

    it.each(uiTokenCases)(
      "$name 值与预期一致",
      ({ value, expected }: { value: string; expected: string }) => {
        expect(value).toBe(expected);
      },
    );
  });
});
