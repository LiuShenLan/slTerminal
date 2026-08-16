// html-panel.test.tsx — HtmlPanel 组件测试
//
// 覆盖：
//   1. 渲染状态 — loading/loaded(iframe)/error/三态切换
//   2. iframe 属性 — srcDoc/sandbox/style
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签
//   5. 注入脚本内容 — 键盘转发 + 片段链接拦截
//   6. postMessage 键盘转发桥 + SEC-03 校验（含负面用例；jsdom 模拟，真实 WebView2 由 L4 验收）

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

/** 等待 loaded 态 iframe 出现并返回元素（IHE-08：消除重复 waitFor 模式） */
async function waitForLoaded(
  getByTitle: (title: string) => HTMLElement,
  filePath: string,
): Promise<HTMLIFrameElement> {
  return waitFor(() => getByTitle(`HTML 预览: ${filePath}`) as HTMLIFrameElement, {
    timeout: 3000,
  });
}

/** 等待 error 态文案出现（IHE-08：消除重复 waitFor 模式） */
async function waitForError(
  getByText: (text: string | RegExp) => HTMLElement,
  message: string | RegExp,
): Promise<void> {
  await waitFor(() => {
    expect(getByText(message)).toBeDefined();
  }, { timeout: 3000 });
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
    const iframe = await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(iframe.tagName).toBe("IFRAME");
  });

  it("iframe sandbox 为 allow-scripts", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(iframe.getAttribute("sandbox")).toBe("allow-scripts");
  });

  it("iframe 不含 srcDoc 以外的 url 属性", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/index.html");
    // src 不设置（用 srcDoc），src 应为空或 about:blank
    const src = iframe.getAttribute("src");
    expect(src === null || src === "" || src === "about:blank").toBe(true);
  });

  it('加载完成后"加载中"文字消失', async () => {
    mocks.mockReadFile.mockResolvedValue("<p>done</p>");
    const { queryByText, getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(queryByText("加载中...")).toBeNull();
  });

  it("readFile reject 显示错误信息", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("权限不足"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitForError(getByText, "加载失败: 权限不足");
  });

  it("错误信息红色", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("fail"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitForError(getByText, "加载失败: fail");
    const el = getByText("加载失败: fail");
    expect(el.style.color).toBe("rgb(217, 112, 107)");
  });

  it("filePath 为 undefined 显示错误", () => {
    const { getByText } = renderHtmlPanel(undefined);
    expect(getByText("加载失败: 未指定文件路径")).toBeDefined();
  });

  it("加载中背景色为 PANEL_BG", () => {
    mocks.mockReadFile.mockReturnValue(new Promise(() => {}));
    const { container } = renderHtmlPanel("C:/test/index.html");
    const outerDiv = container.firstChild as HTMLElement;
    expect(outerDiv.style.background).toBe("rgb(10, 10, 11)");
  });

  it("iframe 样式填满容器", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(iframe.style.width).toBe("100%");
    expect(iframe.style.height).toBe("100%");
    expect(iframe.style.border).toBe("medium");
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

    const iframe = await waitForLoaded(getByTitle, "C:/test/new.html");
    expect(iframe.getAttribute("srcDoc")).toContain("<h1>New Content</h1>");
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

    const iframe = await waitForLoaded(getByTitle, "C:/test/3.html");
    expect(iframe.getAttribute("srcDoc")).toContain("<h1>Third</h1>");
  });

  // ==========================================================================
  // 边界
  // ==========================================================================

  it("空 HTML 文件渲染含注入脚本的完整文档", async () => {
    mocks.mockReadFile.mockResolvedValue("");
    const { getByTitle } = renderHtmlPanel("C:/test/empty.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/empty.html");
    // 空 HTML 经 injectScript 后变成最小完整文档
    expect(iframe.getAttribute("srcDoc")).toContain("<html>");
  });

  it("很大 HTML 内容 srcDoc 正常包含", async () => {
    const bigContent = "<html><body>" + "x".repeat(100_000) + "</body></html>";
    mocks.mockReadFile.mockResolvedValue(bigContent);
    const { getByTitle } = renderHtmlPanel("C:/test/big.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/big.html");
    expect(iframe.getAttribute("srcDoc")).toContain("x".repeat(100_000));
  });

  it("HTML 含 script 标签——原始 script 与注入脚本共存于 srcDoc", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "<html><body><script>document.body.innerHTML='JS OK'</script></body></html>",
    );
    const { getByTitle } = renderHtmlPanel("C:/test/script.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/script.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("JS OK");
    expect(doc).toContain("slterm_key");
  });

  // ==========================================================================
  // 注入脚本内容验证（键盘转发 + 片段链接拦截）
  // ==========================================================================

  it("注入脚本含键盘转发标识符", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("slterm_key");
  });

  it("注入脚本含片段链接拦截 scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("scrollIntoView");
  });

  it("H1: 注入脚本含 scrollIntoView + class-based toggle", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("scrollIntoView");
    expect(doc).toContain("classList.add");
    expect(doc).toContain("classList.remove");
  });

  it("H2: 注入脚本含 closest('a') 链接检测", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("closest");
    expect(doc).toContain('"a"');
  });

  it("H3: 片段链接拦截使用 preventDefault", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("preventDefault");
  });

  it("H4: 注入脚本含 dataset.sltermHash 状态跟踪", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    expect(doc).toContain("dataset.sltermHash");
  });

  it("H5: 注入脚本含键盘转发 + 片段拦截 + CSS 注入", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
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

  it("H6: 注入后原始可见内容保留", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello World</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/a.html");
    expect(iframe.getAttribute("srcDoc")).toContain("<h1>Hello World</h1>");
  });

  // ==========================================================================
  // 注入脚本关键控制流（IHE-07②：非仅字符串包含）
  //
  // 以上 H1-H6 只做关键词存在断言；本小节用正则/切片断言脚本的控制流结构——
  // 监听绑定（capture phase）、postMessage 字段构造、守卫 → preventDefault 顺序。
  // jsdom 不执行 srcdoc iframe 内脚本，真实执行由 L4 E2E 验收。
  // ==========================================================================

  it("控制流: keydown capture 监听绑定 + postMessage 字段构造完整", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    // 监听绑定：document 级 keydown + 第三参数 true（capture phase，先于页面内脚本拦截）
    expect(doc).toMatch(/document\.addEventListener\("keydown",function\(e\)\{/);
    expect(doc).toMatch(/key:e\.key\},"null"\)\},true\)/);
    // postMessage 消息体：type + fingerprint 合成表达式（修饰键条件拼接后接 code）
    const pmStart = doc.indexOf("window.parent.postMessage({");
    expect(pmStart).toBeGreaterThan(-1);
    const pmEnd = doc.indexOf('key:e.key},"null")', pmStart);
    expect(pmEnd).toBeGreaterThan(-1);
    const pmBody = doc.slice(pmStart, pmEnd + 'key:e.key},"null")'.length);
    expect(pmBody).toContain('type:"slterm_key"');
    expect(pmBody).toContain(
      'fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")' +
        '+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code',
    );
    expect(pmBody).toContain(
      "ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey," +
        "metaKey:e.metaKey,code:e.code,key:e.key",
    );
    // postMessage 目标 origin："null"（srcdoc opaque origin 序列化，父窗口据此校验）
    expect(pmBody).toContain('},"null")');
  });

  it("控制流: click capture 监听 + closest/href 守卫 → preventDefault → scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    // 监听绑定：document 级 click + capture phase（事件委托，iframe 内任意深度元素可达）
    expect(doc).toMatch(/document\.addEventListener\("click",function\(e\)\{/);
    expect(doc).toMatch(/\},true\)<\/script>/);
    // 守卫顺序：非 <a> 或 href 非 "#" 开头 → 直接 return（不 preventDefault）
    expect(doc).toMatch(
      /closest\("a"\);if\(!a\)return;var h=a\.getAttribute\("href"\);if\(!h\|\|h\.charAt\(0\)!=="#"\)return;/,
    );
    // 守卫通过后才 preventDefault（拦截默认 #fragment 导航）
    expect(doc).toMatch(/return;e\.preventDefault\(\);var id=h\.slice\(1\);/);
    // 命中目标元素 → classList.add + scrollIntoView（模拟 :target 定位）
    expect(doc).toMatch(/if\(el\)\{el\.classList\.add\("slterm-target"\);el\.scrollIntoView\(/);
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

    await waitForLoaded(getByTitle, "C:/test/a.html");

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
  //
  // 本小节 postMessage 用例均为 jsdom 模拟（dispatchEvent 构造 MessageEvent）——
  // jsdom 无法模拟真实 WebView2 的 origin 序列化/source 引用/消息送达行为，
  // 真实行为由 L4 E2E（真实 WebView2 中 postMessage 往返）验收。
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
    const iframe = await waitForLoaded(getByTitle, filePath);
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
    // 负面用例：type ≠ "slterm_key" 不 dispatch。jsdom 模拟，真实 WebView2 由 L4 验收
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, { type: "other", fingerprint: "Ctrl+KeyW" });

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
  });

  it("postMessage 非全局快捷键 → 忽略", async () => {
    // 负面用例：未知 fingerprint 不 dispatch。jsdom 模拟，真实 WebView2 由 L4 验收
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
    // 负面用例：origin ≠ "null" 不 dispatch。jsdom 模拟，真实 WebView2 由 L4 验收
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
    // 负面用例：source ≠ contentWindow 不 dispatch。jsdom 模拟，真实 WebView2 由 L4 验收
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
    // err 为字符串（非 Error 实例）→ err instanceof Error false 分支：String(err) === "权限不足"。
    // 精确文案断言锁定该分支（若误走 err.message 将显示 "加载失败: undefined"）
    await waitForError(getByText, "加载失败: 权限不足");
  });

  it("E13: readFile throw 普通对象 → String(err) 分支不崩溃", async () => {
    mocks.mockReadFile.mockRejectedValue({ code: 500 });
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    // err 为普通对象 → err instanceof Error false 分支：String({code:500}) === "[object Object]"。
    // 精确文案断言锁定该分支（若误走 err.message 将显示 "加载失败: undefined"）
    await waitForError(getByText, "加载失败: [object Object]");
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
