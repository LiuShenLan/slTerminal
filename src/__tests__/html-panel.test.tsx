// html-panel.test.tsx — HtmlPanel 组件测试（S10-② 预览迁独立 webview 后重写）
//
// 覆盖：
//   1. 渲染状态 — loading/ready/error + 工具条带（切换条/HUD 悬浮带）
//   2. 预览窗口编排 — previewSync（几何/显隐）/ previewRender（装配产物推送）/
//      previewClose（卸载销毁）/ label = preview-<panelId>
//      （SEC-03：sync 失败一次性告警 + 成功复位；close 失败直告警）
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签（宿主 <script> 不经转义原样保留）
//   5. 注入脚本内容 — fragmentNav + zoom 运行时（keydown 转发段退役，旧类型名
//      零残留，CP-013）
//   6. 消息桥（Tauri event 中继）：上行 zoom/nav 按 label 过滤 + nonce 校验 +
//      HUD 状态机；负面用例（伪造 nonce/异 label/异类型/旧键转发类型静默忽略
//      ——CP-013 终态集合）；下行 reset 经 emitPreviewDownlink；宿主
//      iframe-loaded 状态 → 重建归位
//   7. SEC-04 nonce——注入脚本携带面板 nonce / 伪造 nonce 忽略 / 实例隔离
//
// 注：预览内容现渲染于独立 WebviewWindow（jsdom 无窗口）——真实 WebView2 往返
// （窗口创建/宿主页桥/iframe 执行）由 L4 E2E 验收；本文件在 ipc/preview mock
// 边界上测主窗侧编排与校验逻辑。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, waitFor, fireEvent, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  // useCodeMirror mock：捕获调用参数（面板层逻辑真实，CM 层隔离——edit 击键
  // 经捕获的 onDocContent 手动驱动，见「viewMode 形态切换」describe）
  const mockUseCodeMirror = vi.fn();

  // 预览窗口编排 mock：捕获主窗侧对预览窗口的全部调用 + 事件订阅回调
  const previewSync = vi.fn<(..._a: unknown[]) => Promise<void>>(() => Promise.resolve());
  const previewClose = vi.fn<(..._a: unknown[]) => Promise<void>>(() => Promise.resolve());
  const previewRender = vi.fn<(..._a: unknown[]) => Promise<void>>(() => Promise.resolve());
  const emitPreviewDownlink = vi.fn<(..._a: unknown[]) => Promise<void>>(() => Promise.resolve());
  const uplinkHandlers: Array<(msg: Record<string, unknown>) => void> = [];
  const statusHandlers: Array<(msg: Record<string, unknown>) => void> = [];
  const onPreviewUplink = vi.fn((cb: (msg: Record<string, unknown>) => void) => {
    uplinkHandlers.push(cb);
    return () => {};
  });
  const onPreviewHostStatus = vi.fn((cb: (msg: Record<string, unknown>) => void) => {
    statusHandlers.push(cb);
    return () => {};
  });
  // 主窗移动订阅回调捕获（SEC-03 用例：几何变化经移动事件即时触发下一轮 sync）
  const movedHandlers: Array<() => void> = [];
  const onMainWindowMoved = vi.fn((cb: () => void) => {
    movedHandlers.push(cb);
    return () => {};
  });
  return {
    mockReadFile,
    mockUseCodeMirror,
    previewSync,
    previewClose,
    previewRender,
    emitPreviewDownlink,
    onPreviewUplink,
    onPreviewHostStatus,
    onMainWindowMoved,
    uplinkHandlers,
    statusHandlers,
    movedHandlers,
    resetAll() {
      mockReadFile.mockReset();
      mockUseCodeMirror.mockReset();
      previewSync.mockReset();
      previewClose.mockReset();
      previewRender.mockReset();
      emitPreviewDownlink.mockReset();
      onPreviewUplink.mockReset();
      onPreviewHostStatus.mockReset();
      onMainWindowMoved.mockReset();
      uplinkHandlers.length = 0;
      statusHandlers.length = 0;
      movedHandlers.length = 0;
      previewSync.mockResolvedValue(undefined);
      previewClose.mockResolvedValue(undefined);
      previewRender.mockResolvedValue(undefined);
      emitPreviewDownlink.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readFile: mocks.mockReadFile,
}));

// S10-②：预览窗口编排/消息桥 mock（PreviewFrame 经 src/ipc/preview 与 src/ipc/window）
vi.mock("../ipc/preview", () => ({
  makePreviewLabel: (panelId: string) => `preview-${panelId}`,
  previewSync: (...a: unknown[]) => mocks.previewSync(...a),
  previewClose: (...a: unknown[]) => mocks.previewClose(...a),
  previewRender: (...a: unknown[]) => mocks.previewRender(...a),
  emitPreviewDownlink: (...a: unknown[]) => mocks.emitPreviewDownlink(...a),
  onPreviewUplink: (cb: (msg: Record<string, unknown>) => void) => mocks.onPreviewUplink(cb),
  onPreviewHostStatus: (cb: (msg: Record<string, unknown>) => void) =>
    mocks.onPreviewHostStatus(cb),
}));

vi.mock("../ipc/window", () => ({
  onMainWindowMoved: (cb: () => void) => mocks.onMainWindowMoved(cb),
  onMainWindowResized: (cb: () => void) => mocks.onMainWindowMoved(cb),
  onMainWindowScaleChanged: (cb: () => void) => mocks.onMainWindowMoved(cb),
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

/** 单面板默认 label（panelId test-panel-1 → preview-test-panel-1） */
const PANEL_LABEL = "preview-test-panel-1";
// 已退役键盘转发类型/信任标记名（拼接构造——CP-013 验证 grep 该名于 src/
// 须零命中，勿写成字面量）
const RETIRED_KEY_TYPE = ["slterm", "key"].join("_");
const RETIRED_MARKER = ["__slterm", "postMessage"].join("_");

function renderHtmlPanel(filePath: string | undefined, panelId = "test-panel-1") {
  return render(
    React.createElement(HtmlPanel, {
      params: { panelId, filePath },
    }),
  );
}

/** 最近一次 previewRender 推送的装配产物（最终注入文档串） */
function lastRenderedDoc(): string {
  const calls = mocks.previewRender.mock.calls;
  expect(calls.length).toBeGreaterThan(0);
  return calls[calls.length - 1]![1] as string;
}

/**
 * 等待 ready 态内容推送发生（readFile resolve → PreviewFrame 挂载 → 推送装配
 * 产物）。jsdom 无真实预览窗口——ready 判定 = 内容推送已完成 + 事件订阅已挂。
 */
async function waitForRender(_filePath = "C:/test/a.html") {
  void _filePath; // 形参仅作可读性标注（ready 判定与路径无关）
  await waitFor(
    () => {
      expect(mocks.previewRender).toHaveBeenCalled();
    },
    { timeout: 3000 },
  );
  // flush passive effects（PreviewFrame 事件订阅注册在 useEffect）——DOM 出现
  // 不保证订阅已挂载，紧接派发消息会丢包（顺序竞态，见消息用例失败史）
  await act(async () => {});
  return lastRenderedDoc();
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

/** 从装配产物提取注入脚本拼入的 nonce（SEC-04 校验值） */
function extractNonce(doc: string): string {
  const m = /nonce:"([0-9a-f]+)"/.exec(doc);
  if (!m) throw new Error("装配产物未找到 nonce——注入脚本结构已变，测试需同步");
  return m[1]!;
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

  it("加载完成后：主窗口无 iframe，推送装配产物至预览窗口", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello</h1>");
    const { container } = renderHtmlPanel("C:/test/index.html");
    await waitForRender("C:/test/index.html");
    // 渲染内容迁独立 webview——主窗口不再有 iframe/srcdoc（S10-②）
    expect(container.querySelector("iframe")).toBeNull();
    // 装配产物 = 内容 + 注入脚本（injectScript + buildInjectedScript 机制原样）
    expect(lastRenderedDoc()).toContain("<h1>Hello</h1>");
    // label = preview-<panelId>（WDIO 驱动句柄契约）
    expect(mocks.previewRender.mock.calls[0]![0]).toBe(PANEL_LABEL);
  });

  it("几何编排：previewSync 以面板 label + CSS 视口矩形驱动", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/index.html");
    await waitForRender("C:/test/index.html");
    expect(mocks.previewSync).toHaveBeenCalled();
    const args = mocks.previewSync.mock.calls[0]!;
    expect(args[0]).toBe(PANEL_LABEL);
    // jsdom 无布局：矩形为 0 → visible=false（不可见不建窗——宿主加载兜底）
    expect(args[5]).toBe(false);
  });

  it('加载完成后"加载中"文字消失', async () => {
    mocks.mockReadFile.mockResolvedValue("<p>done</p>");
    const { queryByText } = renderHtmlPanel("C:/test/index.html");
    await waitForRender("C:/test/index.html");
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

  it("ready 后工具条带承载切换条/HUD（悬浮区坐标协调单点语义）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { container } = renderHtmlPanel("C:/test/index.html");
    await waitForRender("C:/test/index.html");
    // 工具条带（切换条悬浮带——不落入预览窗口覆盖范围，交互可用）
    const switcher = container.querySelector('[data-e2e="html-mode-switcher"]');
    expect(switcher).not.toBeNull();
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

    const { rerender } = renderHtmlPanel("C:/test/old.html");

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "test-panel-1", filePath: "C:/test/new.html" },
      }),
    );

    resolveOld("<h1>Old Content</h1>");

    await waitForRender("C:/test/new.html");
    expect(lastRenderedDoc()).toContain("<h1>New Content</h1>");
    expect(lastRenderedDoc()).not.toContain("<h1>Old Content</h1>");
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

    const { rerender } = renderHtmlPanel("C:/test/1.html");

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

    await waitForRender("C:/test/3.html");
    expect(lastRenderedDoc()).toContain("<h1>Third</h1>");
    expect(lastRenderedDoc()).not.toContain("<h1>First</h1>");
  });

  // ==========================================================================
  // 边界
  // ==========================================================================

  it("空 HTML 文件渲染含注入脚本的完整文档", async () => {
    mocks.mockReadFile.mockResolvedValue("");
    renderHtmlPanel("C:/test/empty.html");
    await waitForRender("C:/test/empty.html");
    // 空 HTML 经 injectScript 后变成最小完整文档
    expect(lastRenderedDoc()).toContain("<html>");
  });

  it("很大 HTML 内容正常推送", async () => {
    const bigContent = "<html><body>" + "x".repeat(100_000) + "</body></html>";
    mocks.mockReadFile.mockResolvedValue(bigContent);
    renderHtmlPanel("C:/test/big.html");
    await waitForRender("C:/test/big.html");
    expect(lastRenderedDoc()).toContain("x".repeat(100_000));
  });

  it("HTML 含 script 标签——宿主脚本原样保留（不经字符串级转义，CP-031）", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "<html><body><script>document.body.innerHTML='JS OK'</script></body></html>",
    );
    renderHtmlPanel("C:/test/script.html");
    const doc = await waitForRender("C:/test/script.html");
    // 宿主自带 </script> 不再被转义为 <\/script>（存量转义函数消亡）——
    // 新预览域（自定义协议宿主页，CSP meta 放行内联脚本）内宿主脚本真实可执行
    expect(doc).toContain("<script>document.body.innerHTML='JS OK'</script>");
    expect(doc).not.toContain("<\\/script>");
    // 注入脚本（zoom 运行时）仍就位
    expect(doc).toContain("sltermZoom(document,window)");
  });

  // ==========================================================================
  // 注入脚本内容验证（fragmentNav 片段拦截 + zoom 运行时——keydown 转发段已退役）
  // ==========================================================================

  it("装配产物不再含键盘转发（旧类型/信任标记零残留，CP-013）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).not.toContain(RETIRED_KEY_TYPE);
    expect(doc).not.toContain(RETIRED_MARKER);
    expect(doc).not.toContain('addEventListener("keydown"');
    expect(doc).not.toContain("fingerprint");
  });

  it("注入脚本含片段链接拦截 scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("scrollIntoView");
  });

  it("H1: 注入脚本含 scrollIntoView + class-based toggle", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("scrollIntoView");
    expect(doc).toContain("classList.add");
    expect(doc).toContain("classList.remove");
  });

  it("H2: 注入脚本含 closest('a') 链接检测", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("closest");
    expect(doc).toContain('"a"');
  });

  it("H3: 片段链接拦截使用 preventDefault", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("preventDefault");
  });

  it("H4: 注入脚本含 dataset.sltermHash 状态跟踪", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("dataset.sltermHash");
  });

  it("H5: 注入脚本含片段拦截 + CSS 注入 + zoom 运行时（无 keydown 段）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    // 片段拦截 + toggle
    expect(doc).toContain("scrollIntoView");
    expect(doc).toContain("classList.add");
    expect(doc).toContain("classList.remove");
    // CSS 注入
    expect(doc).toContain("createElement");
    expect(doc).toContain("slterm-target");
    // addEventListener：click（fragmentNav）+ wheel/message（zoom 运行时参数对象）
    const matches = doc.match(/addEventListener/g);
    expect(matches).not.toBeNull();
    expect(matches!.length).toBe(3);
    // zoom 运行时（恒末位）：wheel 捕获 + 下行 message 监听 + 挂载调用
    expect(doc).toContain('doc.addEventListener("wheel"');
    expect(doc).toContain('win.addEventListener("message"');
    expect(doc).toContain("sltermZoom(document,window)");
    expect(doc).toContain("slterm_zoom");
    expect(doc).toContain("slterm_reset");
  });

  it("H6: 注入后原始可见内容保留", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello World</h1>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toContain("<h1>Hello World</h1>");
  });

  it("H7: 注入脚本整段可被 JS 解析（防拼接边界 SyntaxError 类回归）", async () => {
    // 2026-09-06 实证：click 段原为 script 末语句无分号，追加 zoom 段后同串拼接
    // 无分隔 → 整段注入脚本 SyntaxError——L2 全绿、仅 L4 真实 iframe 执行暴露。
    // 本用例把注入脚本体经 new Function 做 parse-only 校验，锁死拼接边界。
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    const scriptBody = /<script>([\s\S]*?)<\/script>/.exec(doc)?.[1];
    expect(scriptBody).toBeDefined();
    expect(() => new Function(scriptBody!)).not.toThrow();
  });

  // ==========================================================================
  // 注入脚本关键控制流（IHE-07②：非仅字符串包含）
  //
  // 以上 H1-H6 只做关键词存在断言；本小节用正则断言脚本的控制流结构——
  // 监听绑定（capture phase）、守卫 → preventDefault 顺序。jsdom 不执行
  // srcdoc iframe 内脚本，真实执行由 L4 E2E 验收。
  // ==========================================================================

  it("控制流: click capture 监听 + closest/href 守卫 → preventDefault → scrollIntoView", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
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
  // 预览窗口生命周期（renderer="always" 语义）
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
  });

  it("unmount → 销毁预览窗口（previewClose + label）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { unmount } = renderHtmlPanel("C:/test/a.html");
    await waitForRender("C:/test/a.html");
    expect(mocks.previewClose).not.toHaveBeenCalled();
    unmount();
    expect(mocks.previewClose).toHaveBeenCalledTimes(1);
    expect(mocks.previewClose.mock.calls[0]![0]).toBe(PANEL_LABEL);
  });

  // ==========================================================================
  // SEC-03：预览窗口命令 catch 可观测化（sync 一次性告警 + 成功复位；close 直告警）
  // ==========================================================================

  /** 触发主窗移动即时同步（50ms 节流定时器 → 等 60ms 越过节流窗口落地） */
  async function triggerMovedSync() {
    await act(async () => {
      for (const h of mocks.movedHandlers) h();
      await new Promise((r) => setTimeout(r, 60));
    });
  }

  it("previewSync 失败一次性告警：连续两轮只 warn 一次；成功复位后再失败再 warn", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // 几何可变桩：轮询/移动触发的下一轮读到新矩形才真正再发 previewSync
    // （等值早退分支不重复发——亚像素抖动抑制语义不变）
    let rect = { x: 10, y: 20, width: 300, height: 200 };
    const rectSpy = vi
      .spyOn(Element.prototype, "getBoundingClientRect")
      .mockImplementation(
        () =>
          ({
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
            toJSON: () => ({}),
          }) as DOMRect,
      );
    try {
      mocks.mockReadFile.mockResolvedValue("<p>test</p>");
      mocks.previewSync.mockRejectedValue(new Error("窗口域异常"));
      renderHtmlPanel("C:/test/a.html");
      await waitForRender("C:/test/a.html");
      // 挂载即 sync 一轮 → 失败 → 告警一次
      expect(mocks.previewSync.mock.calls.length).toBeGreaterThanOrEqual(1);
      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(String(warnSpy.mock.calls[0]![0])).toContain("previewSync 失败");

      // 第二轮失败：几何变化触发再 sync → 旗标已置位，不重复告警
      rect = { x: 10, y: 20, width: 320, height: 200 };
      const callsBefore2 = mocks.previewSync.mock.calls.length;
      await triggerMovedSync();
      expect(mocks.previewSync.mock.calls.length).toBeGreaterThan(callsBefore2);
      expect(warnSpy).toHaveBeenCalledTimes(1);

      // 成功一轮 → 旗标复位
      mocks.previewSync.mockResolvedValue(undefined);
      rect = { x: 10, y: 20, width: 340, height: 200 };
      const callsBefore3 = mocks.previewSync.mock.calls.length;
      await triggerMovedSync();
      expect(mocks.previewSync.mock.calls.length).toBeGreaterThan(callsBefore3);

      // 再失败 → 重新告警一次（累计 2 次）
      mocks.previewSync.mockRejectedValue(new Error("窗口域异常"));
      rect = { x: 10, y: 20, width: 360, height: 200 };
      const callsBefore4 = mocks.previewSync.mock.calls.length;
      await triggerMovedSync();
      expect(mocks.previewSync.mock.calls.length).toBeGreaterThan(callsBefore4);
      expect(warnSpy).toHaveBeenCalledTimes(2);
    } finally {
      rectSpy.mockRestore();
      warnSpy.mockRestore();
    }
  });

  it("previewClose 失败 → console.warn（窗口可能残留可观测，一次性事件）", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      mocks.mockReadFile.mockResolvedValue("<p>test</p>");
      const { unmount } = renderHtmlPanel("C:/test/a.html");
      await waitForRender("C:/test/a.html");
      mocks.previewClose.mockRejectedValue(new Error("窗口不存在"));
      unmount();
      await act(async () => {});
      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(String(warnSpy.mock.calls[0]![0])).toContain("previewClose 失败");
      expect(warnSpy.mock.calls[0]![1]).toBe(PANEL_LABEL);
    } finally {
      warnSpy.mockRestore();
    }
  });

  // ==========================================================================
  // 消息桥（S10-②：Tauri event 中继——label 归属 + nonce + 白名单校验）
  //
  // 上行来自预览窗口宿主页桥（iframe 文档消息转发）；主窗侧校验链 = 旧 iframe
  // 通道同款（label 归属替代 origin/source——事件通道无跨窗口 source 语义）。
  // jsdom 无真实窗口/宿主页——本小节在 ipc/preview 事件订阅 mock 边界上驱动
  // 上行回调，真实 WebView2 往返由 L4 E2E 验收。
  // ==========================================================================

  /** 向上行处理器派发消息（模拟宿主页桥转发） */
  function dispatchUplink(msg: Record<string, unknown>) {
    for (const h of mocks.uplinkHandlers) {
      h(msg);
    }
  }

  /** 向宿主状态处理器派发消息（模拟宿主页 iframe 加载完成） */
  function dispatchHostStatus(status: string, label = PANEL_LABEL) {
    for (const h of mocks.statusHandlers) {
      h({ label, status });
    }
  }

  /** 渲染面板并返回 {container, nonce, unmount}（消息桥用例统一入口） */
  async function renderRenderedPanel(filePath = "C:/test/a.html") {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { container, unmount } = renderHtmlPanel(filePath);
    const doc = await waitForRender(filePath);
    const nonce = extractNonce(doc);
    return { container, nonce, unmount };
  }

  /** 查询 HUD 气泡元素（data-e2e） */
  function queryHud(container: HTMLElement): HTMLElement | null {
    return container.querySelector('[data-e2e="html-zoom-hud"]');
  }

  /** 构造 slterm_zoom 上行（合法 nonce 缺省自动补齐；omit = 不带 nonce） */
  function zoomUplink(
    nonce: string,
    zoom: number,
    opts?: { label?: string; nonceOverride?: string | "omit" },
  ) {
    return {
      label: opts?.label ?? PANEL_LABEL,
      type: "slterm_zoom",
      nonce: opts?.nonceOverride === "omit" ? undefined : opts?.nonceOverride ?? nonce,
      zoom,
    };
  }

  it("上行 slterm_zoom 合法（label + nonce）→ 父侧镜像更新 + HUD 气泡显示", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.1)));
    const hud = queryHud(container);
    expect(hud).not.toBeNull();
    expect(hud!.textContent).toContain("110%");
    expect(hud!.querySelector('[data-e2e="html-zoom-reset"]')).not.toBeNull();
  });

  it("HUD: 百分比取整显示（zoom 1.234 → 123%）", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.234)));
    expect(queryHud(container)!.textContent).toContain("123%");
  });

  it("HUD: 回落 100% 的变化消息仍显示气泡（Chrome 语义——超时才消失）", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.5)));
    expect(queryHud(container)!.textContent).toContain("150%");
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.0)));
    expect(queryHud(container)).not.toBeNull();
    expect(queryHud(container)!.textContent).toContain("100%");
  });

  it("HUD: 等值 zoom 消息不续期隐藏计时（防重复上报造成僵尸气泡）", async () => {
    const { container, nonce } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUplink(zoomUplink(nonce, 1.1)));
      expect(queryHud(container)).not.toBeNull();
      act(() => vi.advanceTimersByTime(2500));
      expect(queryHud(container)).not.toBeNull();
      await act(async () => dispatchUplink(zoomUplink(nonce, 1.1)));
      act(() => vi.advanceTimersByTime(600));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: 3s 无缩放操作自动消失；期间续期", async () => {
    const { container, nonce } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUplink(zoomUplink(nonce, 1.1)));
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      await act(async () => dispatchUplink(zoomUplink(nonce, 1.2)));
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      act(() => vi.advanceTimersByTime(200));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: SEC-04 负面——伪造/缺失 nonce、非数值 zoom 不弹气泡", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () =>
      dispatchUplink(zoomUplink(nonce, 1.5, { nonceOverride: "omit" })),
    );
    expect(queryHud(container)).toBeNull();
    await act(async () =>
      dispatchUplink(
        zoomUplink(nonce, 1.5, { nonceOverride: "00000000000000000000000000000000" }),
      ),
    );
    expect(queryHud(container)).toBeNull();
    // 非数值 zoom（字符串/NaN）——isFiniteZoom 拒绝
    await act(async () =>
      dispatchUplink({ ...zoomUplink(nonce, 1.5), zoom: "1.5" }),
    );
    expect(queryHud(container)).toBeNull();
    await act(async () => dispatchUplink({ ...zoomUplink(nonce, 1.5), zoom: NaN }));
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 异面板 label 的上行被忽略（label 归属守卫替代旧 origin/source 校验）", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () =>
      dispatchUplink(zoomUplink(nonce, 1.5, { label: "preview-other-panel" })),
    );
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 空载荷/未知类型上行静默忽略（不抛异常）", async () => {
    const { container } = await renderRenderedPanel();
    expect(() => {
      dispatchUplink(null as unknown as Record<string, unknown>);
    }).not.toThrow();
    expect(queryHud(container)).toBeNull();
    await act(async () => dispatchUplink({ label: PANEL_LABEL, type: "other" }));
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 已退役键盘转发类型上行被静默忽略——无按键重放、无命令消费（CP-013）", async () => {
    // 旧键盘转发通道：合法 nonce 的键转发消息也不得触发任何 KeyboardEvent/
    // 关页签（上行终态集合 = 渲染态白名单）
    const { nonce } = await renderRenderedPanel();
    const spy = vi.spyOn(window, "dispatchEvent");
    await act(async () =>
      dispatchUplink({
        label: PANEL_LABEL,
        type: RETIRED_KEY_TYPE,
        nonce,
        fingerprint: "Ctrl+KeyW",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
        code: "KeyW",
        key: "w",
      }),
    );
    await new Promise((r) => setTimeout(r, 10));
    const kbEvents = spy.mock.calls.filter(([e]) => e instanceof KeyboardEvent);
    expect(kbEvents.length).toBe(0);
    // 无下行命令重放
    expect(mocks.emitPreviewDownlink).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("上行 slterm_font_probe（TE-08 字体探针）→ nonce 校验后写主窗全局", async () => {
    // E2E 专用上行：iframe 内 fonts.check 结果经宿主桥转发——nonce 校验同
    // 渲染态消息（SEC-04），写 window.__slterm_e2e_fontProbe 供 L4 断言
    const w = window as unknown as { __slterm_e2e_fontProbe?: Record<string, boolean> };
    delete w.__slterm_e2e_fontProbe;
    try {
      const { nonce } = await renderRenderedPanel();
      await act(async () =>
        dispatchUplink({ label: PANEL_LABEL, type: "slterm_font_probe", nonce, loaded: true }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_LABEL]).toBe(true);
      // 伪造 nonce 拒绝——不覆盖既有值
      await act(async () =>
        dispatchUplink({ label: PANEL_LABEL, type: "slterm_font_probe", nonce: "bad", loaded: true }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_LABEL]).toBe(true);
      // loaded 非布尔 → 记 false（不误报 true）
      await act(async () =>
        dispatchUplink({ label: PANEL_LABEL, type: "slterm_font_probe", nonce, loaded: "yes" }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_LABEL]).toBe(false);
    } finally {
      delete w.__slterm_e2e_fontProbe;
    }
  });

  it("点重置 → 下行 slterm_reset（emitPreviewDownlink）+ 立即隐藏 + 回声不复活", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.3)));
    expect(queryHud(container)).not.toBeNull();

    fireEvent.click(queryHud(container)!.querySelector('[data-e2e="html-zoom-reset"]')!);
    // 下行载荷：slterm_reset + 面板 nonce（事件通道经宿主页桥注入 iframe，
    // iframe 侧 source===parent + nonce 校验——语义与旧 postMessage 通道一致）
    expect(mocks.emitPreviewDownlink).toHaveBeenCalledTimes(1);
    expect(mocks.emitPreviewDownlink.mock.calls[0]![0]).toEqual({
      label: PANEL_LABEL,
      type: "slterm_reset",
      nonce,
    });
    // 立即隐藏（不等回声）
    expect(queryHud(container)).toBeNull();
    // 复位回声 zoom=1 与基准等值 → 不复活
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.0)));
    expect(queryHud(container)).toBeNull();
  });

  it("宿主状态 iframe-loaded（内容重建）→ 气泡清空归位", async () => {
    const { container, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUplink(zoomUplink(nonce, 1.7)));
    expect(queryHud(container)).not.toBeNull();
    // 内容重建完成（iframe 加载）→ 新文档 zoom 已归 1 → 静默复位隐藏 HUD
    await act(async () => dispatchHostStatus("iframe-loaded"));
    expect(queryHud(container)).toBeNull();
    // html 面板 keepZoom 关——无 zoom_set 下行恢复（保持「重建归 100%」现状语义）
    expect(mocks.emitPreviewDownlink).not.toHaveBeenCalled();
  });

  it("HUD: 卸载后隐藏计时器不泄漏（advance 不抛错）", async () => {
    const { container, nonce, unmount } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUplink(zoomUplink(nonce, 1.1)));
      expect(queryHud(container)).not.toBeNull();
      unmount();
      expect(() => act(() => vi.advanceTimersByTime(5000))).not.toThrow();
    } finally {
      vi.useRealTimers();
    }
  });

  // ==========================================================================
  // SEC-04: nonce 校验（面板挂载期随机值——装配产物拼入，事件上行校验一致）
  // ==========================================================================

  it("SEC-04: 装配产物 zoom/scroll/nav 消息携带面板 nonce", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    expect(doc).toMatch(/type:"slterm_zoom",nonce:"[0-9a-f]{32}"/);
  });

  it("SEC-04: 同面板重渲染 nonce 稳定（注入产物与校验值不漂移）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { rerender } = renderHtmlPanel("C:/test/a.html");
    const firstNonce = extractNonce(await waitForRender("C:/test/a.html"));

    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/a.html" },
      }),
    );

    const doc = await waitForRender("C:/test/a.html");
    expect(extractNonce(doc)).toBe(firstNonce);
  });

  it("SEC-04: 两个独立面板实例 nonce 互不相同 + label 各归各（防全局共享 nonce）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    render(
      React.createElement("div", null, [
        React.createElement(HtmlPanel, { key: "a", params: { panelId: "pa", filePath: "C:/test/a.html" } }),
        React.createElement(HtmlPanel, { key: "b", params: { panelId: "pb", filePath: "C:/test/b.html" } }),
      ]),
    );
    await waitFor(() => {
      expect(mocks.previewRender.mock.calls.length).toBeGreaterThanOrEqual(2);
    }, { timeout: 3000 });
    // 按 label 分组取各自产物
    const renderArgs = mocks.previewRender.mock.calls.map((c) => [c[0], c[1]] as const);
    const docA = renderArgs.find(([l]) => l === "preview-pa")![1] as string;
    const docB = renderArgs.find(([l]) => l === "preview-pb")![1] as string;
    const nonceA = extractNonce(docA);
    const nonceB = extractNonce(docB);
    expect(nonceA).toMatch(/^[0-9a-f]{32}$/);
    expect(nonceB).toMatch(/^[0-9a-f]{32}$/);
    expect(nonceA).not.toBe(nonceB);
  });
});

// ═══════════════════════════════════════════════════════════════════
// S4: viewMode 形态切换（render ↔ edit）+ 草稿快照往返
//
// CM 实现经 mock 隔离；面板层逻辑真实：
//   - useCodeMirror 调用参数（container 随形态 null/div、initialDoc 快照回填、
//     gitGutterEnabled=false）
//   - edit 击键经捕获的 onDocContent 手动驱动 → 断言 render 态产物展示草稿
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

  it("默认形态 render：推送装配产物 + 工具条带切换条（渲染/编辑 两钮）", async () => {
    const { container } = renderHtmlPanelWithMode(undefined);
    await waitForRender("C:/test/index.html");
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
    const { container } = renderHtmlPanelWithMode("preview" as never);
    await waitForRender("C:/test/index.html");
    expect(container.querySelector('[data-e2e="html-mode-render"]')).not.toBeNull();
  });

  it("点「编辑」→ edit 形态：预览窗口销毁 + CM 容器出现 + useCodeMirror 参数", async () => {
    const { container } = renderHtmlPanelWithMode(undefined);
    await waitForRender("C:/test/index.html");
    mocks.mockUseCodeMirror.mockClear();
    mocks.previewClose.mockClear();

    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });

    // render 形态退出 → PreviewFrame 卸载 → 预览窗口销毁
    expect(mocks.previewClose).toHaveBeenCalledTimes(1);
    expect(mocks.previewClose.mock.calls[0]![0]).toBe(PANEL_LABEL);
    // edit 形态渲染 CM 容器 div（无主窗口 iframe/锚点）
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
    const { container } = renderHtmlPanelWithMode(undefined);
    await waitForRender("C:/test/index.html");
    mocks.mockUseCodeMirror.mockClear();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const call = cmCalls[cmCalls.length - 1]![0] as {
      fontSize?: number;
      onFontSizeChange?: (size: number) => void;
    };
    expect(call.fontSize).toBe(14);
    expect(typeof call.onFontSizeChange).toBe("function");
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
    const { container } = render(
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
    await waitForRender("C:/test/index.html");
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });
    expect(updateParameters).toHaveBeenCalledWith(
      expect.objectContaining({ viewMode: "edit" }),
    );
  });

  it("edit 击键草稿 → 切回 render → 产物展示草稿（非磁盘内容）", async () => {
    const { container } = renderHtmlPanelWithMode(undefined);
    await waitForRender("C:/test/index.html");

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

    // 切回 render → 重新推送装配产物（草稿内容）
    mocks.previewRender.mockClear();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-render"]')!);
    });
    await waitForRender("C:/test/index.html");
    expect(lastRenderedDoc()).toContain("<h1>Draft Edit</h1>");
    // 读盘不重复（草稿快照回填路径）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("viewMode=edit 布局恢复：直接 edit 形态（无预览窗口），读盘一次后快照回填 CM", async () => {
    renderHtmlPanelWithMode("edit");
    // edit div 仅 ready 后渲染——loading 期间无预览编排
    expect(mocks.previewSync).not.toHaveBeenCalled();
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
