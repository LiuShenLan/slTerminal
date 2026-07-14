// html-panel.test.tsx — HtmlPanel 组件测试
//
// 覆盖：
//   1. 渲染状态 — loading/loaded(iframe)/error/三态切换
//   2. iframe 属性 — srcDoc/sandbox/style
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签
//   5. 注入脚本内容 — 键盘转发 + 片段链接拦截
//   6. postMessage 键盘转发桥

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  const mockExportContextBindings = vi.fn<() => { keystroke: string }[]>(() => []);
  return {
    mockReadFile,
    mockExportContextBindings,
    resetAll() {
      mockReadFile.mockReset();
      mockExportContextBindings.mockReset();
      mockExportContextBindings.mockReturnValue([]);
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readFile: mocks.mockReadFile,
}));

vi.mock("../features/shortcuts/ShortcutRegistry", () => ({
  getShortcutRegistry: () => ({
    exportContextBindings: mocks.mockExportContextBindings,
  }),
}));

import { HtmlPanel } from "../panels/html";

function renderHtmlPanel(filePath: string | undefined) {
  return render(
    React.createElement(HtmlPanel, {
      params: {
        panelId: "test-panel-1",
        filePath,
      },
    }),
  );
}

describe("HtmlPanel", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  afterEach(() => {
    cleanup();
  });

  // ==========================================================================
  // 渲染状态
  // ==========================================================================

  it("初始渲染显示加载中", () => {
    mocks.mockReadFile.mockReturnValue(new Promise(() => {}));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    expect(getByText("加载中...")).toBeDefined();
  });

  it("加载完成后渲染 iframe（srcDoc）", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe).toBeDefined();
      expect(iframe.tagName).toBe("IFRAME");
    });
  });

  it("iframe sandbox 为 allow-scripts", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe.getAttribute("sandbox")).toBe("allow-scripts");
    });
  });

  it("iframe 不含 srcDoc 以外的 url 属性", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      // src 不设置（用 srcDoc），src 应为空或 about:blank
      const src = iframe.getAttribute("src");
      expect(src === null || src === "" || src === "about:blank").toBe(true);
    });
  });

  it('加载完成后"加载中"文字消失', async () => {
    mocks.mockReadFile.mockResolvedValue("<p>done</p>");
    const { queryByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(queryByText("加载中...")).toBeNull();
    });
  });

  it("readFile reject 显示错误信息", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("权限不足"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText("加载失败: 权限不足")).toBeDefined();
    });
  });

  it("错误信息红色", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("fail"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const el = getByText("加载失败: fail");
      expect(el.style.color).toBe("rgb(244, 71, 71)");
    });
  });

  it("filePath 为 undefined 显示错误", () => {
    const { getByText } = renderHtmlPanel(undefined);
    expect(getByText("加载失败: 未指定文件路径")).toBeDefined();
  });

  it("加载中背景色为 PANEL_BG", () => {
    mocks.mockReadFile.mockReturnValue(new Promise(() => {}));
    const { container } = renderHtmlPanel("C:/test/index.html");
    const outerDiv = container.firstChild as HTMLElement;
    expect(outerDiv.style.background).toBe("rgb(30, 30, 30)");
  });

  it("iframe 样式填满容器", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe.style.width).toBe("100%");
      expect(iframe.style.height).toBe("100%");
      expect(iframe.style.border).toBe("medium");
    });
  });

  // ==========================================================================
  // 竞态取消
  // ==========================================================================

  it("快速切换 filePath 后旧请求结果不覆盖新请求", async () => {
    let resolveOld: (v: string) => void = () => {};
    const oldPromise = new Promise<string>((resolve) => {
      resolveOld = resolve;
    });

    mocks.mockReadFile
      .mockReturnValueOnce(oldPromise)
      .mockResolvedValueOnce("<h1>New Content</h1>");

    const { rerender, getByTitle } = renderHtmlPanel("C:/test/old.html");

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "test-panel-1", filePath: "C:/test/new.html" },
      }),
    );

    resolveOld("<h1>Old Content</h1>");

    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/new.html");
      expect(iframe.getAttribute("srcDoc")).toContain("<h1>New Content</h1>");
    });
  });

  it("组件卸载后 readFile resolve 不报错", async () => {
    let resolveLater: (v: string) => void = () => {};
    const pending = new Promise<string>((resolve) => {
      resolveLater = resolve;
    });
    mocks.mockReadFile.mockReturnValue(pending);

    const { unmount } = renderHtmlPanel("C:/test/index.html");
    unmount();

    resolveLater("<p>late</p>");
    await new Promise((r) => setTimeout(r, 10));
  });

  it("连续三次切换只显示最后一次结果", async () => {
    let resolveFirst: (v: string) => void = () => {};
    const firstPromise = new Promise<string>((r) => {
      resolveFirst = r;
    });

    mocks.mockReadFile
      .mockReturnValueOnce(firstPromise)
      .mockResolvedValueOnce("<h1>Second</h1>")
      .mockResolvedValueOnce("<h1>Third</h1>");

    const { rerender, getByTitle } = renderHtmlPanel("C:/test/1.html");

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/2.html" },
      }),
    );

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/3.html" },
      }),
    );

    resolveFirst("<h1>First</h1>");

    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/3.html");
      expect(iframe.getAttribute("srcDoc")).toContain("<h1>Third</h1>");
    });
  });

  // ==========================================================================
  // 边界
  // ==========================================================================

  it("空 HTML 文件渲染含注入脚本的完整文档", async () => {
    mocks.mockReadFile.mockResolvedValue("");
    const { getByTitle } = renderHtmlPanel("C:/test/empty.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/empty.html");
      // 空 HTML 经 injectScript 后变成最小完整文档
      expect(iframe.getAttribute("srcDoc")).toContain("<html>");
    });
  });

  it("很大 HTML 内容 srcDoc 正常包含", async () => {
    const bigContent = "<html><body>" + "x".repeat(100_000) + "</body></html>";
    mocks.mockReadFile.mockResolvedValue(bigContent);
    const { getByTitle } = renderHtmlPanel("C:/test/big.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/big.html");
      expect(iframe.getAttribute("srcDoc")).toContain("x".repeat(100_000));
    });
  });

  it("HTML 含 script 标签——原始 script 与注入脚本共存于 srcDoc", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "<html><body><script>document.body.innerHTML='JS OK'</script></body></html>",
    );
    const { getByTitle } = renderHtmlPanel("C:/test/script.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/script.html");
      const doc = iframe.getAttribute("srcDoc")!;
      expect(doc).toContain("JS OK");
      expect(doc).toContain("slterm_key");
    });
  });

  // ==========================================================================
  // 注入脚本内容验证（键盘转发 + 片段链接拦截）
  // ==========================================================================

  it("注入脚本含键盘转发标识符", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement;
      expect(iframe.getAttribute("srcDoc")).toContain("slterm_key");
    });
  });

  it("注入脚本含片段链接拦截 scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement;
      expect(iframe.getAttribute("srcDoc")).toContain("scrollIntoView");
    });
  });

  it("H1: 注入脚本含 scrollIntoView + class-based toggle", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("scrollIntoView");
      expect(doc).toContain("classList.add");
      expect(doc).toContain("classList.remove");
    });
  });

  it("H2: 注入脚本含 closest('a') 链接检测", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("closest");
      expect(doc).toContain('"a"');
    });
  });

  it("H3: 片段链接拦截使用 preventDefault", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("preventDefault");
    });
  });

  it("H4: 注入脚本含 dataset.sltermHash 状态跟踪", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("dataset.sltermHash");
    });
  });

  it("H5: 注入脚本含键盘转发 + 片段拦截 + CSS 注入", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      // 键盘转发
      expect(doc).toContain("slterm_key");
      expect(doc).toContain("postMessage");
      // 片段拦截 + toggle
      expect(doc).toContain("scrollIntoView");
      expect(doc).toContain("classList.add");
      expect(doc).toContain("classList.remove");
      // CSS 注入
      expect(doc).toContain("createElement");
      expect(doc).toContain("slterm-target");
      // 两个 addEventListener
      const matches = doc.match(/addEventListener/g);
      expect(matches).not.toBeNull();
      expect(matches!.length).toBe(2);
    });
  });

  it("H6: 注入后原始可见内容保留", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello World</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement;
      expect(iframe.getAttribute("srcDoc")).toContain("<h1>Hello World</h1>");
    });
  });

  // ==========================================================================
  // renderer="always" 生命周期
  // ==========================================================================

  it("相同 filePath 重渲染不触发 readFile 多次", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { rerender } = renderHtmlPanel("C:/test/a.html");

    await waitFor(() => {
      expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
    });

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/a.html" },
      }),
    );

    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("unmount 后 cleanup 阻止 setState", async () => {
    let resolveLater: (v: string) => void = () => {};
    const pending = new Promise<string>((r) => {
      resolveLater = r;
    });
    mocks.mockReadFile.mockReturnValue(pending);

    const { unmount, queryByText } = renderHtmlPanel("C:/test/a.html");
    expect(queryByText("加载中...")).toBeDefined();

    unmount();

    resolveLater("<p>late</p>");
    await new Promise((r) => setTimeout(r, 10));

    expect(document.querySelector("iframe")).toBeNull();
  });

  it("iframe 在 unmount 后被销毁", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle, unmount } = renderHtmlPanel("C:/test/a.html");

    await waitFor(() => {
      expect(getByTitle("HTML 预览: C:/test/a.html")).toBeDefined();
    });

    unmount();

    expect(document.querySelector("iframe")).toBeNull();
  });

  // ==========================================================================
  // postMessage 键盘转发桥
  // ==========================================================================

  it("postMessage 命中全局快捷键 → window.dispatchEvent", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");

    const spy = vi.spyOn(window, "dispatchEvent");
    window.postMessage(
      {
        type: "slterm_key",
        fingerprint: "Ctrl+KeyW",
        ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
        code: "KeyW", key: "w",
      },
      "*",
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("postMessage 非 slterm_key type → 忽略", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");

    const spy = vi.spyOn(window, "dispatchEvent");
    window.postMessage({ type: "other", fingerprint: "Ctrl+KeyW" }, "*");

    await new Promise((r) => setTimeout(r, 10));
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("postMessage 非全局快捷键 → 忽略", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");

    const spy = vi.spyOn(window, "dispatchEvent");
    window.postMessage(
      {
        type: "slterm_key",
        fingerprint: "Ctrl+KeyA",
        ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
        code: "KeyA", key: "a",
      },
      "*",
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("无 filePath 时不创建 iframe（error 态）", () => {
    const { queryByTitle } = renderHtmlPanel(undefined);
    expect(queryByTitle(/HTML 预览/)).toBeNull();
  });

  // ==========================================================================
  // 边界 (E10-E14)
  // ==========================================================================

  it("E10: postMessage data 为 null → 不抛异常", () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    // data 为 null 时 !e.data 短路，不崩溃
    expect(() => window.postMessage(null, "*")).not.toThrow();
  });

  it("E11: postMessage data 缺 fingerprint → 忽略", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");

    const spy = vi.spyOn(window, "dispatchEvent");
    window.postMessage({ type: "slterm_key" }, "*");

    await new Promise((r) => setTimeout(r, 10));
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("E12: readFile throw 非 Error 对象 → 错误消息用 String(err)", async () => {
    mocks.mockReadFile.mockRejectedValue("权限不足");
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText("加载失败: 权限不足")).toBeDefined();
    });
  });

  it("E13: readFile throw 普通对象 → 不崩溃", async () => {
    mocks.mockReadFile.mockRejectedValue({ code: 500 });
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText(/加载失败/)).toBeDefined();
    });
  });

  it("E14: postMessage 缺 ctrlKey 等字段 → KeyboardEvent 用 ?? false 兜底", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");

    const spy = vi.spyOn(window, "dispatchEvent");
    // 只发 fingerprint 和 type，缺所有修饰键字段
    window.postMessage(
      { type: "slterm_key", fingerprint: "Ctrl+KeyW" },
      "*",
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(spy).toHaveBeenCalled();
    const event = spy.mock.calls[spy.mock.calls.length - 1]?.[0] as KeyboardEvent;
    expect(event.ctrlKey).toBe(false);
    expect(event.shiftKey).toBe(false);
    expect(event.code).toBe("");
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });
});
