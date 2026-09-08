/**
 * Markdown 面板域 E2E spec（S6 + S10-② 预览迁独立 webview 适配）：默认 edit +
 * 形态切换、渲染管线产物（标题/表格/hljs/KaTeX/相对图片 data: URL/mermaid
 * SVG）、主窗口快捷键关闭、预览 Ctrl+滚轮缩放（事件属性通道触发注入接管）。
 *
 * 【预览渲染于独立 webview（S10-②，ADR-0019）】内容断言经 switchToWindow 到
 * 预览窗口（句柄 = preview-<panelId>）读宿主页 iframe srcdoc；HUD/切换条在
 * 主窗口工具条带；用例结束前一律切回 main（spike 驱动契约，见
 * e2e-tests/CLAUDE.md「多 webview WDIO 可达性」节）。
 *
 * 半端到端边界（e2e-tests/CLAUDE.md DOC-02）：键盘为合成 keydown（主窗口
 * ShortcutRegistry 路径）；md 内缩放触发走 raw HTML 事件属性（`<img onerror>`
 * ——宿主 <script> 静态化缺陷已随 S10-② 消亡（CP-031），事件属性通道在预览域
 * 无 CSP 下同样成立；本地缺失图片必然尝试加载 → error 稳定触发；
 * raw img 相对 src 进资源收集 → 读取失败回退原 src（缺口语义）→ iframe
 * 内加载缺失文件触发 error——通道闭环实证）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  createProject,
  waitPreviewDocContains,
  switchToMainWindow,
  previewWindowLabel,
} from "./specUtils";

/** 建项目并打开 markdownviewer 面板；返回 panelId */
async function spawnMarkdownPanel(
  projectDir: string,
  mdPath: string,
  viewMode?: string,
): Promise<string> {
  await waitForWorkspaceReady();
  await createProject(projectDir);
  await waitForDockviewApi();
  const panelId = `e2e-md-${Date.now()}`;
  await browser.execute(
    (args: { pid: string; path: string; viewMode?: string }) => {
      window.__dockviewApi!.addPanel({
        id: args.pid,
        component: "markdownviewer",
        params: {
          panelId: args.pid,
          filePath: args.path,
          ...(args.viewMode ? { viewMode: args.viewMode } : {}),
        },
      });
    },
    { pid: panelId, path: mdPath, viewMode },
  );
  return panelId;
}

/** 读主窗 HUD 文本（markdown-zoom-hud——工具条带内；隐藏恒不渲染） */
async function readPanelHud(): Promise<string | null> {
  return browser.execute(
    () => document.querySelector('[data-e2e="markdown-zoom-hud"]')?.textContent ?? null,
  );
}

describe("Markdown 面板三形态", () => {
  it("默认形态 edit：切换条三态；切 preview 渲染产物完整（标题/表格/hljs/KaTeX）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(
      mdPath,
      [
        "# E2E 标题",
        "",
        "| a | b |",
        "| --- | --- |",
        "| 1 | 2 |",
        "",
        "```js",
        "const x = 1;",
        "```",
        "",
        "行内 $x^2$ 与块级 $$\\int_0^1 x\\,dx$$",
      ].join("\n"),
      "utf8",
    );
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath);
      // 默认 edit：切换条三态 + 无预览窗口编排
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="markdown-mode-switcher"]')),
        { timeout: 15000, timeoutMsg: "markdown 切换条未出现" },
      );
      const switcherText = await browser.execute(
        () => document.querySelector('[data-e2e="markdown-mode-switcher"]')?.textContent ?? "",
      );
      expect(switcherText).toContain("编辑");
      expect(switcherText).toContain("编辑/预览");
      expect(switcherText).toContain("预览");
      expect((await browser.getWindowHandles()).includes(previewWindowLabel(panelId))).toBe(false);

      // 切预览 → 预览窗口渲染管线产物（宿主页 iframe srcdoc 可读）
      await browser.execute(() => {
        document.querySelector<HTMLButtonElement>('[data-e2e="markdown-mode-preview"]')?.click();
      });
      await waitPreviewDocContains(panelId, "<h1>E2E 标题</h1>");
      await waitPreviewDocContains(panelId, "<table>");
      await waitPreviewDocContains(panelId, "hljs-keyword");
      await waitPreviewDocContains(panelId, "katex");
      await waitPreviewDocContains(panelId, "katex-display");
      // KaTeX 内联字体已装配（ADR-0018——产物在预览域渲染）
      await waitPreviewDocContains(panelId, "data:font/woff2;base64,");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("相对图片经沙箱通道 data: URL 内联 + mermaid 宿主渲染 SVG", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-img-"));
    const pixelPath = join(tempDir, "pixel.png");
    writeFileSync(
      pixelPath,
      Buffer.from(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
        "base64",
      ),
    );
    expect(existsSync(pixelPath)).toBe(true);
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(
      mdPath,
      "![像素](./pixel.png)\n\n```mermaid\ngraph TD\n  A --> B\n```",
      "utf8",
    );
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, "preview");
      // 图片经沙箱通道 data: URL 内联（缺口语义不出现）；mermaid SVG 注入
      await waitPreviewDocContains(panelId, "data:image/png;base64,");
      await waitPreviewDocContains(panelId, "<svg");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("预览态经主窗口 Ctrl+W 快捷键关闭该页签", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-close-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(mdPath, "# 关闭测试", "utf8");
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitPreviewDocContains(panelId, "关闭测试");
      // S10-②：键盘不跨窗口（预览窗口 focusable=false）——预览态下主窗口
      // ShortcutRegistry 仍持焦点，合成 Ctrl+W → global.closeTab → 关闭活跃
      // 面板（md 面板为活跃面板）；面板卸载 → 预览窗口随之销毁
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (pid: string) => {
              window.dispatchEvent(
                new KeyboardEvent("keydown", {
                  ctrlKey: true,
                  code: "KeyW",
                  key: "w",
                  bubbles: true,
                  cancelable: true,
                }),
              );
              return window.__dockviewApi?.getPanel(pid) === undefined;
            },
            panelId,
          ),
        { timeout: 10000, timeoutMsg: "markdown 面板未被主窗口 Ctrl+W 快捷键关闭" },
      );
      const closed = await browser.execute(
        (pid: string) => window.__dockviewApi?.getPanel(pid) === undefined,
        panelId,
      );
      expect(closed).toBe(true);
      // 面板关闭 → PreviewFrame 卸载 → 预览窗口销毁出列（异步）
      await browser.waitUntil(
        async () => !(await browser.getWindowHandles()).includes(previewWindowLabel(panelId)),
        { timeout: 10000, timeoutMsg: "关闭面板后预览窗口未出列" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("预览 Ctrl+滚轮缩放：事件属性通道触发注入接管 → 主窗 HUD 121%", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-zoom-"));
    const mdPath = join(tempDir, "doc.md");
    // raw HTML 事件属性通道（预览域无 CSP——onerror 正常执行）：
    // 本地缺失图片必然尝试加载（img 无 preload 语义）→ error 稳定触发 →
    // 合成 2 格 Ctrl+wheel → zoomRuntime 接管 → 上行 → 主窗 HUD。
    // 注：raw img 的相对 src 会进资源收集（png 白名单）→ 读取失败回退原 src
    //（缺口语义）→ iframe 内加载缺失文件触发 error——通道闭环。
    writeFileSync(
      mdPath,
      [
        "# 缩放",
        "",
        '<img src="./__missing__.png" alt="x" onerror="for(var i=0;i<2;i++){document.dispatchEvent(new WheelEvent(\'wheel\',{deltaY:-120,ctrlKey:true,cancelable:true}))}">',
      ].join("\n"),
      "utf8",
    );
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitPreviewDocContains(panelId, "缩放");
      // 2 格等比 ×1.1² ≈ 1.21 → markdown HUD 121%
      await browser.waitUntil(
        async () => (await readPanelHud())?.includes("121%") ?? false,
        { timeout: 20000, timeoutMsg: "markdown 预览缩放 HUD 未达 121%" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("edit 态 Ctrl+滚轮缩放编辑器字号：合成 WheelEvent → 字号 14→15（EditorPanel 同语义）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-fontsize-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(mdPath, "# 字号\n\nmarker", "utf8");
    try {
      await spawnMarkdownPanel(tempDir, mdPath);
      // 可见 .cm-content 挂载（默认 edit 形态）
      await browser.waitUntil(
        async () =>
          await browser.execute(
            () =>
              Array.from(document.querySelectorAll(".cm-content")).some(
                (el) => el.getClientRects().length > 0,
              ),
          ),
        { timeout: 15000, timeoutMsg: "md edit 编辑器未挂载" },
      );
      // 合成 Ctrl+wheel 一格（deltaY -120）到 .cm-content——wheel 由 useCodeMirror
      // 挂载于 CM 容器（capture），命中即调共享 editorFontSize store setter
      await browser.execute(() => {
        const cm = Array.from(document.querySelectorAll(".cm-content")).find(
          (el) => el.getClientRects().length > 0,
        );
        cm?.dispatchEvent(
          new WheelEvent("wheel", { deltaY: -120, ctrlKey: true, cancelable: true, bubbles: true }),
        );
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => {
            const scroller = document.querySelector(".cm-scroller");
            return scroller ? getComputedStyle(scroller).fontSize : null;
          })) === "15px",
        { timeout: 10000, timeoutMsg: "md 编辑 Ctrl+滚轮未生效（字号未 14→15）" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
      await switchToMainWindow();
    }
  });
});
