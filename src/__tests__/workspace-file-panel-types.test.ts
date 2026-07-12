// workspace-filePanelTypes.test.ts — FILE_PANEL_TYPES + isAlwaysRenderPanel 语义测试
//
// 验证文件型面板集合的成员关系 + renderer 模式判断。

import { describe, it, expect } from "vitest";
import { FILE_PANEL_TYPES, isAlwaysRenderPanel } from "../workspace/panelRegistry";

describe("FILE_PANEL_TYPES", () => {
  // 55. FILE_PANEL_TYPES 为 Set 类型
  it("为 Set 类型", () => {
    expect(FILE_PANEL_TYPES).toBeInstanceOf(Set);
  });

  // 56. editor 参与标题计算
  it("editor 在集合中", () => {
    expect(FILE_PANEL_TYPES.has("editor")).toBe(true);
  });

  // 57. htmlviewer 参与标题计算
  it("htmlviewer 在集合中", () => {
    expect(FILE_PANEL_TYPES.has("htmlviewer")).toBe(true);
  });

  // 58. terminal 不参与文件标题计算
  it("terminal 不在集合中", () => {
    expect(FILE_PANEL_TYPES.has("terminal")).toBe(false);
  });

  // 补充：size 正确
  it("集合大小为 2", () => {
    expect(FILE_PANEL_TYPES.size).toBe(2);
  });

  // 补充：未知面板类型不在集合中
  it("未知面板类型不在集合中", () => {
    expect(FILE_PANEL_TYPES.has("unknown")).toBe(false);
  });
});

// ============================================================================
// isAlwaysRenderPanel（白屏修复）
// ============================================================================

describe("isAlwaysRenderPanel", () => {
  // 68. htmlviewer 为 viewer 类型 → renderer="always"
  it("htmlviewer 返回 true", () => {
    expect(isAlwaysRenderPanel("htmlviewer")).toBe(true);
  });

  // 69. editor 不是 viewer → 不强制 always
  it("editor 返回 false", () => {
    expect(isAlwaysRenderPanel("editor")).toBe(false);
  });

  // terminal 也需要 renderer="always"（PTY 存活）
  it("terminal 返回 true", () => {
    expect(isAlwaysRenderPanel("terminal")).toBe(true);
  });

  // 未知类型不在 PANEL_TYPES 中 → false
  it("未知类型返回 false", () => {
    expect(isAlwaysRenderPanel("unknown")).toBe(false);
  });

  // 空字符串 → false
  it("空字符串返回 false", () => {
    expect(isAlwaysRenderPanel("")).toBe(false);
  });
});
