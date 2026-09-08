// markdown-panel.test.tsx — MarkdownPanel 三形态编排器测试
// （S10-② 预览迁独立 webview 后重写——渲染内容经 previewRender 推送，
//  iframe/srcdoc 断言改为装配产物捕获断言）
//
// 面板层逻辑真实（mock 外围：CM 桥/渲染管线资源/mermaid/allotment 布局）：
//   1. 默认 edit + 工具条带切换条三态
//   2. edit ↔ split ↔ preview 形态切换与布局（CP-037：CM 恒挂载、preview 态
//      visible=false 隐藏保活——undo/光标跨形态保留）
//   3. 草稿防抖渲染（fake timers 300ms）与切形态 stale 立即渲染
//   4. viewMode/splitRatio params 恢复与非法回退
//   5. 链接 slterm_nav 上行 → external 系统浏览器 / local 应用内打开
//   6. 形态切换持久化 persistPanelParams
//   7. keepZoom/keepScrollRatio：iframe-loaded 状态事件后下行恢复
//
// 注：渲染内容现于独立 WebviewWindow（jsdom 无窗口）——真实 WebView2 往返由
// L4 E2E 验收；本文件在 ipc/preview mock 边界上测主窗侧编排与消息桥。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import {
  render,
  cleanup,
  waitFor,
  fireEvent,
  act,
} from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  const mockReadResource = vi.fn();
  const mockUseCodeMirror = vi.fn();
  const mockMermaidRender = vi.fn();
  const mockMermaidInit = vi.fn();
  const mockPersist = vi.fn();
  const mockOpenUrl = vi.fn();
  const mockOpenFileInActivePage = vi.fn();
  const mockExportContextBindings = vi.fn<() => { keystroke: string }[]>(() => []);

  // 预览窗口编排 mock（PreviewFrame 经 src/ipc/preview 与 src/ipc/window）
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
  const onMainWindowMoved = vi.fn((cb: () => void) => { void cb; return () => {}; });
  return {
    mockReadFile,
    mockReadResource,
    mockUseCodeMirror,
    mockMermaidRender,
    mockMermaidInit,
    mockPersist,
    mockOpenUrl,
    mockOpenFileInActivePage,
    mockExportContextBindings,
    previewSync,
    previewClose,
    previewRender,
    emitPreviewDownlink,
    onPreviewUplink,
    onPreviewHostStatus,
    onMainWindowMoved,
    uplinkHandlers,
    statusHandlers,
    resetAll() {
      for (const fn of [
        mockReadFile, mockReadResource, mockUseCodeMirror, mockMermaidRender,
        mockMermaidInit, mockPersist, mockOpenUrl, mockOpenFileInActivePage,
        mockExportContextBindings, previewSync, previewClose, previewRender,
        emitPreviewDownlink, onPreviewUplink, onPreviewHostStatus, onMainWindowMoved,
      ]) {
        fn.mockReset();
      }
      mockExportContextBindings.mockReturnValue([]);
      uplinkHandlers.length = 0;
      statusHandlers.length = 0;
      previewSync.mockResolvedValue(undefined);
      previewClose.mockResolvedValue(undefined);
      previewRender.mockResolvedValue(undefined);
      emitPreviewDownlink.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readFile: mocks.mockReadFile,
  readResourceBase64: mocks.mockReadResource,
}));
vi.mock("../ipc/shell", () => ({ openUrl: mocks.mockOpenUrl }));
vi.mock("../features/shortcuts/ShortcutRegistry", () => ({
  getShortcutRegistry: () => ({
    exportContextBindings: mocks.mockExportContextBindings,
  }),
}));
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
}));
// EDITOR_FONT_SPEC: EditorPanel→LargeFileViewer 模块级读取（CP-022 字体单点复用）——mock 缺失会致 import 期 TypeError
vi.mock("../panels/editor/useCodeMirror", () => ({
  useCodeMirror: (opts: unknown) => mocks.mockUseCodeMirror(opts),
  EDITOR_FONT_SPEC: { ".cm-scroller": { fontFamily: "monospace" } },
}));
vi.mock("mermaid", () => ({
  default: {
    initialize: mocks.mockMermaidInit,
    render: mocks.mockMermaidRender,
  },
}));
// allotment mock：Pane 透传 visible 为 data-pane-visible（真实 allotment 的
// visible=false 只收拢尺寸不卸载 children——CP-037 隐藏保活依赖此契约，见
// allotment Pane 实现；data 属性供 DOM 断言锁死）
vi.mock("allotment", () => ({
  Allotment: Object.assign(
    ({ children }: { children?: React.ReactNode }) =>
      React.createElement("div", { "data-testid": "allotment" }, children),
    {
      Pane: ({ children, visible }: { children?: React.ReactNode; visible?: boolean }) =>
        React.createElement(
          "div",
          { "data-pane-visible": visible === undefined ? "absent" : String(visible) },
          children,
        ),
    },
  ),
}));
vi.mock("../workspace/persistPanelParams", () => ({
  persistPanelParams: mocks.mockPersist,
}));
vi.mock("../workspace/openFile", () => ({
  openFileInActivePage: mocks.mockOpenFileInActivePage,
}));

import { MarkdownPanel } from "../panels/markdown";
import { useFontSize } from "../stores/fontSize";

/** md 面板默认 label（panelId md-1 → preview-md-1） */
const PANEL_LABEL = "preview-md-1";

function renderPanel(opts: {
  viewMode?: string;
  splitRatio?: unknown;
  filePath?: string;
  withApi?: boolean;
} = {}) {
  const updateParameters = vi.fn();
  const containerApi = { toJSON: () => ({}) };
  return {
    ...render(
      React.createElement(MarkdownPanel, {
        api:
          opts.withApi !== false
            ? ({ updateParameters } as never)
            : undefined,
        containerApi: opts.withApi !== false ? (containerApi as never) : undefined,
        params: {
          panelId: "md-1",
          filePath: opts.filePath ?? "C:/docs/note.md",
          viewMode: opts.viewMode as never,
          splitRatio: opts.splitRatio as never,
        },
      }),
    ),
    updateParameters,
  };
}

/** 从装配产物提取注入 nonce（SEC-04 校验值） */
function extractNonce(doc: string): string {
  const m = /nonce:"([0-9a-f]+)"/.exec(doc);
  if (!m) throw new Error("装配产物未找到 nonce");
  return m[1]!;
}

/**
 * 等待某次 previewRender 推送产物包含指定内容（渲染管线产物落地判定——
 * jsdom 无预览窗口，iframe/srcDoc 断言改为产物捕获断言）。
 * 返回该产物串。
 */
async function waitForPushedDoc(substring: string): Promise<string> {
  await waitFor(() => {
    const found = mocks.previewRender.mock.calls.some(
      (c) => (c[1] as string).includes(substring),
    );
    expect(found).toBe(true);
  }, { timeout: 3000 });
  const calls = mocks.previewRender.mock.calls;
  return calls[calls.length - 1]![1] as string;
}

/** 等 CM 桥就绪：最近一次 useCodeMirror 调用带非 null 容器（ready + bump 后） */
async function waitForCmMounted() {
  await waitFor(() => {
    const calls = mocks.mockUseCodeMirror.mock.calls;
    const last = calls[calls.length - 1]![0] as { container: unknown };
    expect(last.container).not.toBeNull();
  }, { timeout: 3000 });
}

/** 取各次 useCodeMirror 调用传入的 container 序列 */
function cmContainerSeq(): unknown[] {
  return mocks.mockUseCodeMirror.mock.calls.map(
    (c) => (c[0] as { container: unknown }).container,
  );
}

/** CP-037 防复发代理：EditorView 卸载重建次数 ≈ container「元素 ↔ null」迁移计数
 * （mock 层无真实 EditorView——以 null→非 null 迁移代理构造 spy） */
function countCmRebuilds(): number {
  let rebuilds = 0;
  let alive = false;
  for (const container of cmContainerSeq()) {
    if (container == null) {
      alive = false;
    } else if (!alive) {
      alive = true;
      rebuilds += 1;
    }
  }
  return rebuilds;
}

/** 向预览消息桥上行处理器派发消息（模拟宿主页桥转发 iframe 文档上行） */
function dispatchUplink(msg: Record<string, unknown>) {
  for (const h of mocks.uplinkHandlers) {
    h(msg);
  }
}

/** 向宿主状态处理器派发消息（iframe 加载完成 = 内容重建完成） */
function dispatchHostStatus(status: string, label = PANEL_LABEL) {
  for (const h of mocks.statusHandlers) {
    h({ label, status });
  }
}

describe("MarkdownPanel", () => {
  beforeEach(() => {
    mocks.resetAll();
    mocks.mockReadFile.mockResolvedValue("# 磁盘内容\n\n正文");
    mocks.mockReadResource.mockResolvedValue("iVBORw0KGgo=");
    mocks.mockMermaidRender.mockResolvedValue({ svg: "<svg>mmd</svg>" });
  });

  afterEach(() => {
    cleanup();
  });

  it("编辑字号接线：store→useCodeMirror props 闭环（Ctrl+滚轮缩放语义，EditorPanel 同款）", async () => {
    useFontSize.setState({ editorFontSize: 14 });
    mocks.mockReadFile.mockResolvedValue("# 标题\n\n正文");
    renderPanel({});
    await waitForCmMounted();

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

  it("默认形态 edit：切换条三态 + 无预览窗口编排（CM 桥挂载）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    const switcher = container.querySelector('[data-e2e="markdown-mode-switcher"]');
    expect(switcher).not.toBeNull();
    expect(switcher!.textContent).toContain("编辑");
    expect(switcher!.textContent).toContain("编辑/预览");
    expect(switcher!.textContent).toContain("预览");
    // edit 形态：CM 容器在，无预览推送（PreviewFrame 未挂载）
    expect(mocks.previewRender).not.toHaveBeenCalled();
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown; gitGutterEnabled?: boolean };
    expect(last.container).not.toBeNull();
    expect(last.gitGutterEnabled).toBe(false);
  });

  it("切 preview → CM pane 隐藏保活（container 恒非 null）；渲染产物推送预览窗口", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();

    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });

    const doc = await waitForPushedDoc("<h1>磁盘内容</h1>");
    // 渲染管线（markdown-it → 完整文档）产物
    expect(doc.startsWith("<!doctype html>")).toBe(true);
    // 推送 label = preview-<panelId>
    const lastCall = mocks.previewRender.mock.calls[mocks.previewRender.mock.calls.length - 1]!;
    expect(lastCall[0]).toBe(PANEL_LABEL);
    // CP-037 翻转：preview 不再卸载 CM——container 恒传 cmContainerRef.current
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown };
    expect(last.container).not.toBeNull();
  });

  it("形态切换持久化 viewMode", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    expect(mocks.mockPersist).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      { viewMode: "preview" },
    );
  });

  it("草稿防抖渲染：edit 击键 300ms 后 preview 内容更新（fake timers）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    await waitForPushedDoc("磁盘内容");

    // 捕获 CM 桥 onDocContent → 模拟击键
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const cmCall = cmCalls[cmCalls.length - 1]![0] as {
      onDocContent?: (text: string, source: string) => void;
    };
    expect(cmCall.onDocContent).toBeDefined();

    vi.useFakeTimers();
    try {
      await act(async () => {
        cmCall.onDocContent!("# 草稿标题\n\n新内容", "edit");
      });
      // 未到防抖窗口：旧内容仍在（fake timers 下 waitFor 冻结——同步断言）
      const before = mocks.previewRender.mock.calls;
      const lastBefore = before[before.length - 1]![1] as string;
      expect(lastBefore).toContain("磁盘内容");
      // 跨过 300ms 防抖 → 重渲染草稿（act(async) flush 微任务链）
      await act(async () => {
        vi.advanceTimersByTime(350);
      });
    } finally {
      vi.useRealTimers();
    }
    await waitForPushedDoc("草稿标题");
  });

  it("preview 切回 edit：CM 恒挂载不重建（免快照回填/免二次读盘）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    // CP-037：preview 态 CM pane 隐藏保活——切回 edit 不卸载重建、不经 initialDoc
    // 快照回填（doc 全程经 onDocContent 持续同步，initialDoc 恒为当前 doc）
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-edit"]')!);
    });
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as {
      initialDoc?: string;
      container: unknown;
    };
    expect(last.container).not.toBeNull();
    expect(last.initialDoc).toContain("磁盘内容");
    expect(countCmRebuilds()).toBe(1);
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("viewMode=preview 布局恢复：直入预览推送；非法值回退 edit", async () => {
    renderPanel({ viewMode: "preview" });
    await waitForPushedDoc("磁盘内容");

    cleanup();
    const r2 = renderPanel({ viewMode: "bogus" });
    await waitForCmMounted();
    // 非法 viewMode → 回退 edit（无预览推送）
    expect(r2.container.querySelector('[data-e2e="markdown-mode-edit"]')).not.toBeNull();
  });

  it("相对图片与 mermaid 入预览（资源 data URL / svg）", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "![图](./img/a.png)\n\n```mermaid\ngraph TD\n  A --> B\n```",
    );
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    const doc = await waitForPushedDoc("data:image/png;base64,iVBORw0KGgo=");
    expect(doc).toContain("<svg>mmd</svg>");
  });

  it("链接点击：slterm_nav 上行 → external 系统浏览器；本地相对 → 应用内打开", async () => {
    mocks.mockReadFile.mockResolvedValue("[外](https://x.com) [内](./next.md)");
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    const doc = await waitForPushedDoc("https://x.com");
    const nonce = extractNonce(doc);

    await act(async () => {
      dispatchUplink({ label: PANEL_LABEL, type: "slterm_nav", nonce, href: "https://x.com" });
    });
    expect(mocks.mockOpenUrl).toHaveBeenCalledWith("https://x.com");

    await act(async () => {
      dispatchUplink({ label: PANEL_LABEL, type: "slterm_nav", nonce, href: "./next.md" });
    });
    expect(mocks.mockOpenFileInActivePage).toHaveBeenCalledWith("C:/docs/next.md");

    // # 锚点与空 → 分类在面板侧忽略（linkPolicy）
    await act(async () => {
      dispatchUplink({ label: PANEL_LABEL, type: "slterm_nav", nonce, href: "#sec" });
    });
    expect(mocks.mockOpenUrl).toHaveBeenCalledTimes(1);
    expect(mocks.mockOpenFileInActivePage).toHaveBeenCalledTimes(1);
  });

  it("keepZoom/keepScrollRatio：iframe-loaded 后按镜像下行恢复（zoom_set/scroll_set）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    const doc = await waitForPushedDoc("磁盘内容");
    const nonce = extractNonce(doc);

    // 缩放/滚动上行 → 父侧镜像
    await act(async () => {
      dispatchUplink({ label: PANEL_LABEL, type: "slterm_zoom", nonce, zoom: 1.3 });
    });
    await act(async () => {
      dispatchUplink({ label: PANEL_LABEL, type: "slterm_scroll", nonce, ratio: 0.5 });
    });
    expect(mocks.emitPreviewDownlink).not.toHaveBeenCalled();

    // 内容重建完成 → 归 1 静默复位 + 按镜像下行恢复（keepZoom/keepScrollRatio 开）
    await act(async () => dispatchHostStatus("iframe-loaded"));
    expect(mocks.emitPreviewDownlink).toHaveBeenCalledTimes(2);
    expect(mocks.emitPreviewDownlink.mock.calls[0]![0]).toEqual({
      label: PANEL_LABEL,
      type: "slterm_zoom_set",
      nonce,
      zoom: 1.3,
    });
    expect(mocks.emitPreviewDownlink.mock.calls[1]![0]).toEqual({
      label: PANEL_LABEL,
      type: "slterm_scroll_set",
      nonce,
      ratio: 0.5,
    });
  });

  it("split 形态：双 pane 布局（CM + PreviewFrame 锚点）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-split"]')!);
    });
    // split：预览内容推送（PreviewFrame 挂载于右 pane）
    await waitFor(() => {
      expect(mocks.previewRender).toHaveBeenCalled();
    });
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown };
    expect(last.container).not.toBeNull();
  });

  describe("CP-037 preview 隐藏保活（防复发组）", () => {
    it("① edit 输入 → preview 往返 → EditorView 构造仅一次（无卸载重建）", async () => {
      const { container } = renderPanel();
      await waitForCmMounted();

      const cmCalls = mocks.mockUseCodeMirror.mock.calls;
      const editCall = cmCalls[cmCalls.length - 1]![0] as {
        onDocContent?: (text: string, source: string) => void;
      };
      await act(async () => {
        editCall.onDocContent!("# 草稿标题\n\n新内容", "edit");
      });

      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      await waitForPushedDoc("草稿标题");
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-edit"]')!);
      });
      await waitForCmMounted();

      expect(countCmRebuilds()).toBe(1);
    });

    it("② preview 态 CM pane 仍在 DOM（visible=false 隐藏而非卸载）", async () => {
      const { container } = renderPanel();
      await waitForCmMounted();
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      await waitForPushedDoc("磁盘内容");

      // CM pane（恒 index 0）+ 预览 pane 并存——隐藏保活而非条件卸载
      const allotment = container.querySelector('[data-testid="allotment"]')!;
      expect(allotment.childElementCount).toBe(2);
      const cmPane = allotment.firstElementChild as HTMLElement;
      expect(cmPane.getAttribute("data-pane-visible")).toBe("false");
      const cmDiv = cmPane.firstElementChild;
      expect(cmDiv).not.toBeNull(); // CM 容器元素仍在 DOM

      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-edit"]')!);
      });
      const cmPaneBack = container
        .querySelector('[data-testid="allotment"]')!
        .firstElementChild as HTMLElement;
      expect(cmPaneBack.getAttribute("data-pane-visible")).toBe("true");
      expect(cmPaneBack.firstElementChild).toBe(cmDiv);
    });

    it("③ preview 态 onDocContent 驱动链不断（doc 同步 → 回 edit 内容一致）", async () => {
      const { container } = renderPanel();
      await waitForCmMounted();
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      await waitForPushedDoc("磁盘内容");

      // preview 态 CM 恒挂载——捕获其 onDocContent（隐藏态仍写回 doc）
      const cmCalls = mocks.mockUseCodeMirror.mock.calls;
      const previewCall = cmCalls[cmCalls.length - 1]![0] as {
        container: unknown;
        onDocContent?: (text: string, source: string) => void;
      };
      expect(previewCall.container).not.toBeNull();
      expect(previewCall.onDocContent).toBeDefined();

      vi.useFakeTimers();
      try {
        await act(async () => {
          previewCall.onDocContent!("# 草稿标题\n\n新内容", "edit");
        });
        await act(async () => {
          vi.advanceTimersByTime(350);
        });
      } finally {
        vi.useRealTimers();
      }
      await waitForPushedDoc("草稿标题");

      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-edit"]')!);
      });
      const cmAfter = mocks.mockUseCodeMirror.mock.calls;
      const lastEdit = cmAfter[cmAfter.length - 1]![0] as {
        initialDoc?: string;
        container: unknown;
      };
      expect(lastEdit.container).not.toBeNull();
      expect(lastEdit.initialDoc).toContain("草稿标题");
      expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
    });
  });
});
