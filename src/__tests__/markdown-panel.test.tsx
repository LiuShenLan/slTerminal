// markdown-panel.test.tsx — MarkdownPanel 三形态编排器测试
//
// 面板层逻辑真实（mock 外围：CM 桥/渲染管线资源/mermaid/allotment 布局）：
//   1. 默认 edit + 悬浮切换条三态
//   2. edit ↔ split ↔ preview 形态切换与布局（preview-only CM 卸载、快照回填）
//   3. 草稿防抖渲染（fake timers 300ms）与切形态 stale 立即渲染
//   4. viewMode/splitRatio params 恢复与非法回退
//   5. 链接 slterm_nav 上行 → external 系统浏览器 / local 应用内打开
//   6. 形态切换持久化 persistPanelParams

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
    resetAll() {
      mockReadFile.mockReset();
      mockReadResource.mockReset();
      mockUseCodeMirror.mockReset();
      mockMermaidRender.mockReset();
      mockMermaidInit.mockReset();
      mockPersist.mockReset();
      mockOpenUrl.mockReset();
      mockOpenFileInActivePage.mockReset();
      mockExportContextBindings.mockReset();
      mockExportContextBindings.mockReturnValue([]);
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
vi.mock("../panels/editor/useCodeMirror", () => ({
  useCodeMirror: (opts: unknown) => mocks.mockUseCodeMirror(opts),
}));
vi.mock("mermaid", () => ({
  default: {
    initialize: mocks.mockMermaidInit,
    render: mocks.mockMermaidRender,
  },
}));
vi.mock("allotment", () => ({
  Allotment: Object.assign(
    ({ children }: { children?: React.ReactNode }) =>
      React.createElement("div", { "data-testid": "allotment" }, children),
    { Pane: ({ children }: { children?: React.ReactNode }) => React.createElement("div", null, children) },
  ),
}));
vi.mock("../workspace/persistPanelParams", () => ({
  persistPanelParams: mocks.mockPersist,
}));
vi.mock("../workspace/openFile", () => ({
  openFileInActivePage: mocks.mockOpenFileInActivePage,
}));

import { MarkdownPanel } from "../panels/markdown";

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

/** 从 PreviewFrame srcDoc 提取注入 nonce（SEC-04 校验值） */
function extractNonce(iframe: HTMLIFrameElement): string {
  const doc = iframe.getAttribute("srcDoc") ?? "";
  const m = /nonce:"([0-9a-f]{32})"/.exec(doc);
  if (!m) throw new Error("srcDoc 未找到 nonce");
  return m[1]!;
}

/** 等 CM 桥就绪：最近一次 useCodeMirror 调用带非 null 容器（ready + bump 后） */
async function waitForCmMounted(container: HTMLElement | null = null) {
  await waitFor(() => {
    const calls = mocks.mockUseCodeMirror.mock.calls;
    const last = calls[calls.length - 1]![0] as { container: unknown };
    expect(last.container).not.toBeNull();
  }, { timeout: 3000 });
  return container;
}

/** 派发通过 origin/source 校验的消息（iframe 内容上行模拟） */
function dispatchFromFrame(iframe: HTMLIFrameElement, data: Record<string, unknown>) {
  window.dispatchEvent(
    new MessageEvent("message", {
      origin: "null",
      source: iframe.contentWindow,
      data: { nonce: extractNonce(iframe), ...data },
    }),
  );
}

async function waitForFrame(
  getByTitle: (t: string) => HTMLElement,
): Promise<HTMLIFrameElement> {
  return waitFor(
    () => getByTitle(`Markdown 预览: C:/docs/note.md`) as HTMLIFrameElement,
    { timeout: 3000 },
  );
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

  it("默认形态 edit：切换条三态 + 无预览（CM 桥挂载）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    const switcher = container.querySelector('[data-e2e="markdown-mode-switcher"]');
    expect(switcher).not.toBeNull();
    expect(switcher!.textContent).toContain("编辑");
    expect(switcher!.textContent).toContain("编辑/预览");
    expect(switcher!.textContent).toContain("预览");
    // edit 形态：CM 容器在（allotment 单 pane），无 PreviewFrame
    expect(container.querySelector("iframe")).toBeNull();
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown; gitGutterEnabled?: boolean };
    expect(last.container).not.toBeNull();
    expect(last.gitGutterEnabled).toBe(false);
  });

  it("切 preview → allotment 只渲染 PreviewFrame；渲染管线产物入 srcDoc", async () => {
    const { container, getByTitle } = renderPanel();
    await waitForCmMounted();

    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });

    const iframe = await waitForFrame(getByTitle);
    // 渲染管线（markdown-it → 完整文档）产物
    const doc = iframe.getAttribute("srcDoc")!;
    expect(doc).toContain("<h1>磁盘内容</h1>");
    expect(doc.startsWith("<!doctype html>")).toBe(true);
    // preview-only：CM 卸载（container=null）
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown };
    expect(last.container).toBeNull();
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
    const { container, getByTitle } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    await waitForFrame(getByTitle);

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
      // 未到防抖窗口：旧内容仍在
      await act(async () => {
        vi.advanceTimersByTime(100);
      });
      // fake timers 下 waitFor 轮询被冻结——切回真实时钟前直接同步断言
      const iframeBefore = container.querySelector("iframe") as HTMLIFrameElement;
      expect(iframeBefore.getAttribute("srcDoc")).toContain("磁盘内容");

      // 跨过 300ms 防抖 → 重渲染草稿（act(async) flush 微任务链——资源 mock
      // 立即 resolve，Promise.all 在 act 内完成）
      await act(async () => {
        vi.advanceTimersByTime(250);
      });
    } finally {
      vi.useRealTimers();
    }
    // 真实时钟下等待渲染产物落地
    const iframe = await waitForFrame(getByTitle);
    await waitFor(() => {
      expect(iframe.getAttribute("srcDoc")).toContain("草稿标题");
    }, { timeout: 3000 });
  });

  it("preview 切回 edit：initialDoc 快照回填（免二次读盘）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    // preview 期间 CM 卸载——其 initialDoc 快照 = 当前 doc
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
    // 读盘仅一次（快照回填不重复 IPC）
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  it("viewMode=preview 布局恢复：直入预览；非法值回退 edit", async () => {
    const { getByTitle } = renderPanel({ viewMode: "preview" });
    // 直入 preview：渲染异步完成 → iframe 出现
    await waitForFrame(getByTitle);

    cleanup();
    const r2 = renderPanel({ viewMode: "bogus" });
    await waitForCmMounted();
    // 非法 viewMode → 回退 edit（无 iframe）
    expect(r2.container.querySelector("iframe")).toBeNull();
    const switcher = r2.container.querySelector('[data-e2e="markdown-mode-edit"]') as HTMLButtonElement;
    expect(switcher.style.background).not.toBe("none");
  });

  it("相对图片与 mermaid 入预览（资源 data URL / svg）", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "![图](./img/a.png)\n\n```mermaid\ngraph TD\n  A --> B\n```",
    );
    const { container, getByTitle } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    const iframe = await waitForFrame(getByTitle);
    await waitFor(() => {
      const doc = iframe.getAttribute("srcDoc")!;
      expect(doc).toContain("data:image/png;base64,iVBORw0KGgo=");
      expect(doc).toContain("<svg>mmd</svg>");
    }, { timeout: 3000 });
  });

  it("链接点击：external → 系统浏览器；本地相对 → 应用内打开（+ 前缀分类）", async () => {
    mocks.mockReadFile.mockResolvedValue("[外](https://x.com) [内](./next.md)");
    const { container, getByTitle } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
    });
    const iframe = await waitForFrame(getByTitle);
    // 预览产物含两链接（渲染完成）
    await waitFor(() => {
      expect(iframe.getAttribute("srcDoc")).toContain("https://x.com");
    }, { timeout: 3000 });

    await act(async () => {
      dispatchFromFrame(iframe, { type: "slterm_nav", href: "https://x.com" });
    });
    expect(mocks.mockOpenUrl).toHaveBeenCalledWith("https://x.com");

    await act(async () => {
      dispatchFromFrame(iframe, { type: "slterm_nav", href: "./next.md" });
    });
    expect(mocks.mockOpenFileInActivePage).toHaveBeenCalledWith("C:/docs/next.md");

    // # 锚点与空 → 忽略
    await act(async () => {
      dispatchFromFrame(iframe, { type: "slterm_nav", href: "#sec" });
    });
    expect(mocks.mockOpenUrl).toHaveBeenCalledTimes(1);
    expect(mocks.mockOpenFileInActivePage).toHaveBeenCalledTimes(1);
  });

  it("split 形态：双 pane 布局（CM + PreviewFrame）", async () => {
    const { container } = renderPanel();
    await waitForCmMounted();
    await act(async () => {
      fireEvent.click(container.querySelector('[data-e2e="markdown-mode-split"]')!);
    });
    // split：CM 与 PreviewFrame 并存
    await waitFor(() => {
      expect(container.querySelector("iframe")).not.toBeNull();
    });
    const cmCalls = mocks.mockUseCodeMirror.mock.calls;
    const last = cmCalls[cmCalls.length - 1]![0] as { container: unknown };
    expect(last.container).not.toBeNull();
  });
});

