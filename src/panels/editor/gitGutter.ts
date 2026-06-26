// gitGutter.ts — CodeMirror 6 行内 diff 边栏扩展
//
// 职责：
// - 定义 AddedMarker / ModifiedMarker / DeletedMarker 三种 GutterMarker
// - 定义 setDiffMarkers StateEffect 用于从外部传入 DiffHunk[]
// - 定义 diffMarkersField StateField 存储 RangeSet<GutterMarker>
// - 导出 diffGutter() 扩展函数和 updateDiffGutter() 更新函数
// - 从 DiffHunk[]（Rust git_diff 返回）映射到行号标记
//
// 硬约束 #6: 配色从 theme/colors.ts 的 GIT_GUTTER_COLORS 引用，不硬编码

import { EditorView, GutterMarker, gutter } from "@codemirror/view";
import { EditorState, StateField, StateEffect, RangeSet, RangeSetBuilder } from "@codemirror/state";
import type { DiffHunk } from "../../types/git";
import { GIT_GUTTER_COLORS } from "../../theme/colors";

// ── GutterMarker 子类 ────────────────────────────────────────
// 使用 toDOM() 而非 elementClass，避免维护外部 CSS，
// 配色直接从 GIT_GUTTER_COLORS token 引用。

/** 新增行标记 — 绿色竖条 */
class AddedMarker extends GutterMarker {
  toDOM(): Node {
    const el = document.createElement("div");
    el.style.cssText = `background-color:${GIT_GUTTER_COLORS.added};width:3px;height:100%;margin-left:6px;`;
    return el;
  }
}

/** 修改行标记 — 蓝色竖条 */
class ModifiedMarker extends GutterMarker {
  toDOM(): Node {
    const el = document.createElement("div");
    el.style.cssText = `background-color:${GIT_GUTTER_COLORS.modified};width:3px;height:100%;margin-left:6px;`;
    return el;
  }
}

/** 删除行标记 — 灰色三角（因工作区无此行，显示在相邻行） */
class DeletedMarker extends GutterMarker {
  toDOM(): Node {
    const el = document.createElement("div");
    // 用 CSS border 画朝上的三角形，表示该行上方有删除
    el.style.cssText =
      `width:0;height:0;border-left:4px solid transparent;` +
      `border-right:4px solid transparent;` +
      `border-bottom:6px solid ${GIT_GUTTER_COLORS.deleted};` +
      `margin-left:4px;margin-top:4px;`;
    return el;
  }
}

// ── StateEffect ──────────────────────────────────────────────

/** 从外部设置 diff 标记。Payload 为 Rust git_diff 返回的 DiffHunk[] */
export const setDiffMarkers = StateEffect.define<DiffHunk[]>();

// ── DiffHunk → RangeSet<GutterMarker> 构建逻辑 ───────────────

/**
 * 将 unified diff hunk 数组转换为 RangeSet<GutterMarker>。
 *
 * 映射算法（DiffHunk 行号均为 1-based，与 CM6 doc.line(n) 一致）：
 * - oldLines=0 → 纯新增，newStart..newStart+newLines-1 全部标 AddedMarker
 * - newLines=0 → 纯删除，在 newStart 位置标 DeletedMarker（三角指示符）
 * - 两者均>0 →
 *     - 前 min(oldLines,newLines) 行标 ModifiedMarker
 *     - newLines>oldLines 时，超出部分标 AddedMarker
 *     - oldLines>newLines 时，在 newStart+newLines 处标 DeletedMarker
 */
function buildRangeSet(state: EditorState, hunks: DiffHunk[]): RangeSet<GutterMarker> {
  const builder = new RangeSetBuilder<GutterMarker>();
  const doc = state.doc;

  // 预取所有行信息以减少 doc.line() 调用
  const lineCache = new Map<number, { from: number }>();
  const getLineFrom = (lineNo: number): number => {
    if (lineNo <= 0 || lineNo > doc.lines) return -1;
    let cached = lineCache.get(lineNo);
    if (!cached) {
      cached = { from: doc.line(lineNo).from };
      lineCache.set(lineNo, cached);
    }
    return cached.from;
  };

  for (const hunk of hunks) {
    if (hunk.oldLines === 0) {
      // 纯新增：所有 new 行标为 added
      for (let i = 0; i < hunk.newLines; i++) {
        const pos = getLineFrom(hunk.newStart + i);
        if (pos >= 0) builder.add(pos, pos, new AddedMarker());
      }
    } else if (hunk.newLines === 0) {
      // 纯删除：在 newStart 位置标三角形
      const pos = getLineFrom(hunk.newStart);
      if (pos >= 0) builder.add(pos, pos, new DeletedMarker());
    } else {
      // 修改：min(oldLines,newLines) 行为 modified
      const shared = Math.min(hunk.oldLines, hunk.newLines);
      for (let i = 0; i < shared; i++) {
        const pos = getLineFrom(hunk.newStart + i);
        if (pos >= 0) builder.add(pos, pos, new ModifiedMarker());
      }

      // new 侧超出 → added
      if (hunk.newLines > hunk.oldLines) {
        for (let i = shared; i < hunk.newLines; i++) {
          const pos = getLineFrom(hunk.newStart + i);
          if (pos >= 0) builder.add(pos, pos, new AddedMarker());
        }
      }

      // old 侧超出 → 在最后修改行之后标 deleted
      if (hunk.oldLines > hunk.newLines) {
        const pos = getLineFrom(hunk.newStart + hunk.newLines);
        if (pos >= 0) builder.add(pos, pos, new DeletedMarker());
      }
    }
  }

  return builder.finish();
}

// ── StateField ───────────────────────────────────────────────

/** 存储 diff gutter 标记的 StateField。
 *
 * update 时先调 set.map(tr.changes) 让 CM6 自动将 marker 位置
 * 按文档变更同步。然后处理 setDiffMarkers effect 重建 RangeSet。
 * 注意：大量编辑后 diff 可能过时，调用方应在适当时机重新
 * gitDiff 并 dispatch setDiffMarkers。
 */
export const diffMarkersField = StateField.define<RangeSet<GutterMarker>>({
  create(): RangeSet<GutterMarker> {
    return RangeSet.empty;
  },

  update(set: RangeSet<GutterMarker>, tr): RangeSet<GutterMarker> {
    // 文档变更时自动映射 marker 位置
    set = set.map(tr.changes);

    for (const e of tr.effects) {
      if (e.is(setDiffMarkers)) {
        if (e.value.length === 0) {
          set = RangeSet.empty;
        } else {
          set = buildRangeSet(tr.state, e.value);
        }
      }
    }

    return set;
  },
});

// ── Gutter 扩展 ──────────────────────────────────────────────

/** 返回 diff gutter 扩展，挂到 EditorState 扩展列表中 */
export function diffGutter() {
  // 用 COMPUTED gutter 类型：挂载自己的 CSS 类 + 从 StateField 读 RangeSet
  return [
    diffMarkersField,
    gutter({
      class: "cm-diff-gutter",
      markers: (view): RangeSet<GutterMarker> => view.state.field(diffMarkersField),
    }),
  ];
}

// ── 更新函数 ─────────────────────────────────────────────────

/**
 * 更新编辑器的 diff 边栏标记。
 *
 * 调用方式：
 *   const hunks = await gitDiff(repoPath, filePath);
 *   updateDiffGutter(view, hunks);
 *
 * 通常在文件打开后和 Ctrl+S 保存后调用。
 */
export function updateDiffGutter(view: EditorView, hunks: DiffHunk[]): void {
  view.dispatch({
    effects: setDiffMarkers.of(hunks),
  });
}

/**
 * 清除 diff 边栏标记（文件未关联 git、git 不可用等场景）。
 */
export function clearDiffGutter(view: EditorView): void {
  view.dispatch({
    effects: setDiffMarkers.of([]),
  });
}
