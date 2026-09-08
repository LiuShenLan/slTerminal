// markdown-panel.test.tsx — MarkdownPanel 三形态编排器测试
//
// 面板层逻辑真实（mock 外围：CM 桥/渲染管线资源/mermaid/allotment 布局）：
//   1. 默认 edit + 悬浮切换条三态
//   2. edit ↔ split ↔ preview 形态切换与布局（CP-037：CM 恒挂载、preview 态
//      visible=false 隐藏保活——undo/光标跨形态保留）
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

/** 取各次 useCodeMirror 调用传入的 container 序列 */
function cmContainerSeq(): unknown[] {
  return mocks.mockUseCodeMirror.mock.calls.map(
    (c) => (c[0] as { container: unknown }).container,
  );
}

/** CP-037 防复发代理：EditorView 卸载重建次数 ≈ container 参数「非 null → null →
 * 非 null」的 null→非 null 迁移计数——真实 useCodeMirror 以 container effect 驱动
 * new EditorView（useCodeMirror.ts 容器 effect 先例），container 每从元素跳 null 再
 * 跳回即一次重建；mock 层无真实 EditorView，故以该迁移数作为构造 spy */
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

  it("编辑字号接线：store→useCodeMirror props 闭环（Ctrl+滚轮缩放语义，EditorPanel 同款）", async () => {
    useFontSize.setState({ editorFontSize: 14 });
    mocks.mockReadFile.mockResolvedValue("# 标题\n\n正文");
    const { container } = renderPanel({});
    await waitForCmMounted(container);

    // 接线：hook 收到 store 初值字号与 setter（wheel 事件被 hook 无条件挂载，
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

  it("切 preview → CM pane 隐藏保活（container 恒非 null）；渲染管线产物入 srcDoc", async () => {
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
    // CP-037 翻转：preview 不再卸载 CM——container 恒传 cmContainerRef.current
    //（CM pane 恒挂载，仅 allotment visible=false 隐藏）
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
    // 防复发锚（翻转自「preview 卸载 CM」断言）：全程仅初始挂载一次构造——
    // 卸载重建（container null→非 null 迁移）在 preview 往返中不再发生
    expect(countCmRebuilds()).toBe(1);
    // 读盘仅一次（内容经 onDocContent 同步回 doc state，不重复 IPC）
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

  describe("CP-037 preview 隐藏保活（防复发组）", () => {
    it("① edit 输入 → preview 往返 → EditorView 构造仅一次（无卸载重建）", async () => {
      const { container, getByTitle } = renderPanel();
      await waitForCmMounted();

      // edit 态输入草稿（带内容跨形态往返的前置）
      const cmCalls = mocks.mockUseCodeMirror.mock.calls;
      const editCall = cmCalls[cmCalls.length - 1]![0] as {
        onDocContent?: (text: string, source: string) => void;
      };
      await act(async () => {
        editCall.onDocContent!("# 草稿标题\n\n新内容", "edit");
      });

      // preview → edit 往返
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      await waitForFrame(getByTitle);
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-edit"]')!);
      });
      await waitForCmMounted();

      // EditorView 构造 spy（container null→非 null 迁移代理，见 countCmRebuilds）
      // 全流程仅初始挂载 1 次；修复前（preview 卸载 CM）此场景回 edit 会重建 → 2 次
      expect(countCmRebuilds()).toBe(1);
    });

    it("② preview 态 CM pane 仍在 DOM（visible=false 隐藏而非卸载）", async () => {
      const { container, getByTitle } = renderPanel();
      await waitForCmMounted();
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      await waitForFrame(getByTitle);

      // CM pane（恒 index 0）+ 预览 pane 并存——隐藏保活而非条件卸载（修复前仅 1 个）
      const allotment = container.querySelector('[data-testid="allotment"]')!;
      expect(allotment.childElementCount).toBe(2);
      const cmPane = allotment.firstElementChild as HTMLElement;
      expect(cmPane.getAttribute("data-pane-visible")).toBe("false");
      const cmDiv = cmPane.firstElementChild;
      expect(cmDiv).not.toBeNull(); // CM 容器元素仍在 DOM

      // 回 edit：visible 翻转为 true，容器仍是原 DOM 节点（未卸载重建）
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
      const { container, getByTitle } = renderPanel();
      await waitForCmMounted();
      await act(async () => {
        fireEvent.click(container.querySelector('[data-e2e="markdown-mode-preview"]')!);
      });
      const iframe = await waitForFrame(getByTitle);

      // preview 态 CM 恒挂载——捕获其 onDocContent（隐藏态仍写回 doc）
      const cmCalls = mocks.mockUseCodeMirror.mock.calls;
      const previewCall = cmCalls[cmCalls.length - 1]![0] as {
        container: unknown;
        onDocContent?: (text: string, source: string) => void;
      };
      expect(previewCall.container).not.toBeNull();
      expect(previewCall.onDocContent).toBeDefined();

      // preview 态模拟击键 → 驱动链（onDocContent → doc state → 预览重渲染）不断
      vi.useFakeTimers();
      try {
        await act(async () => {
          previewCall.onDocContent!("# 草稿标题\n\n新内容", "edit");
        });
        // 跨过 300ms 防抖（fake timers 下 waitFor 冻结——act 内 flush 微任务链）
        await act(async () => {
          vi.advanceTimersByTime(350);
        });
      } finally {
        vi.useRealTimers();
      }
      await waitFor(() => {
        expect(iframe.getAttribute("srcDoc")).toContain("草稿标题");
      }, { timeout: 3000 });

      // 回 edit：内容一致（doc 全程经 onDocContent 同步，无磁盘/快照回填）
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

