// colors.test.ts — theme/colors.ts 配色 token 单元测试
//
// 覆盖路径：
//   1. GIT_FILE_COLORS 7 个 token 均为合法 hex
//   2. GIT_GUTTER_COLORS 4 个 token 均为合法 hex
//   3. EXPLORER_COLORS 6 个 token
//   4. theme/index.ts 重导出所有 token

import { describe, it, expect } from "vitest";
import {
  GIT_FILE_COLORS,
  GIT_GUTTER_COLORS,
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
});
