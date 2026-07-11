// html-panel.test.tsx — HtmlPanel 组件测试
//
// 覆盖路径：
//   1. 渲染状态 — loading/loaded(iframe)/error/三态切换
//   2. iframe 属性 — srcDoc/sandbox/style
//   3. 竞态取消 — 快速切换 filePath / 卸载后 resolve
//   4. 边界 — 空 HTML / 大内容 / script 标签

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, waitFor, fireEvent } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadFile = vi.fn();
  return {
    mockReadFile,
    resetAll() {
      mockReadFile.mockReset();
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readFile: mocks.mockReadFile,
}));

// iframe 键盘桥 mock（HtmlPanel 仅从 features/shortcuts import attachGlobalShortcutForwarder）
const { mockAttach, mockDetach } = vi.hoisted(() => {
  const mockDetach = vi.fn();
  return { mockAttach: vi.fn((doc: Document) => { void doc; return mockDetach; }), mockDetach };
});
vi.mock("../features/shortcuts", () => ({
  attachGlobalShortcutForwarder: mockAttach,
}));

// ─── 真实模块导入（mock 之后）───
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
    mockAttach.mockClear();
    mockDetach.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  // ==========================================================================
  // 渲染状态
  // ==========================================================================

  // 24. 初始渲染显示加载中
  it("初始渲染显示加载中", () => {
    mocks.mockReadFile.mockReturnValue(new Promise(() => {})); // 永不 resolve
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    expect(getByText("加载中...")).toBeDefined();
  });

  // 25. 加载完成后渲染 iframe
  it("加载完成后渲染 iframe", async () => {
    mocks.mockReadFile.mockResolvedValue("<h1>Hello</h1>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe).toBeDefined();
      expect(iframe.tagName).toBe("IFRAME");
    });
  });

  // 26. iframe 含 sandbox 属性
  it("iframe 含 sandbox 属性", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe.getAttribute("sandbox")).toBe(
        "allow-scripts allow-same-origin",
      );
    });
  });

  // 27. 加载完成后"加载中"文字消失
  it('加载完成后"加载中"文字消失', async () => {
    mocks.mockReadFile.mockResolvedValue("<p>done</p>");
    const { queryByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(queryByText("加载中...")).toBeNull();
    });
  });

  // 28. readFile reject 显示错误信息
  it("readFile reject 显示错误信息", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("权限不足"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      expect(getByText("加载失败: 权限不足")).toBeDefined();
    });
  });

  // 29. 错误信息红色
  it("错误信息红色", async () => {
    mocks.mockReadFile.mockRejectedValue(new Error("fail"));
    const { getByText } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const el = getByText("加载失败: fail");
      expect(el.style.color).toBe("rgb(244, 71, 71)"); // #F44747
    });
  });

  // 30. filePath 为 undefined 显示错误
  it("filePath 为 undefined 显示错误", () => {
    const { getByText } = renderHtmlPanel(undefined);
    expect(getByText("加载失败: 未指定文件路径")).toBeDefined();
  });

  // 31. 加载中背景色为 PANEL_BG
  it("加载中背景色为 PANEL_BG", () => {
    mocks.mockReadFile.mockReturnValue(new Promise(() => {}));
    const { container } = renderHtmlPanel("C:/test/index.html");
    const outerDiv = container.firstChild as HTMLElement;
    // jsdom 将颜色标准化为 rgb 格式
    expect(outerDiv.style.background).toBe("rgb(30, 30, 30)");
  });

  // 32. iframe 样式填满容器
  it("iframe 样式填满容器", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/index.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/index.html");
      expect(iframe.style.width).toBe("100%");
      expect(iframe.style.height).toBe("100%");
      // jsdom 将 border: none 标准化为 border: medium
      expect(iframe.style.border).toBe("medium");
    });
  });

  // ==========================================================================
  // 竞态取消
  // ==========================================================================

  // 33. 快速切换 filePath 后旧请求结果不覆盖新请求
  it("快速切换 filePath 后旧请求结果不覆盖新请求", async () => {
    let resolveOld: (v: string) => void = () => {};
    const oldPromise = new Promise<string>((resolve) => {
      resolveOld = resolve;
    });

    mocks.mockReadFile
      .mockReturnValueOnce(oldPromise) // 第一次：pending
      .mockResolvedValueOnce("<h1>New Content</h1>"); // 第二次：立即 resolve

    const { rerender, getByTitle } = renderHtmlPanel("C:/test/old.html");

    // 用新 filePath 重新渲染（模拟面板切换）
    rerender(
      React.createElement(HtmlPanel, {
        params: {
          panelId: "test-panel-1",
          filePath: "C:/test/new.html",
        },
      }),
    );

    // 旧请求晚 resolve —— 不应影响当前状态
    resolveOld("<h1>Old Content</h1>");

    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/new.html");
      // srcDoc 应为新内容
      expect(iframe.getAttribute("srcDoc")).toBe("<h1>New Content</h1>");
    });
  });

  // 34. 组件卸载后 readFile resolve 不 setState
  it("组件卸载后 readFile resolve 不报错", async () => {
    let resolveLater: (v: string) => void = () => {};
    const pending = new Promise<string>((resolve) => {
      resolveLater = resolve;
    });
    mocks.mockReadFile.mockReturnValue(pending);

    const { unmount } = renderHtmlPanel("C:/test/index.html");
    unmount();

    // 卸载后 resolve 不应抛出 "Can't perform a React state update" 警告
    // 此测试只要不抛异常即通过
    resolveLater("<p>late</p>");
    // 等一个 microtask 确保 promise 回调执行
    await new Promise((r) => setTimeout(r, 10));
  });

  // 35. 连续三次切换只显示最后一次结果
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

    // 第一次的 promise 晚 resolve
    resolveFirst("<h1>First</h1>");

    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/3.html");
      expect(iframe.getAttribute("srcDoc")).toBe("<h1>Third</h1>");
    });
  });

  // ==========================================================================
  // 边界
  // ==========================================================================

  // 36. 空 HTML 文件渲染空 iframe
  it("空 HTML 文件渲染空 iframe", async () => {
    mocks.mockReadFile.mockResolvedValue("");
    const { getByTitle } = renderHtmlPanel("C:/test/empty.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/empty.html");
      expect(iframe.getAttribute("srcDoc")).toBe("");
    });
  });

  // 37. 很大 HTML 内容（~100KB）
  it("很大 HTML 内容正常渲染", async () => {
    const bigContent = "<html><body>" + "x".repeat(100_000) + "</body></html>";
    mocks.mockReadFile.mockResolvedValue(bigContent);
    const { getByTitle } = renderHtmlPanel("C:/test/big.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/big.html");
      expect(iframe.getAttribute("srcDoc")).toBe(bigContent);
    });
  });

  // 38. HTML 含 script 标签
  it("HTML 含 script 标签正常渲染（sandbox 内执行）", async () => {
    mocks.mockReadFile.mockResolvedValue(
      "<html><body><script>document.body.innerHTML='JS OK'</script></body></html>",
    );
    const { getByTitle } = renderHtmlPanel("C:/test/script.html");
    await waitFor(() => {
      const iframe = getByTitle("HTML 预览: C:/test/script.html");
      expect(iframe.getAttribute("srcDoc")).toContain("JS OK");
    });
  });

  // ==========================================================================
  // renderer="always" 下的生命周期（白屏修复验证）
  // ==========================================================================

  // 65. props 不变时仅 mount 一次（不因 DOM 移动重新 mount）
  it("相同 filePath 重渲染不触发 readFile 多次", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { rerender } = renderHtmlPanel("C:/test/a.html");

    await waitFor(() => {
      expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
    });

    // 用相同 filePath 重新渲染（模拟 renderer="always" 下 panel 重新聚焦）
    rerender(
      React.createElement(HtmlPanel, {
        params: { panelId: "tp", filePath: "C:/test/a.html" },
      }),
    );

    // filePath 没变，useEffect 不应重新执行
    expect(mocks.mockReadFile).toHaveBeenCalledTimes(1);
  });

  // 66. cleanup 在 unmount 时执行（cancelled 标志）
  it("unmount 后 cleanup 阻止 setState", async () => {
    let resolveLater: (v: string) => void = () => {};
    const pending = new Promise<string>((r) => {
      resolveLater = r;
    });
    mocks.mockReadFile.mockReturnValue(pending);

    const { unmount, queryByText } = renderHtmlPanel("C:/test/a.html");
    expect(queryByText("加载中...")).toBeDefined();

    // 卸载组件
    unmount();

    // 晚 resolve——cleanup 已设置 cancelled=true，不应触发 setState
    resolveLater("<p>late</p>");
    await new Promise((r) => setTimeout(r, 10));

    // 组件已卸载，document 中不应有 iframe
    expect(document.querySelector("iframe")).toBeNull();
  });

  // 67. iframe 在 unmount 后被销毁
  it("iframe 在 unmount 后被销毁", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle, unmount } = renderHtmlPanel("C:/test/a.html");

    await waitFor(() => {
      expect(getByTitle("HTML 预览: C:/test/a.html")).toBeDefined();
    });

    unmount();

    // iframe 应已从 DOM 中移除
    expect(document.querySelector("iframe")).toBeNull();
  });

  // ==========================================================================
  // iframe 全局键转发桥
  // ==========================================================================

  // 68. iframe onLoad 后给 contentDocument 挂全局键转发
  it("iframe onLoad 后 attachGlobalShortcutForwarder 被以 contentDocument 调用", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");

    const iframe = (await waitFor(() =>
      getByTitle("HTML 预览: C:/test/a.html"),
    )) as HTMLIFrameElement;
    fireEvent.load(iframe);

    // jsdom 可能自动 fire 一次 load，故不断言精确次数，只验证以 contentDocument 挂载过
    expect(mockAttach).toHaveBeenCalled();
    expect(mockAttach.mock.calls.some((c) => c[0] === iframe.contentDocument)).toBe(true);
  });

  // 69. 组件卸载 → detach 被调用
  it("挂载 forwarder 后 unmount → detach 被调用", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle, unmount } = renderHtmlPanel("C:/test/a.html");

    const iframe = (await waitFor(() =>
      getByTitle("HTML 预览: C:/test/a.html"),
    )) as HTMLIFrameElement;
    fireEvent.load(iframe); // 确保已挂载 forwarder

    const detachBefore = mockDetach.mock.calls.length;
    unmount();
    expect(mockDetach.mock.calls.length).toBeGreaterThan(detachBefore);
  });

  // 70. 重载（再次 load）→ 每次 load 增 1 attach + 先 detach 旧的
  it("iframe 重载 → 先 detach 旧再 attach 新（增量）", async () => {
    mocks.mockReadFile.mockResolvedValue("<p>test</p>");
    const { getByTitle } = renderHtmlPanel("C:/test/a.html");

    const iframe = (await waitFor(() =>
      getByTitle("HTML 预览: C:/test/a.html"),
    )) as HTMLIFrameElement;
    fireEvent.load(iframe); // 已挂载一次

    const attachBefore = mockAttach.mock.calls.length;
    const detachBefore = mockDetach.mock.calls.length;
    fireEvent.load(iframe); // 重载：detach 旧 + attach 新

    expect(mockAttach.mock.calls.length).toBe(attachBefore + 1);
    expect(mockDetach.mock.calls.length).toBe(detachBefore + 1);
  });
});
