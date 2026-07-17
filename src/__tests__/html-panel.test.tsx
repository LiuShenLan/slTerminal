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
    }, { timeout: 3000 });
  });

  it("iframe sandbox 为 allow-scripts", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe.getAttribute("sandbox")).toBe("allow-scripts");
    }, { timeout: 3000 });
  });

  it("iframe 不含 srcDoc 以外的 url 属性", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      // src 不设置（用 srcDoc），src 应为空或 about:blank
      const src = iframe.getAttribute("src");
      expect(src === null || src === "" || src === "about:blank").toBe(true);
    }, { timeout: 3000 });
  });

  it('加载完成后"加载中"文字消失', async () => {
    mocks.mockReadFile.mockResolvedValue("<p>done</p>");
    const { queryByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(queryByText("加载中...")).toBeNull();
    }, { timeout: 3000 });
  });

  it("readFile reject 显示错误信息", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("权限不足"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText("加载失败: 权限不足")).toBeDefined();
    }, { timeout: 3000 });
  });

  it("错误信息红色", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("fail"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const el = getByText("加载失败: fail");
      expect(el.style.color).toBe("rgb(244, 71, 71)");
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
  });

  it("很大 HTML 内容 srcDoc 正常包含", async () => {
    const bigContent = "<html><body>" + "x".repeat(100_000) + "</body></html>";
    mocks.mockReadFile.mockResolvedValue(bigContent);
    const { getByTitle } = renderHtmlPanel("C:/test/big.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/big.html");
      expect(iframe.getAttribute("srcDoc")).toContain("x".repeat(100_000));
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
  });

  it("注入脚本含片段链接拦截 scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement;
      expect(iframe.getAttribute("srcDoc")).toContain("scrollIntoView");
    }, { timeout: 3000 });
  });

  it("H1: 注入脚本含 scrollIntoView + class-based toggle", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("scrollIntoView");
      expect(doc).toContain("classList.add");
      expect(doc).toContain("classList.remove");
    }, { timeout: 3000 });
  });

  it("H2: 注入脚本含 closest('a') 链接检测", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("closest");
      expect(doc).toContain('"a"');
    }, { timeout: 3000 });
  });

  it("H3: 片段链接拦截使用 preventDefault", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("preventDefault");
    }, { timeout: 3000 });
  });

  it("H4: 注入脚本含 dataset.sltermHash 状态跟踪", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const doc = (getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement).getAttribute("srcDoc")!;
      expect(doc).toContain("dataset.sltermHash");
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });
  });

  it("H6: 注入后原始可见内容保留", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello World</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/a.html") as HTMLIFrameElement;
      expect(iframe.getAttribute("srcDoc")).toContain("<h1>Hello World</h1>");
    }, { timeout: 3000 });
  });

  // ==========================================================================
  // renderer="always" 生命周期
  // ==========================================================================

  it("相同 filePath 重渲染不触发 readFile 多次", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { rerender } = renderHtmlPanel("C:/test/a.html");

    await waitFor(() => {
      expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

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
    }, { timeout: 3000 });

    unmount();

    expect(document.querySelector("iframe")).toBeNull();
  });

  // ==========================================================================
  // postMessage 键盘转发桥
  //
  // 注意：handleMessage 校验 e.origin === "null" + e.source === iframe.contentWindow。
  // window.postMessage() 在 jsdom 中不走 window.dispatchEvent（与浏览器行为不同），
  // 故直接 dispatchEvent(new MessageEvent(...)) 构造测试消息。spy 统计需过滤
  // MessageEvent（测试自身派发），只检查 KeyboardEvent（handler 派发）。
  // ==========================================================================

  /** 辅助：构造通过 origin + source 校验的 MessageEvent 并 dispatch */
  function dispatchTrustedKey(iframe: HTMLIFrameElement, data: Record<string, unknown>) {
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: "null",
        source: iframe.contentWindow,
        data,
      }),
    );
  }

  /** 从 spy 中统计 handler 派发的 KeyboardEvent */
  function kbDispatchCount(spy: ReturnType<typeof vi.spyOn>) {
    return spy.mock.calls.filter(
      (call: unknown[]) => call[0] instanceof KeyboardEvent,
    ).length;
  }

  /** 等待 iframe 渲染后获取其 DOM 元素 */
  async function getRenderedIframe(filePath = "C:/test/a.html") {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel(filePath);
    const iframe = await waitFor(() => getByTitle(`HTML 预览: ${filePath}`) as HTMLIFrameElement, { timeout: 3000 });
    return { iframe, getByTitle };
  }

  it("postMessage 命中全局快捷键 → 派发 KeyboardEvent", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyW",
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
      code: "KeyW", key: "w",
    });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBeGreaterThan(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("postMessage 非 slterm_key type → 忽略", async () => {
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, { type: "other", fingerprint: "Ctrl+KeyW" });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
  });

  it("postMessage 非全局快捷键 → 忽略", async () => {
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyA",
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
      code: "KeyA", key: "a",
    });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
  });

  it("无 filePath 时不创建 iframe（error 态）", () => {
    const { queryByTitle } = renderHtmlPanel(undefined);
    expect(queryByTitle(/HTML 预览/)).toBeNull();
  });

  // ==========================================================================
  // SEC-03: origin + source 校验
  // ==========================================================================

  it("SEC-03: 伪造 origin 的消息被忽略", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    // 用非 "null" origin 构造消息 → origin 校验不通过，忽略
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: "http://evil.com",
        source: iframe.contentWindow,
        data: {
          type: "slterm_key",
          fingerprint: "Ctrl+KeyW",
          ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
          code: "KeyW", key: "w",
        },
      }),
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-03: 非本 iframe source 的消息被忽略", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    // 用 window（父窗口）作为 source → source 校验不通过，忽略
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: "null",
        source: window, // 非 iframe.contentWindow
        data: {
          type: "slterm_key",
          fingerprint: "Ctrl+KeyW",
          ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
          code: "KeyW", key: "w",
        },
      }),
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-03: 本 iframe 消息（origin=null + source 匹配）正常转发", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyW",
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
      code: "KeyW", key: "w",
    });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBeGreaterThan(0);

    // 验证合成 KeyboardEvent 携带信任标记
    const kbCalls = spy.mock.calls.filter(([e]) => e instanceof KeyboardEvent);
    const kbEvent = kbCalls[0]![0] as KeyboardEvent & { __slterm_postMessage?: boolean };
    expect(kbEvent.__slterm_postMessage).toBe(true);

    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-03: origin=null 但 source 为 null 时忽略（无 source 的消息不可信）", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: "null",
        source: null, // 无 source
        data: {
          type: "slterm_key",
          fingerprint: "Ctrl+KeyW",
          ctrlKey: true,
        },
      }),
    );

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  // ==========================================================================
  // 边界 (E10-E14)
  // ==========================================================================

  it("E10: postMessage data 为 null → 不抛异常", async () => {
    const { iframe } = await getRenderedIframe();
    // data 为 null 时 !e.data 短路，不崩溃（前提：通过 origin + source 校验）
    expect(() => {
      dispatchTrustedKey(iframe, null as unknown as Record<string, unknown>);
    }).not.toThrow();
  });

  it("E11: postMessage data 缺 fingerprint → 忽略", async () => {
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, { type: "slterm_key" });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
  });

  it("E12: readFile throw 非 Error 对象 → 错误消息用 String(err)", async () => {
    mocks.mockReadFile.mockRejectedValue("权限不足");
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText("加载失败: 权限不足")).toBeDefined();
    }, { timeout: 3000 });
  });

  it("E13: readFile throw 普通对象 → 不崩溃", async () => {
    mocks.mockReadFile.mockRejectedValue({ code: 500 });
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText(/加载失败/)).toBeDefined();
    }, { timeout: 3000 });
  });

  it("E14: postMessage 缺 ctrlKey 等字段 → KeyboardEvent 用 ?? false 兜底", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    // 只发 fingerprint 和 type，缺所有修饰键字段
    dispatchTrustedKey(iframe, { type: "slterm_key", fingerprint: "Ctrl+KeyW" });

    await new Promise((r) => setTimeout(r, 10));
    const kbCalls = spy.mock.calls.filter(([e]) => e instanceof KeyboardEvent);
    expect(kbCalls.length).toBeGreaterThan(0);
    const event = kbCalls[0]![0] as KeyboardEvent;
    expect(event.ctrlKey).toBe(false);
    expect(event.shiftKey).toBe(false);
    expect(event.code).toBe("");
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });
});
