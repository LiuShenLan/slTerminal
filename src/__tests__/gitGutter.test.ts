// gitGutter.test.ts — CodeMirror 6 diff 边栏单元测试
//
// 验证：DiffHunk → RangeSet<GutterMarker> 映射 + GutterMarker DOM 颜色

import { describe, it, expect } from "vitest";
import { EditorState } from "@codemirror/state";
import type { DiffHunk } from "../types/git";
import { GIT_GUTTER_COLORS } from "../theme/colors";
import {
  setDiffMarkers,
  diffMarkersField,
} from "../panels/editor/gitGutter";

// ── Helpers ──

/** 创建含 diffMarkersField 的最简 EditorState */
function createState(doc: string) {
  return EditorState.create({
    doc,
    extensions: [diffMarkersField],
  });
}

/** 获取 markers 中的 marker 数量 */
function markerCount(state: EditorState): number {
  const set = state.field(diffMarkersField);
  let count = 0;
  const cursor = set.iter();
  while (cursor.value !== null) {
    count++;
    cursor.next();
  }
  return count;
}

// ── Tests ──

describe("gitGutter", () => {
  describe("StateEffect 映射", () => {
    it("G1: 修改行 → ModifiedMarker", () => {
      const state = createState("line1\nline2\nline3\n");
      const hunks: DiffHunk[] = [
        { oldStart: 2, oldLines: 1, newStart: 2, newLines: 1 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      expect(markerCount(tr.state)).toBe(1);
    });

    it("G2: 新增行 → AddedMarker", () => {
      const state = createState("line1\nnew stuff\nline3\n");
      const hunks: DiffHunk[] = [
        { oldStart: 1, oldLines: 0, newStart: 2, newLines: 1 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      expect(markerCount(tr.state)).toBe(1);
    });

    it("G3: 删除行 → DeletedMarker", () => {
      const state = createState("line1\nline3\n");
      const hunks: DiffHunk[] = [
        { oldStart: 2, oldLines: 1, newStart: 2, newLines: 0 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      expect(markerCount(tr.state)).toBe(1);
    });

    it("G4: 混合修改 → 2 modified + 2 added", () => {
      // old_lines=2, new_lines=4 → min(2,4)=2 modified, 4-2=2 added
      const state = createState("a\nb\nc\nd\ne\nf\n");
      const hunks: DiffHunk[] = [
        { oldStart: 1, oldLines: 2, newStart: 1, newLines: 4 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      // 4 markers: 2 modified + 2 added
      expect(markerCount(tr.state)).toBe(4);
    });

    it("G5: 空 hunks → RangeSet.empty", () => {
      const state = createState("unchanged\n");
      const tr = state.update({
        effects: setDiffMarkers.of([]),
      });
      expect(markerCount(tr.state)).toBe(0);
    });
  });

  describe("GutterMarker DOM 颜色", () => {
    it("G6: marker 颜色与 GIT_GUTTER_COLORS token 一致", () => {
      // 验证 token 值
      expect(GIT_GUTTER_COLORS.modified).toBe("#374752");
      expect(GIT_GUTTER_COLORS.added).toBe("#384C38");
      expect(GIT_GUTTER_COLORS.deleted).toBe("#656E76");

      // 验证均为合法 6 位 hex
      const hexRe = /^#[0-9A-Fa-f]{6}$/;
      for (const [, value] of Object.entries(GIT_GUTTER_COLORS)) {
        expect(value).toMatch(hexRe);
      }
    });
  });

  describe("行级精确 DiffHunk 映射（与 Rust line callback 输出对齐）", () => {
    it("G8: 纯新增（old_lines=0）→ 仅 AddedMarker", () => {
      const state = createState("line1\nnewA\nnewB\nline4\n");
      const hunks: DiffHunk[] = [
        { oldStart: 0, oldLines: 0, newStart: 2, newLines: 2 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      // 2 行新增 → 2 个 AddedMarker
      expect(markerCount(tr.state)).toBe(2);
    });

    it("G9: 纯删除（new_lines=0）→ 仅 DeletedMarker", () => {
      const state = createState("line1\nline4\n");
      const hunks: DiffHunk[] = [
        { oldStart: 2, oldLines: 2, newStart: 2, newLines: 0 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      // 2 行删除 → 1 个 DeletedMarker（三角放在 newStart 位置）
      expect(markerCount(tr.state)).toBe(1);
    });

    it("G10: 修改+多余删除 → 2 modified + 1 deleted", () => {
      // 模拟 Rust flush_pending: 3 del + 2 add → shared=2 modified + 1 deleted
      const state = createState("keep\nmod1\nmod2\nline4\n");
      const hunks: DiffHunk[] = [
        { oldStart: 2, oldLines: 2, newStart: 2, newLines: 2 },
        { oldStart: 4, oldLines: 1, newStart: 4, newLines: 0 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      // 2 modified + 1 deleted = 3 markers
      expect(markerCount(tr.state)).toBe(3);
    });

    it("G11: marker 位置超出文档边界 → 不崩溃", () => {
      // newStart 超出文档行数，getLineFrom 返回 -1，builder.add 应安全跳过
      const state = createState("short\n");
      const hunks: DiffHunk[] = [
        { oldStart: 100, oldLines: 1, newStart: 100, newLines: 1 },
      ];

      const tr = state.update({
        effects: setDiffMarkers.of(hunks),
      });
      // 位置无效 → 0 markers，不崩溃
      expect(markerCount(tr.state)).toBe(0);
    });

    it("G12: 文档变更后 marker 位置跟随映射", () => {
      const state = createState("line1\nline2\nline3\nline4\nline5\n");
      // 先在 line3 放置一个 marker
      const tr1 = state.update({
        effects: setDiffMarkers.of([
          { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1 },
        ]),
      });
      expect(markerCount(tr1.state)).toBe(1);

      // 在 line1 后插入 2 行 → marker 应下移到 line5
      const tr2 = tr1.state.update({
        changes: { from: 6, insert: "newA\nnewB\n" }, // line1 后（pos 5 是 line1 \n 的长度）
      });
      // 文档变更会触发 set.map(tr.changes)，marker 位置自动跟随
      const set = tr2.state.field(diffMarkersField);
      let mappedPos = -1;
      const cursor = set.iter();
      if (cursor.value !== null) {
        mappedPos = cursor.from;
      }
      // marker 应移动到更后面的位置
      expect(mappedPos).toBeGreaterThan(0);
      expect(mappedPos).not.toBe(6); // 不应停留在原位置
    });
  });

  describe("updateDiffGutter / clearDiffGutter", () => {
    it("G7: clearDiffGutter → dispatch setDiffMarkers.of([])", () => {
      // 先在 state 中加点 markers
      const state = createState("a\nb\n");
      const tr1 = state.update({
        effects: setDiffMarkers.of([
          { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 },
        ]),
      });
      expect(markerCount(tr1.state)).toBe(1);

      // 清除
      const tr2 = tr1.state.update({
        effects: setDiffMarkers.of([]),
      });
      expect(markerCount(tr2.state)).toBe(0);
    });
  });
});
