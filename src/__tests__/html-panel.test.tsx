// html-panel.test.tsx — HtmlPanel 组件测试
//
// 覆盖：
//   1. 渲染状态 — loading/loaded(iframe)/error/三态切换
//   2. iframe 属性 — srcDoc/sandbox/style
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签
//   5. 注入脚本内容 — 键盘转发 + 片段链接拦截
//   6. postMessage 键盘转发桥 + SEC-03 校验（含负面用例；jsdom 模拟，真实 WebView2 由 L4 验收）
//   7. SEC-04 nonce 校验——注入脚本携带面板 nonce / 无 nonce / 伪造 nonce 忽略 / 合法 nonce 触发 / 实例隔离

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, waitFor, fireEvent, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  const mockExportContextBindings = vi.fn<() => { keystroke: string }[]>(() => []);
  // useCodeMirror mock：捕获调用参数（面板层逻辑真实，CM 层隔离——edit 击键
  // 经捕获的 onDocContent 手动驱动，见「viewMode 形态切换」describe）
  const mockUseCodeMirror = vi.fn();
  return {
    mockReadFile,
    mockExportContextBindings,
    mockUseCodeMirror,
    resetAll() {
      mockReadFile.mockReset();
      mockExportContextBindings.mockReset();
      mockUseCodeMirror.mockReset();
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

// S4: HtmlPanel 编辑态经 useCodeMirror 桥接——mock 隔离 CM 实现
// EDITOR_FONT_SPEC: EditorPanel→LargeFileViewer 模块级读取（CP-022 字体单点复用）——
// mock 缺失会致 import 期 TypeError
vi.mock("../panels/editor/useCodeMirror", () => ({
  useCodeMirror: (opts: unknown) => mocks.mockUseCodeMirror(opts),
  EDITOR_FONT_SPEC: { ".cm-scroller": { fontFamily: "monospace" } },
}));

import { HtmlPanel } from "../panels/html";
import { useFontSize } from "../stores/fontSize";

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
  const el = await waitFor(
    () => getByTitle(`HTML 预览: ${filePath}`) as HTMLIFrameElement,
    { timeout: 3000 },
  );
  // flush passive effects（PreviewFrame 消息总线监听注册在 useEffect）——DOM 出现
  // 不保证监听已挂载，紧接派发消息会丢包（顺序竞态，见 postMessage 用例失败史）
  await act(async () => {});
  return el;
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
    // 四个 addEventListener：keydown/click（转发与拦截）+ wheel/message（缩放运行时）
    const matches = doc.match(/addEventListener/g);
    expect(matches).not.toBeNull();
    expect(matches!.length).toBe(4);
    // 缩放运行时（第 4 段）：wheel 捕获 + 下行 message 监听 + 挂载调用
    // （运行时为参数化函数 function(doc,win)，wheel/message 挂参数对象上）
    expect(doc).toContain('doc.addEventListener("wheel"');
    expect(doc).toContain('win.addEventListener("message"');
    expect(doc).toContain("sltermZoom(document,window)");
    expect(doc).toContain("slterm_zoom");
    expect(doc).toContain("slterm_reset");
  });

  it("H6: 注入后原始可见内容保留", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello World</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const iframe = await waitForLoaded(getByTitle, "C:/test/a.html");
    expect(iframe.getAttribute("srcDoc")).toContain("<h1>Hello World</h1>");
  });

  it("H7: 注入脚本整段可被 JS 解析（防拼接边界 SyntaxError 类回归）", async () => {
    // 2026-09-06 实证：click 段原为 script 末语句无分号，追加第 4 段（zoom 运行时）后
    // 同串拼接 "},true)var sltermZoom" 无分隔 → 整段注入脚本 SyntaxError —— L2 全绿、
    // 仅 L4 真实 iframe 执行暴露（HUD 永不出现）。本用例把注入脚本体经 new Function
    // 做 parse-only 校验，锁死拼接边界。
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    const scriptBody = /<script>([\s\S]*?)<\/script>/.exec(doc)?.[1];
    expect(scriptBody).toBeDefined();
    expect(() => new Function(scriptBody!)).not.toThrow();
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
    expect(doc).toMatch(/key:e\.key\},"\*"\)\},true\)/);
    // postMessage 消息体：type + fingerprint 合成表达式（修饰键条件拼接后接 code）
    const pmStart = doc.indexOf("window.parent.postMessage({");
    expect(pmStart).toBeGreaterThan(-1);
    const pmEnd = doc.indexOf('key:e.key},"*")', pmStart);
    expect(pmEnd).toBeGreaterThan(-1);
    const pmBody = doc.slice(pmStart, pmEnd + 'key:e.key},"*")'.length);
    expect(pmBody).toContain('type:"slterm_key"');
    expect(pmBody).toContain(
      'fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")' +
        '+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code',
    );
    expect(pmBody).toContain(
      "ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey," +
        "metaKey:e.metaKey,code:e.code,key:e.key",
    );
    // postMessage targetOrigin "*"（2026-09-06 实证：须匹配接收方窗口 origin；
    // opaque 源只影响父侧 e.origin 序列化 "null"，与发送 targetOrigin 无关）
    expect(pmBody).toContain('},"*")');
  });

  it("控制流: click capture 监听 + closest/href 守卫 → preventDefault → scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    // 监听绑定：document 级 click + capture phase（事件委托，iframe 内任意深度元素可达）
    expect(doc).toMatch(/document\.addEventListener\("click",function\(e\)\{/);
    expect(doc).toMatch(/\},true\);var sltermZoom=/);
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

  /** 从 iframe srcDoc 提取注入脚本中拼入的 nonce（SEC-04 校验值） */
  function extractNonce(iframe: HTMLIFrameElement): string {
    const doc = iframe.getAttribute("srcDoc") ?? "";
    const m = /nonce:"([0-9a-f]+)"/.exec(doc);
    if (!m) throw new Error("srcDoc 未找到 nonce——注入脚本结构已变，测试需同步");
    return m[1]!;
  }

  /**
   * 辅助：构造通过 origin + source 校验的 MessageEvent 并 dispatch。
   * @param nonceOverride 消息携带的 nonce 值——缺省自动取 srcDoc 中合法 nonce；
   *   "omit" = 消息不带 nonce 字段（SEC-04 负面用例）；其它字符串 = 伪造 nonce
   */
  function dispatchTrustedKey(
    iframe: HTMLIFrameElement,
    data: Record<string, unknown>,
    nonceOverride?: string | "omit",
  ) {
    const payload =
      nonceOverride === "omit"
        ? { ...data }
        : { nonce: nonceOverride ?? extractNonce(iframe), ...data };
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: "null",
        source: iframe.contentWindow,
        data: payload,
      }),
    );
  }

  /** 从 spy 中统计 handler 派发的 KeyboardEvent */
  function kbDispatchCount(spy: ReturnType<typeof vi.spyOn>) {
    return spy.mock.calls.filter(
      (call: unknown[]) => call[0] instanceof KeyboardEvent,
    ).length;
  }

  /** 等待 iframe 渲染后获取其 DOM 元素（含 container/unmount 供 HUD 用例） */
  async function getRenderedIframe(filePath = "C:/test/a.html") {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { container, getByTitle, unmount } = renderHtmlPanel(filePath);
    const iframe = await waitForLoaded(getByTitle, filePath);
    return { container, iframe, getByTitle, unmount };
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
    // 只发 fingerprint 和 type（nonce 自动补齐），缺所有修饰键字段
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

  // ==========================================================================
  // SEC-04: postMessage nonce 校验（面板挂载期随机值，防 iframe 内任意脚本伪造）
  //
  // 威胁模型：iframe 内任意脚本可 postMessage 伪造 slterm_key（origin/source 校验
  // 挡不住 iframe 自身内容——origin 同为 "null"、source 同为 contentWindow）。
  // 修复 = 面板挂载生成随机 nonce 拼入注入脚本，父窗口校验消息 nonce 一致才转发。
  // jsdom 无法执行 srcdoc 内脚本，这里验证父窗口校验逻辑 + 注入脚本含 nonce；
  // 真实 WebView2 往返由 L4 验收。
  // ==========================================================================

  it("SEC-04: 注入脚本的 keydown postMessage 携带面板 nonce", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");
    const doc = (await waitForLoaded(getByTitle, "C:/test/a.html")).getAttribute("srcDoc")!;
    // nonce 为 128 位随机数十六进制串（32 字符），拼在 type 字段之后
    expect(doc).toMatch(/type:"slterm_key",nonce:"[0-9a-f]{32}",fingerprint:/);
  });

  it("SEC-04: 合法 nonce 的消息正常触发快捷键", async () => {
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

  it("SEC-04: 无 nonce 的消息被忽略（伪造消息不带 nonce）", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyW",
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
      code: "KeyW", key: "w",
    }, "omit");

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-04: nonce 不符的消息被忽略（伪造 nonce）", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyW",
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
      code: "KeyW", key: "w",
    }, "00000000000000000000000000000000");

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-04: nonce 非字符串（如数字）被忽略", async () => {
    mocks.mockExportContextBindings.mockReturnValue([{ keystroke: "Ctrl+KeyW" }]);
    const { iframe } = await getRenderedIframe();

    const spy = vi.spyOn(window, "dispatchEvent");
    dispatchTrustedKey(iframe, {
      type: "slterm_key",
      fingerprint: "Ctrl+KeyW",
      ctrlKey: true,
    }, 123 as unknown as string);

    await new Promise((r) => setTimeout(r, 10));
    expect(kbDispatchCount(spy)).toBe(0);
    spy.mockRestore();
    mocks.mockExportContextBindings.mockReturnValue([]);
  });

  it("SEC-04: 同面板重渲染 nonce 稳定（注入脚本与校验值不漂移）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { rerender, getByTitle } = renderHtmlPanel("C:/test/a.html");
    const first = await waitForLoaded(getByTitle, "C:/test/a.html");
    const firstNonce = extractNonce(first);

    // 触发一次重渲染（StrictMode 双渲染同源场景：ref 惰性初始化不重复生成）
    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/a.html" },
      }),
    );

    const second = await waitForLoaded(getByTitle, "C:/test/a.html");
    expect(extractNonce(second)).toBe(firstNonce);
  });

  it("SEC-04: 两个独立面板实例 nonce 互不相同（防全局共享 nonce）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = render(
      React.createElement("div", null, [
        React.createElement(HtmlPanel, { key: "a", params: { panelId: "pa", filePath: "C:/test/a.html" } }),
        React.createElement(HtmlPanel, { key: "b", params: { panelId: "pb", filePath: "C:/test/b.html" } }),
      ]),
    );
    const iframeA = await waitForLoaded(getByTitle, "C:/test/a.html");
    const iframeB = await waitForLoaded(getByTitle, "C:/test/b.html");
    expect(extractNonce(iframeA)).toMatch(/^[0-9a-f]{32}$/);
    expect(extractNonce(iframeB)).toMatch(/^[0-9a-f]{32}$/);
    expect(extractNonce(iframeA)).not.toBe(extractNonce(iframeB));
  });

  // ==========================================================================
  // Ctrl+滚轮缩放 HUD（瞬态缩放指示器）
  //
  // iframe 文档内缩放执行由注入脚本完成（html-zoom-runtime.test.ts 桩执行覆盖）；
  // 本小节锁父窗口侧：slterm_zoom 上行的 HUD 显示/隐藏/续期/复位下行/重建复位。
  // 缩放值的等比步进/clamp 属 zoomRuntime（注入侧），此处只喂可信消息断言 UI。
  // ==========================================================================

  /** 构造通过 origin + source + nonce 校验的 slterm_zoom 上行消息 */
  function dispatchZoom(
    iframe: HTMLIFrameElement,
    zoom: number,
    nonceOverride?: string | "omit",
  ) {
    dispatchTrustedKey(iframe, { type: "slterm_zoom", zoom }, nonceOverride);
  }

  /** 查询 HUD 气泡元素（data-e2e） */
  function queryHud(container: HTMLElement): HTMLElement | null {
    return container.querySelector('[data-e2e="html-zoom-hud"]');
  }

  it("HUD: 默认无缩放时气泡不显示", async () => {
    const { container, getByTitle } = renderHtmlPanel("C:/test/a.html");
    await waitForLoaded(getByTitle, "C:/test/a.html");
    expect(queryHud(container)).toBeNull();
  });

  it("HUD: 可信 slterm_zoom 上报 → 气泡显示百分比与重置按钮", async () => {
    const { container, iframe } = await getRenderedIframe();
    await act(async () => dispatchZoom(iframe, 1.1));
    const hud = queryHud(container);
    expect(hud).not.toBeNull();
    expect(hud!.textContent).toContain("110%");
    expect(hud!.querySelector('[data-e2e="html-zoom-reset"]')).not.toBeNull();
  });

  it("HUD: 百分比取整显示（zoom 1.234 → 123%）", async () => {
    const { container, iframe } = await getRenderedIframe();
    await act(async () => dispatchZoom(iframe, 1.234));
    expect(queryHud(container)!.textContent).toContain("123%");
  });

  it("HUD: 回落 100% 的变化消息仍显示气泡（Chrome 语义——超时才消失）", async () => {
    const { container, iframe } = await getRenderedIframe();
    await act(async () => dispatchZoom(iframe, 1.5));
    expect(queryHud(container)!.textContent).toContain("150%");
    await act(async () => dispatchZoom(iframe, 1.0));
    expect(queryHud(container)).not.toBeNull();
    expect(queryHud(container)!.textContent).toContain("100%");
  });

  it("HUD: 等值 zoom 消息不续期隐藏计时（防重复上报造成僵尸气泡）", async () => {
    const { container, iframe } = await getRenderedIframe();
    // 渲染完成后才启用假时钟（waitForLoaded 依赖真实 timer）
    vi.useFakeTimers();
    try {
      await act(async () => dispatchZoom(iframe, 1.1));
      expect(queryHud(container)).not.toBeNull();
      // 2.5s 时收到等值重复上报（文档内 zoom 未变，正常不发送；恶意/自身脚本兜底防御）
      act(() => vi.advanceTimersByTime(2500));
      expect(queryHud(container)).not.toBeNull();
      await act(async () => dispatchZoom(iframe, 1.1));
      // 等值消息若续期计时 → 此时仍显示；不续期 → 首条后 3s 已隐藏
      act(() => vi.advanceTimersByTime(600));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: 3s 无缩放操作自动消失；期间续期", async () => {
    const { container, iframe } = await getRenderedIframe();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchZoom(iframe, 1.1));
      // 2.9s 内仍在（未到 3s）
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      // 续期：新变化消息重置 3s 计时
      await act(async () => dispatchZoom(iframe, 1.2));
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      // 距最后消息超过 3s → 消失
      act(() => vi.advanceTimersByTime(200));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: SEC-04 负面——伪造/缺失 nonce、非数值 zoom 不弹气泡", async () => {
    const { container, iframe } = await getRenderedIframe();
    // 缺失 nonce
    await act(async () => dispatchZoom(iframe, 1.5, "omit"));
    expect(queryHud(container)).toBeNull();
    // 伪造 nonce
    await act(async () => dispatchZoom(iframe, 1.5, "00000000000000000000000000000000"));
    expect(queryHud(container)).toBeNull();
    // 非数值 zoom（字符串/NaN）——isFiniteZoom 拒绝
    await act(async () => {
      dispatchTrustedKey(iframe, { type: "slterm_zoom", zoom: "1.5" });
    });
    expect(queryHud(container)).toBeNull();
    await act(async () => {
      dispatchTrustedKey(iframe, { type: "slterm_zoom", zoom: NaN });
    });
    expect(queryHud(container)).toBeNull();
  });

  it("HUD: SEC-03 负面——伪造 origin / 非本 iframe source 的 zoom 消息不弹", async () => {
    const { container, iframe } = await getRenderedIframe();
    const nonce = extractNonce(iframe);
    // 伪造 origin
    await act(async () => {
      window.dispatchEvent(
        new MessageEvent("message", {
          origin: "http://evil.com",
          source: iframe.contentWindow,
          data: { type: "slterm_zoom", nonce, zoom: 1.5 },
        }),
      );
    });
    expect(queryHud(container)).toBeNull();
    // 非本 iframe source
    await act(async () => {
      window.dispatchEvent(
        new MessageEvent("message", {
          origin: "null",
          source: window,
          data: { type: "slterm_zoom", nonce, zoom: 1.5 },
        }),
      );
    });
    expect(queryHud(container)).toBeNull();
  });

  it("HUD: 点重置 → 下行 slterm_reset + 立即隐藏 + 回声不复活", async () => {
    const { container, iframe } = await getRenderedIframe();
    await act(async () => dispatchZoom(iframe, 1.3));
    expect(queryHud(container)).not.toBeNull();

    // spy 下行 postMessage（jsdom 无真实跨窗派发，拦截原实现防 NotImplemented 风险）
    const postSpy = vi
      .spyOn(iframe.contentWindow!, "postMessage")
      .mockImplementation(() => {});
    fireEvent.click(queryHud(container)!.querySelector('[data-e2e="html-zoom-reset"]')!);
    // 断言须先于 mockRestore（restore 会清调用记录）
    // 下行载荷：slterm_reset + 面板 nonce，targetOrigin "*"（与上行同因，见 keydown 段注释）
    expect(postSpy).toHaveBeenCalledTimes(1);
    expect(postSpy).toHaveBeenCalledWith(
      { type: "slterm_reset", nonce: extractNonce(iframe) },
      "*",
    );
    postSpy.mockRestore();
    // 立即隐藏（不等回声）
    expect(queryHud(container)).toBeNull();
    // 复位回声 zoom=1 与基准等值 → 不复活
    await act(async () => dispatchZoom(iframe, 1.0));
    expect(queryHud(container)).toBeNull();
  });

  it("HUD: iframe 重建（srcDoc 重载）→ 气泡清空归位", async () => {
    const { container, iframe } = await getRenderedIframe();
    await act(async () => dispatchZoom(iframe, 1.7));
    expect(queryHud(container)).not.toBeNull();
    // 模拟文档重载完成的 load 事件（新文档内 zoom 已归 1）
    fireEvent.load(iframe);
    expect(queryHud(container)).toBeNull();
  });

  it("HUD: 卸载后隐藏计时器不泄漏（advance 不抛错）", async () => {
    const { container, iframe, unmount } = await getRenderedIframe();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchZoom(iframe, 1.1));
      expect(queryHud(container)).not.toBeNull();
      unmount();
      // 卸载后推进计时器不抛异常（timer 已清理）
      expect(() => act(() => vi.advanceTimersByTime(5000))).not.toThrow();
    } finally {
      vi.useRealTimers();
    }
  });
});

// ═══════════════════════════════════════════════════════════════════
// S4: viewMode 形态切换（render ↔ edit）+ 草稿快照往返
//
// CM 实现经 mock 隔离；面板层逻辑真实：
//   - useCodeMirror 调用参数（container 随形态 null/div、initialDoc 快照回填、
//     gitGutterEnabled=false）
//   - edit 击键经捕获的 onDocContent 手动驱动 → 断言 render 态 srcDoc 展示草稿
// ═══════════════════════════════════════════════════════════════════

function renderHtmlPanelWithMode(mode?: string) {
  return render(
    React.createElement(HtmlPanel, {
      api: { updateParameters: vi.fn() } as never,
      containerApi: {} as never,
      params: {
        panelId: "test-panel-1",
        filePath: "C:/test/index.html",
        viewMode: mode as never,
      },
    }),
  );
}

describe("HtmlPanel viewMode 形态切换", () => {
  beforeEach(() => {
    mocks.resetAll();
    mocks.mockReadFile.mockResolvedValue("<h1>Disk</h1>");
  });

  afterEach(() => {
    cleanup();
  });

  it("默认形态 render：渲染 iframe + 悬浮切换条（渲染/编辑 两钮）", async () => {
    const { getByTitle, container } = renderHtmlPanelWithMode(undefined);
    await waitForLoaded(getByTitle, "C:/test/index.html");
    const switcher = container.querySelector('[data-e2e="html-mode-switcher"]');
    expect(switcher).not.toBeNull();
    // 两按钮文案 + 当前态高亮 render
    expect(switcher!.textContent).toContain("渲染");
    expect(switcher!.textContent).toContain("编辑");
    const renderBtn = container.querySelector('[data-e2e="html-mode-render"]') as HTMLButtonElement;
    expect(renderBtn.style.background).not.toBe("none");
    // render 态 CM 不挂载（container=null）
    expect(mocks.mockUseCodeMirror).toHaveBeenCalled();
    const calls = mocks.mockUseCodeMirror.mock.calls;
    const lastCall = calls[calls.length - 1]![0] as { container: unknown };
    expect(lastCall.container).toBeNull();
  });

  it("非法 viewMode 回退 render（布局 JSON 旧值容错）", async () => {
    const { getByTitle, container } = renderHtmlPanelWithMode("preview" as never);
    await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(container.querySelector('[data-e2e="html-mode-render"]')).not.toBeNull();
  });

  it("点「编辑」→ edit 形态：CM 容器出现 + useCodeMirror 参数（filePath/initialDoc 快照/gitGutterEnabled=false）", async () => {
    const { getByTitle, container } = renderHtmlPanelWithMode(undefined);
    await waitForLoaded(getByTitle, "C:/test/index.html");
    mocks.mockUseCodeMirror.mockClear();

    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });

    // edit 形态渲染 CM 容器 div（无 iframe）
    expect(container.querySelector("iframe")).toBeNull();
    expect(mocks.mockUseCodeMirror).toHaveBeenCalled();
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const call = cmCalls[cmCalls.length - 1]![0] as {
      filePath?: string;
      initialDoc?: string;
      gitGutterEnabled?: boolean;
      container: unknown;
    };
    expect(call.container).not.toBeNull();
    expect(call.filePath).toBe("C:/test/index.html");
    // 磁盘内容已就绪 → initialDoc 快照回填（免二次读盘）
    expect(call.initialDoc).toContain("<h1>Disk</h1>");
    expect(call.gitGutterEnabled).toBe(false);
  });

  it("edit 态编辑字号接线：store→useCodeMirror props 闭环（Ctrl+滚轮缩放语义，EditorPanel 同款）", async () => {
    useFontSize.setState({ editorFontSize: 14 });
    const { getByTitle, container } = renderHtmlPanelWithMode(undefined);
    await waitForLoaded(getByTitle, "C:/test/index.html");
    mocks.mockUseCodeMirror.mockClear();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });
    // 接线：hook 收到 store 初值字号与 setter（wheel 被 hook 无条件挂载，
    // 缺 props 会吞事件无效果——此断言防接线被删）
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const call = cmCalls[cmCalls.length - 1]![0] as {
      fontSize?: number;
      onFontSizeChange?: (size: number) => void;
    };
    expect(call.fontSize).toBe(14);
    expect(typeof call.onFontSizeChange).toBe("function");
    // 闭环：hook 侧滚轮回调（onFontSizeChange(20)）→ store 更新 → 重渲染回传
    await act(async () => {
      call.onFontSizeChange?.(20);
    });
    expect(useFontSize.getState().editorFontSize).toBe(20);
    const after = mocks.mockUseCodeMirror.mock.calls;
    const last = after[after.length - 1]![0] as { fontSize?: number };
    expect(last.fontSize).toBe(20);
  });

  it("形态切换持久化：api.updateParameters 收到 viewMode patch", async () => {
    const updateParameters = vi.fn();
    const { getByTitle, container } = render(
      React.createElement(HtmlPanel, {
        api: { updateParameters } as never,
        containerApi: { toJSON: () => ({}) } as never,
        params: {
          panelId: "tp",
          filePath: "C:/test/index.html",
          viewMode: "render" as never,
        },
      }),
    );
    await waitForLoaded(getByTitle, "C:/test/index.html");
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });
    expect(updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ viewMode: "edit" }),
    );
  });

  it("edit 击键草稿 → 切回 render → srcDoc 展示草稿（非磁盘内容）", async () => {
    const { getByTitle, container } = renderHtmlPanelWithMode(undefined);
    await waitForLoaded(getByTitle, "C:/test/index.html");

    // 切 edit 并捕获 useCodeMirror 的 onDocContent，模拟击键
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const call = cmCalls[cmCalls.length - 1]![0] as {
      onDocContent?: (text: string, source: string) => void;
    };
    expect(call.onDocContent).toBeDefined();
    await act(async () => {
      call.onDocContent!("<h1>Draft Edit</h1>", "edit");
    });

    // 切回 render → iframe srcDoc 为草稿内容
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-render"]')!);
    });
    const iframe = await waitForLoaded(getByTitle, "C:/test/index.html");
    expect(iframe.getAttribute("srcDoc")).toContain("<h1>Draft Edit</h1>");
    // 读盘不重复（草稿快照回填路径）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("viewMode=edit 布局恢复：直接 edit 形态（无 iframe），读盘一次后快照回填 CM", async () => {
    const { container } = renderHtmlPanelWithMode("edit");
    // edit div 仅 ready 后渲染——loading 期间无 iframe 也无 CM 容器
    expect(container.querySelector("iframe")).toBeNull();
    // 等桥接渲染完成：useCodeMirror 以非 null 容器调用且 initialDoc = 磁盘快照
    await waitFor(() => {
      const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const call = cmCalls[cmCalls.length - 1]![0] as {
        container: unknown;
        initialDoc?: string;
      };
      expect(call.container).not.toBeNull();
      expect(call.initialDoc).toContain("<h1>Disk</h1>");
    }, { timeout: 3000 });
    // 面板层读盘一次（快照回填，CM 不自读盘——无重复读）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });
});
