// repaint-guard.test.ts —— WebView2 陈旧光栅规避 guard 行为/契约测试
//
// 覆盖：
// 1. refreshLineByToggle 原语：display none → 强制 reflow → 还原（同宏任务语义）
// 2. repaintGuard() 返回合法 CM6 扩展（EditorState.create 可消费）
// 3. 集成 smoke：真实 EditorView（jsdom）挂载 + dispatch 文本插入 → 行 DOM 存在、
//    文档正确、无异常（toggle 在同步回调内执行，最终态 display 还原为空）
// 4. 注入面契约（反向/防复发——对照老代码断言全部 CM6 宿主保留 guard 注入，
//    防未来重构悄悄移除导致 bug 复现；theme-overrides.test 源码断言先例同风格）

import { describe, it, expect, vi } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { refreshLineByToggle, repaintGuard } from "../panels/editor/repaintGuard";

const SRC_ROOT = join(__dirname, "../..");

describe("refreshLineByToggle 原语", () => {
  it("display 置 none → 强制 reflow（offsetHeight 读取）→ 还原空串，同宏任务完成", () => {
    const div = document.createElement("div");
    // spy offsetHeight getter：断言读取发生在 display none 期间（强制 reflow 语义）
    const states: string[] = [];
    vi.spyOn(div, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
      states.push(this.style.display);
      return 0;
    });
    refreshLineByToggle(div);
    expect(states).toEqual(["none"]); // 读取时确在 display:none 态
    expect(div.style.display).toBe(""); // 读取后还原
  });

  it("非 HTMLElement 保护：原语对任意元素安全", () => {
    const el = document.createElement("span");
    expect(() => refreshLineByToggle(el)).not.toThrow();
    expect(el.style.display).toBe("");
  });
});

describe("repaintGuard 扩展合法性", () => {
  it("EditorState.create 可消费（不抛异常）", () => {
    expect(() => EditorState.create({ extensions: [repaintGuard()] })).not.toThrow();
  });

  it("重复消费（多宿主同时挂载）不冲突", () => {
    expect(() =>
      EditorState.create({ extensions: [repaintGuard(), repaintGuard()] }),
    ).not.toThrow();
  });
});

describe("repaintGuard 集成（真实 EditorView + jsdom）", () => {
  it("docChanged 后行 DOM 完整、display 还原、无异常", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const view = new EditorView({
      state: EditorState.create({
        doc: "glyphbase-line\n",
        extensions: [repaintGuard()],
      }),
      parent: container,
    });
    try {
      view.dispatch({ changes: { from: 0, insert: "|" } });
      const lineEl = container.querySelector(".cm-line");
      expect(lineEl).not.toBeNull();
      expect(lineEl?.textContent).toContain("|");
      // toggle 同步执行后还原——最终态无残留 display 样式
      expect((lineEl as HTMLElement).style.display).toBe("");
      expect(view.state.doc.toString().startsWith("|glyphbase-line")).toBe(true);
    } finally {
      view.destroy();
      document.body.removeChild(container);
    }
  });

  it("连续多事务输入（复现矩阵关键路径）后 doc 完整", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const view = new EditorView({
      state: EditorState.create({
        doc: "glyphbase-line\n",
        extensions: [repaintGuard()],
      }),
      parent: container,
    });
    try {
      for (let i = 0; i < 5; i++) {
        view.dispatch({ changes: { from: 0, insert: "|" } });
      }
      expect(view.state.doc.toString().startsWith("|||||")).toBe(true);
    } finally {
      view.destroy();
      document.body.removeChild(container);
    }
  });
});

describe("注入面契约（防复发——guard 从宿主被移除即 bug 复现）", () => {
  const HOSTS = [
    "src/panels/editor/useCodeMirror.ts",
    "src/panels/diff/DiffPanel.tsx",
    "src/panels/gitshow/GitShowPanel.tsx",
    "src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx",
  ];

  it("全部 CM6 宿主 extensions 数组保留 repaintGuard() 注入", () => {
    for (const rel of HOSTS) {
      const src = readFileSync(join(SRC_ROOT, rel), "utf8");
      expect(src, `${rel} 应注入 repaintGuard()`).toContain("repaintGuard()");
    }
  });

  it("repaintGuard 模块自身导出原语与扩展（老代码对照：删除即红）", () => {
    expect(typeof refreshLineByToggle).toBe("function");
    expect(typeof repaintGuard).toBe("function");
  });
});
