// repaintGuard —— WebView2/Chromium 陈旧光栅规避（GLYPH bug，2026-09-06 取证）
//
// 症状：CM6 编辑页中连续键入（多事务/跨帧）后，行内部分字形「存在但未绘制」
// ——DOM 文本/几何/字色完整（选中强制重绘即恢复），仅像素缺失。
// 取证（e2e-tests/glyph-repro.e2e.ts，GLYPH_E2E=1）：
//   - GPU 合成/光栅/全软件渲染关闭均复现（G1-G3）→ 非 GPU 环节
//   - CSS 层提升（translateZ/will-change）与 containment 无效（E6 系列）
//   - @codemirror/view 最新版（6.43.11，含 tile tree 修复）仍复现 → 非 CM6 逻辑
//   - 单事务批量输入不丢、跨帧多事务必丢 → Chromium 对 contenteditable 增量
//     文本更新的 paint 缓存陈旧缺陷（引擎缺陷，无上游修复）
// 规避机制：每次 docChanged 后对受影响行做 display:none → 强制同步 reflow →
// 还原（同宏任务完成）。行布局对象重建 → 该行 paint 缓存整行失效 → 每次帧
// 渲染必为全新绘制。不触碰 text node / selection / 装饰 span 结构；文本内容
// 未变 CM6 状态机不感知；同宏任务内还原无视觉闪烁。
// 覆盖宿主：全部 CM6 编辑入口（useCodeMirror / JsonMode / DiffPanel / gitshow）。
// 撤销条件：WebView2/Chromium 引擎修复此缺陷后升级复核移除本 guard。

import { ViewPlugin, type ViewUpdate } from "@codemirror/view";
import type { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

/** 单行重绘原语（导出供单测）：display none → 强制 reflow → 还原（同宏任务） */
export function refreshLineByToggle(lineEl: HTMLElement): void {
  lineEl.style.display = "none";
  void lineEl.offsetHeight; // 强制同步 reflow——display none 立即生效
  lineEl.style.display = "";
}

/** doc 变更后刷新「受影响行」的 paint 缓存（受影响行 = 光标行——键入场景
 * 光标行即编辑行；多行变更（粘贴/reload）为单事务渲染不触发陈旧，无需覆盖） */
class RepaintGuardView {
  constructor(private readonly view: EditorView) {}

  update(update: ViewUpdate): void {
    if (!update.docChanged) return;
    const line = update.state.doc.lineAt(update.state.selection.main.head);
    const dom = this.view.domAtPos(line.from);
    // domAtPos 返回文本节点或元素；closest 回溯到行容器
    const host =
      dom.node instanceof HTMLElement ? dom.node : dom.node.parentElement;
    const lineEl = host?.closest(".cm-line");
    if (lineEl instanceof HTMLElement) {
      refreshLineByToggle(lineEl);
    }
  }
}

const repaintGuardPlugin = ViewPlugin.fromClass(RepaintGuardView);

/** CM6 扩展：doc 变更后刷新受影响行 paint 缓存（全部 CM6 编辑宿主挂载） */
export function repaintGuard(): Extension {
  return repaintGuardPlugin;
}
