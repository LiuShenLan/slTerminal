// workspace-filePanelTypes.test.ts — FILE_PANEL_TYPES 语义测试
//
// 验证文件型面板集合的成员关系：编辑器 + htmlviewer 参与标题计算，终端不参与。

import { describe, it, expect } from "vitest";
import { FILE_PANEL_TYPES } from "../workspace/panelRegistry";

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
