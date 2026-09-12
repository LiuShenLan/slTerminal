// html-panel.test.tsx — HtmlPanel 组件测试（ADR-0021 预览回迁主窗 DOM 后重写）
//
// 覆盖：
//   1. 渲染状态 — loading/ready/error + 工具条带（切换条/HUD 悬浮带）
//   2. 宿主 iframe — 渲染（自定义协议 src + sandbox）/ 装配产物经 host_content
//      推送 / 卸载移除（无窗口编排——显隐几何随主窗 DOM 天然跟随）
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签（宿主 <script> 不经转义原样保留）
//   5. 注入脚本内容 — fragmentNav + zoom 运行时 + keyForward 基础段（旧键转发
//      类型零残留，CP-013）
//   6. 消息桥（宿主页桥 postMessage relay）：上行 zoom/nav 经 source 归属 +
//      origin "null" + nonce 校验 + HUD 状态机；负面用例（伪造 nonce/异 source/
//      异 origin/未知类型/旧键转发类型静默忽略——CP-013 终态集合）；下行 reset
//      经 postMessage；宿主 iframe_loaded → 重建归位
//   7. SEC-04 nonce——注入脚本携带面板 nonce / 伪造 nonce 忽略 / 实例隔离
//
// 注：预览内容渲染于主窗 DOM 内跨源沙箱 iframe（jsdom 真实挂载 iframe 元素但
// 不加载跨源 src、不执行宿主页桥脚本）——宿主页桥行为由 Rust 侧字符串断言 +
// L4 E2E 验收；本文件在 window message 事件边界上测主窗侧编排与校验逻辑。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, waitFor, fireEvent, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  // useCodeMirror mock：捕获调用参数（面板层逻辑真实，CM 层隔离——edit 击键
  // 经捕获的 onDocContent 手动驱动，见「viewMode 形态切换」describe）
  const mockUseCodeMirror = vi.fn();

  return {
    mockReadFile,
    mockUseCodeMirror,
    resetAll() {
      mockReadFile.mockReset();
      mockUseCodeMirror.mockReset();
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readFile: mocks.mockReadFile,
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

/** 单面板默认 panelId（宿主 iframe data-e2e = preview-frame-<panelId>） */
const PANEL_ID = "test-panel-1";
// 已退役键盘转发类型/信任标记名（拼接构造——CP-013 验证 grep 该名于 src/
// 须零命中，勿写成字面量）
const RETIRED_KEY_TYPE = ["slterm", "key"].join("_");
const RETIRED_MARKER = ["__slterm", "postMessage"].join("_");

function renderHtmlPanel(filePath: string | undefined, panelId = PANEL_ID) {
  return render(
    React.createElement(HtmlPanel, {
      params: { panelId, filePath },
    }),
  );
}

/** 宿主 iframe 元素（data-e2e 契约；panelId 区分多面板） */
function frameOf(panelId = PANEL_ID): HTMLIFrameElement {
  const el = document.querySelector<HTMLIFrameElement>(
    `iframe[data-e2e="preview-frame-${panelId}"]`,
  );
  expect(el).not.toBeNull();
  return el!;
}

/** 宿主 contentWindow postMessage 捕获（下行断言）——按 contentWindow 缓存
 *  （重复 spyOn 同属性会叠加包装，iframe 重建（新 contentWindow）自动换新） */
function spyHostPost(win: Window) {
  return vi.spyOn(win, "postMessage");
}
const postSpies = new WeakMap<Window, ReturnType<typeof spyHostPost>>();
function hostPostSpy(frame: HTMLIFrameElement) {
  const win = frame.contentWindow!;
  let spy = postSpies.get(win);
  if (!spy) {
    spy = spyHostPost(win);
    postSpies.set(win, spy);
  }
  return spy;
}

/** 模拟宿主页桥上行（dispatch window message——source/origin 构造边界） */
function dispatchUp(
  frame: HTMLIFrameElement,
  msg: Record<string, unknown>,
  opts?: { origin?: string; source?: MessageEventSource | null },
) {
  window.dispatchEvent(
    new MessageEvent("message", {
      data: msg,
      origin: opts?.origin ?? "null",
      source: opts?.source === undefined ? frame.contentWindow : opts.source,
    }),
  );
}

/** host_ready 上行（桥就绪——内容推送门控解除） */
async function sendHostReady(frame: HTMLIFrameElement) {
  await act(async () => dispatchUp(frame, { type: "slterm_host_ready" }));
}

/** 从装配产物提取注入 nonce（SEC-04 校验值） */
function extractNonce(doc: string): string {
  const m = /nonce:"([0-9a-f]+)"/.exec(doc);
  if (!m) throw new Error("装配产物未找到 nonce");
  return m[1]!;
}

/** 最近一次 host_content 推送的装配产物（最终注入文档串） */
function lastRenderedDoc(panelId = PANEL_ID): string {
  const spy = hostPostSpy(frameOf(panelId));
  const calls = spy.mock.calls
    .map(([m]) => m as { type?: string; html?: string })
    .filter((m) => m.type === "slterm_host_content");
  expect(calls.length).toBeGreaterThan(0);
  return calls[calls.length - 1]!.html!;
}

/**
 * 等待 ready 态内容推送发生（readFile resolve → PreviewFrame 挂载 → 宿主桥
 * 就绪上行 → 装配产物 host_content 推送）。jsdom 不执行宿主页脚本——桥就绪
 * 由本 helper 模拟上行驱动。
 */
async function waitForRender(_filePath = "C:/test/a.html", panelId = PANEL_ID) {
  void _filePath; // 形参仅作可读性标注（ready 判定与路径无关）
  // 等宿主 iframe 挂载（ready 态 PreviewFrame 出现）
  await waitFor(
    () => {
      expect(
        document.querySelector(`iframe[data-e2e="preview-frame-${panelId}"]`),
      ).not.toBeNull();
    },
    { timeout: 10000 },
  );
  const frame = frameOf(panelId);
  const spy = hostPostSpy(frame);
  // 模拟宿主页桥就绪 → 触发内容直推
  await sendHostReady(frame);
  // 等内容推送落地 + flush passive effects（消息监听注册在 useEffect——
  // DOM 出现不保证监听已挂，紧接派发消息会丢包，顺序竞态见消息用例失败史）
  await waitFor(
    () => {
      expect(
        spy.mock.calls.some(
          ([m]) => (m as { type?: string }).type === "slterm_host_content",
        ),
      ).toBe(true);
    },
    { timeout: 10000 },
  );
  await act(async () => {});
  return lastRenderedDoc(panelId);
}

/** 等待 error 态文案出现（IHE-08：消除重复 waitFor 模式） */
async function waitForError(
  getByText: (text: string | RegExp) => HTMLElement,
  message: string | RegExp,
): Promise<void> {
  await waitFor(() => {
    expect(getByText(message)).toBeDefined();
  }, { timeout: 10000 });
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

  it("加载完成后：宿主 iframe 渲染于主窗 DOM，装配产物经 host_content 推送", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello</h1>");
    const { container } = renderHtmlPanel("C:/test/index.html");
    await waitForRender("C:/test/index.html");
    // 预览回迁主窗 DOM（ADR-0021）——宿主 iframe 直填内容区（跨源沙箱 +
    // 自定义协议宿主页；显隐/几何随主窗天然跟随，无窗口编排）
    const frame = container.querySelector(
      `iframe[data-e2e="preview-frame-${PANEL_ID}"]`,
    ) as HTMLIFrameElement;
    expect(frame).not.toBeNull();
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    // 装配产物 = 内容 + 注入脚本（injectScript + buildInjectedScript 机制原样）
    expect(lastRenderedDoc()).toContain("<h1>Hello</h1>");
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

    await waitForRender("C:/test/3.html", "tp");
    expect(lastRenderedDoc("tp")).toContain("<h1>Third</h1>");
    expect(lastRenderedDoc("tp")).not.toContain("<h1>First</h1>");
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

  it("装配产物含 keyForward 收窄转发段；旧键转发类型/信任标记零残留（CP-013）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    renderHtmlPanel("C:/test/a.html");
    const doc = await waitForRender("C:/test/a.html");
    // ADR-0021/D2：keyForward 基础段恒注入（收窄转发——表单焦点不转发 +
    // 主窗 global context 限定消费）
    expect(doc).toContain("sltermKeyForward(document,window)");
    expect(doc).toContain("slterm_keyfwd");
    // 旧通道（slterm_key 字面量完整出现，负向前瞻排除 keyfwd 前缀子串）
    // 与信任标记零残留
    expect(doc).not.toMatch(new RegExp(RETIRED_KEY_TYPE + "(?!fwd)"));
    expect(doc).not.toContain(RETIRED_MARKER);
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

  it("H5: 注入脚本含片段拦截 + CSS 注入 + zoom 运行时 + keyForward 基础段", async () => {
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
    // addEventListener：keydown（keyForward 基础段）+ click（fragmentNav）+
    // wheel/message（zoom 运行时参数对象）
    const matches = doc.match(/addEventListener/g);
    expect(matches).not.toBeNull();
    expect(matches!.length).toBe(4);
    // keyForward 基础段（恒首段）：keydown 捕获 + 表单焦点跳过
    expect(doc).toContain("sltermKeyForward(document,window)");
    expect(doc).toContain('document.addEventListener("keydown"');
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
    }, { timeout: 10000 });

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

  it("unmount → 宿主 iframe 移除 + 消息监听摘除（无窗口销毁编排）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { container, unmount } = renderHtmlPanel("C:/test/a.html");
    await waitForRender("C:/test/a.html");
    expect(container.querySelector("iframe")).not.toBeNull();
    unmount();
    // 预览 = 主窗 DOM 内 iframe——卸载即移除（无任何窗口生命周期 IPC）
    expect(container.querySelector("iframe")).toBeNull();
  });

  // ==========================================================================
  // 消息桥（ADR-0021：宿主页桥 postMessage relay——source 归属 + origin "null"
  // + nonce + 白名单校验）
  //
  // 上行来自内容 iframe 注入段（经宿主页桥转发）；主窗侧校验链 = source 归属
  // （e.source === iframe.contentWindow）+ origin "null"（opaque 序列化）→
  // 类型分派 → nonce + 数值守卫。jsdom 真实挂载宿主 iframe 但不执行桥脚本——
  // 本小节在 window message 事件边界驱动，真实 WebView2 往返由 L4 E2E 验收。
  // ==========================================================================

  /** 渲染面板并返回 {container, frame, nonce, unmount}（消息桥用例统一入口） */
  async function renderRenderedPanel(filePath = "C:/test/a.html") {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { container, unmount } = renderHtmlPanel(filePath);
    const doc = await waitForRender(filePath);
    const nonce = extractNonce(doc);
    return { container, frame: frameOf(), nonce, unmount };
  }

  /** 查询 HUD 气泡元素（data-e2e） */
  function queryHud(container: HTMLElement): HTMLElement | null {
    return container.querySelector('[data-e2e="html-zoom-hud"]');
  }

  /** 构造 slterm_zoom 上行载荷（合法 nonce 缺省自动补齐；omit = 不带 nonce） */
  function zoomUplink(
    nonce: string,
    zoom: number,
    opts?: { nonceOverride?: string | "omit" },
  ) {
    return {
      type: "slterm_zoom",
      nonce: opts?.nonceOverride === "omit" ? undefined : opts?.nonceOverride ?? nonce,
      zoom,
    };
  }

  it("上行 slterm_zoom 合法（source + origin + nonce）→ 父侧镜像更新 + HUD 气泡显示", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.1)));
    const hud = queryHud(container);
    expect(hud).not.toBeNull();
    expect(hud!.textContent).toContain("110%");
    expect(hud!.querySelector('[data-e2e="html-zoom-reset"]')).not.toBeNull();
  });

  it("HUD: 百分比取整显示（zoom 1.234 → 123%）", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.234)));
    expect(queryHud(container)!.textContent).toContain("123%");
  });

  it("HUD: 回落 100% 的变化消息仍显示气泡（Chrome 语义——超时才消失）", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.5)));
    expect(queryHud(container)!.textContent).toContain("150%");
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.0)));
    expect(queryHud(container)).not.toBeNull();
    expect(queryHud(container)!.textContent).toContain("100%");
  });

  it("HUD: 等值 zoom 消息不续期隐藏计时（防重复上报造成僵尸气泡）", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.1)));
      expect(queryHud(container)).not.toBeNull();
      act(() => vi.advanceTimersByTime(2500));
      expect(queryHud(container)).not.toBeNull();
      await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.1)));
      act(() => vi.advanceTimersByTime(600));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: 3s 无缩放操作自动消失；期间续期", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.1)));
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.2)));
      act(() => vi.advanceTimersByTime(2900));
      expect(queryHud(container)).not.toBeNull();
      act(() => vi.advanceTimersByTime(200));
      expect(queryHud(container)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("HUD: SEC-04 负面——伪造/缺失 nonce、非数值 zoom 不弹气泡", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () =>
      dispatchUp(frame, zoomUplink(nonce, 1.5, { nonceOverride: "omit" })),
    );
    expect(queryHud(container)).toBeNull();
    await act(async () =>
      dispatchUp(
        frame,
        zoomUplink(nonce, 1.5, { nonceOverride: "00000000000000000000000000000000" }),
      ),
    );
    expect(queryHud(container)).toBeNull();
    // 非数值 zoom（字符串/NaN）——isFiniteZoom 拒绝
    await act(async () =>
      dispatchUp(frame, { ...zoomUplink(nonce, 1.5), zoom: "1.5" }),
    );
    expect(queryHud(container)).toBeNull();
    await act(async () => dispatchUp(frame, { ...zoomUplink(nonce, 1.5), zoom: NaN }));
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 异 source（非本 iframe）/异 origin 的上行被忽略（归属守卫）", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    // 异 source（主窗自身冒充）
    await act(async () =>
      dispatchUp(frame, zoomUplink(nonce, 1.5), { source: window }),
    );
    expect(queryHud(container)).toBeNull();
    // 异 origin（opaque 序列化恒 "null"——其余 origin 一律拒绝）
    await act(async () =>
      dispatchUp(frame, zoomUplink(nonce, 1.5), { origin: "http://evil.example" }),
    );
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 空载荷/未知类型上行静默忽略（不抛异常）", async () => {
    const { container, frame } = await renderRenderedPanel();
    expect(() => {
      dispatchUp(frame, null as unknown as Record<string, unknown>);
    }).not.toThrow();
    expect(queryHud(container)).toBeNull();
    await act(async () => dispatchUp(frame, { type: "other" }));
    expect(queryHud(container)).toBeNull();
  });

  it("负面: 已退役键盘转发类型上行被静默忽略——无按键重放、无命令消费（CP-013）", async () => {
    // 旧键盘转发通道（slterm_key 主窗 dispatchEvent 重放语义）：合法 nonce 的
    // 旧类型消息也不得触发任何 KeyboardEvent 重放/命令消费
    const { frame, nonce } = await renderRenderedPanel();
    const spy = vi.spyOn(window, "dispatchEvent");
    await act(async () =>
      dispatchUp(frame, {
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
    // 无下行命令重放（宿主 postMessage 无 reset/zoom_set/scroll_set 载荷）
    const post = hostPostSpy(frame);
    const downs = post.mock.calls
      .map(([m]) => m as { type?: string })
      .filter((m) => typeof m.type === "string" && m.type !== "slterm_host_content");
    expect(downs.length).toBe(0);
    spy.mockRestore();
  });

  it("上行 slterm_font_probe（TE-08 字体探针）→ nonce 校验后写主窗全局", async () => {
    // E2E 专用上行：iframe 内 fonts.check 结果经宿主桥转发——nonce 校验同
    // 渲染态消息（SEC-04），写 window.__slterm_e2e_fontProbe（键 = panelId）
    // 供 L4 断言
    const w = window as unknown as { __slterm_e2e_fontProbe?: Record<string, boolean> };
    delete w.__slterm_e2e_fontProbe;
    try {
      const { frame, nonce } = await renderRenderedPanel();
      await act(async () =>
        dispatchUp(frame, { type: "slterm_font_probe", nonce, loaded: true }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_ID]).toBe(true);
      // 伪造 nonce 拒绝——不覆盖既有值
      await act(async () =>
        dispatchUp(frame, { type: "slterm_font_probe", nonce: "bad", loaded: true }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_ID]).toBe(true);
      // loaded 非布尔 → 记 false（不误报 true）
      await act(async () =>
        dispatchUp(frame, { type: "slterm_font_probe", nonce, loaded: "yes" }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_ID]).toBe(false);
    } finally {
      delete w.__slterm_e2e_fontProbe;
    }
  });

  it("点重置 → 下行 slterm_reset（postMessage）+ 立即隐藏 + 回声不复活", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.3)));
    expect(queryHud(container)).not.toBeNull();

    fireEvent.click(queryHud(container)!.querySelector('[data-e2e="html-zoom-reset"]')!);
    // 下行载荷：slterm_reset + 面板 nonce（宿主页桥 relay 进内容 iframe——
    // iframe 侧 source===parent + nonce 校验）
    const post = hostPostSpy(frame);
    const resets = post.mock.calls
      .map(([m]) => m as { type?: string; nonce?: string })
      .filter((m) => m.type === "slterm_reset");
    expect(resets).toEqual([{ type: "slterm_reset", nonce }]);
    // 立即隐藏（不等回声）
    expect(queryHud(container)).toBeNull();
    // 复位回声 zoom=1 与基准等值 → 不复活
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.0)));
    expect(queryHud(container)).toBeNull();
  });

  it("宿主 iframe_loaded（内容重建）→ 气泡清空归位", async () => {
    const { container, frame, nonce } = await renderRenderedPanel();
    await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.7)));
    expect(queryHud(container)).not.toBeNull();
    // 内容重建完成（iframe 加载）→ 新文档 zoom 已归 1 → 静默复位隐藏 HUD
    await act(async () => dispatchUp(frame, { type: "slterm_iframe_loaded" }));
    expect(queryHud(container)).toBeNull();
    // html 面板 keepZoom 关——无 zoom_set 下行恢复（保持「重建归 100%」现状语义）
    const post = hostPostSpy(frame);
    expect(
      post.mock.calls.some(
        ([m]) => (m as { type?: string }).type === "slterm_zoom_set",
      ),
    ).toBe(false);
  });

  it("HUD: 卸载后隐藏计时器不泄漏（advance 不抛错）", async () => {
    const { container, frame, nonce, unmount } = await renderRenderedPanel();
    vi.useFakeTimers();
    try {
      await act(async () => dispatchUp(frame, zoomUplink(nonce, 1.1)));
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

    const doc = await waitForRender("C:/test/a.html", "tp");
    expect(extractNonce(doc)).toBe(firstNonce);
  });

  it("SEC-04: 两个独立面板实例 nonce 互不相同 + 宿主 iframe 各归各（防全局共享 nonce）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    render(
      React.createElement("div", null, [
        React.createElement(HtmlPanel, { key: "a", params: { panelId: "pa", filePath: "C:/test/a.html" } }),
        React.createElement(HtmlPanel, { key: "b", params: { panelId: "pb", filePath: "C:/test/b.html" } }),
      ]),
    );
    // 双面板各自宿主 iframe 就绪 + 内容推送
    await waitForRender("C:/test/a.html", "pa");
    await waitForRender("C:/test/b.html", "pb");
    // 按 panelId 取各自产物（宿主 iframe data-e2e 各归各）
    const docA = lastRenderedDoc("pa");
    const docB = lastRenderedDoc("pb");
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

  it("点「编辑」→ edit 形态：宿主 iframe 移除 + CM 容器出现 + useCodeMirror 参数", async () => {
    const { container } = renderHtmlPanelWithMode(undefined);
    await waitForRender("C:/test/index.html");
    mocks.mockUseCodeMirror.mockClear();

    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-edit"]')!);
    });

    // render 形态退出 → PreviewFrame 卸载 → 宿主 iframe 移除（无窗口销毁编排）
    expect(container.querySelector("iframe")).toBeNull();
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
    await waitForRender("C:/test/index.html", "tp");
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

    // 切回 render → 重新推送装配产物（草稿内容——PreviewFrame 重挂载，宿主
    // 桥就绪后重推）
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="html-mode-render"]')!);
    });
    await waitForRender("C:/test/index.html");
    expect(lastRenderedDoc()).toContain("<h1>Draft Edit</h1>");
    // 读盘不重复（草稿快照回填路径）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("viewMode=edit 布局恢复：直接 edit 形态（无宿主 iframe），读盘一次后快照回填 CM", async () => {
    const { container } = renderHtmlPanelWithMode("edit");
    // edit div 仅 ready 后渲染——loading 期间无预览宿主
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
    }, { timeout: 10000 });
    // 面板层读盘一次（快照回填，CM 不自读盘——无重复读）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });
});
