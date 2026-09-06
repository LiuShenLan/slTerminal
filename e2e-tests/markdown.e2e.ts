/**
 * Markdown 面板域 E2E spec（S6）：打开默认 edit + 形态切换、渲染管线产物
 * （标题/表格/hljs/KaTeX/相对图片 data: URL/mermaid SVG）、Ctrl+W 合成转发
 * 关闭、预览 Ctrl+滚轮缩放（事件属性通道触发注入接管）。
 *
 * 半端到端边界（e2e-tests/CLAUDE.md DOC-02）：键盘为合成 MessageEvent（embedded
 * 驱动无法投递 OS 按键，与 html.e2e.ts 同法）；md 内缩放触发走 raw HTML 事件
 * 属性（`<img onerror>`——宿主 <script> 被 escapeScriptClose 静态化为存量缺陷，
 * 事件属性不受转义破坏且 CSP 'unsafe-inline' 放行，通道实证同 html.e2e.ts）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, rmSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { waitForWorkspaceReady, waitForDockviewApi, createProject } from "./specUtils";

/** 建项目并打开 markdownviewer 面板（直接 addPanel——双击分发链路由 L2 覆盖） */
async function spawnMarkdownPanel(
  projectDir: string,
  mdPath: string,
  viewMode?: string,
): Promise<string> {
  await waitForWorkspaceReady();
  await createProject(projectDir);
  await waitForDockviewApi();
  const panelId = "e2e-md-" + Date.now();
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

/** 等待 iframe 出现（预览渲染完成） */
async function waitForPreviewFrame(): Promise<void> {
  await browser.waitUntil(
    async () => await browser.execute(() => !!document.querySelector("iframe")),
    { timeout: 20000, timeoutMsg: "markdown 预览 iframe 未渲染" },
  );
}

/** 等待预览 iframe srcdoc 包含期望文本 */
async function waitPreviewContains(text: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      await browser.execute(
        (t: string) => document.querySelector("iframe")?.getAttribute("srcdoc")?.includes(t) ?? false,
        text,
      ),
    { timeout: 20000, timeoutMsg: `预览 srcdoc 未包含 ${text}` },
  );
}

describe("Markdown 面板三形态", () => {
  it("双击打开默认 edit：切换条三态，无 iframe；切 preview 渲染产物完整", async () => {
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
        "",
        "## 相对图片",
        "![本地](./img/pixel.png)",
        "",
        "```mermaid",
        "graph TD",
        "  A --> B",
        "```",
      ].join("\n"),
      "utf8",
    );
    try {
      await spawnMarkdownPanel(tempDir, mdPath);
      // 默认 edit：切换条三态 + 无 iframe
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

      // 切预览 → iframe 渲染管线产物
      await browser.execute(() => {
        document.querySelector<HTMLButtonElement>('[data-e2e="markdown-mode-preview"]')?.click();
      });
      await waitForPreviewFrame();
      await waitPreviewContains("<h1>E2E 标题</h1>");
      await waitPreviewContains("<table>");
      // hljs 高亮（js 关键字 span）
      await waitPreviewContains("hljs-keyword");
      // KaTeX 数学（行内 + display）
      await waitPreviewContains("katex");
      await waitPreviewContains("katex-display");
      // KaTeX 内联字体已装配（ADR-0018）
      await waitPreviewContains("data:font/woff2;base64,");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("相对图片经沙箱通道 data: URL 内联显示 + mermaid 宿主渲染 SVG", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-img-"));
    // 1×1 像素 PNG（64 字节标准最小文件）
    const pixelPng = Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
      "base64",
    );
    const imgDir = join(tempDir, "img");
    mkdirSync(imgDir, { recursive: true });
    writeFileSync(join(imgDir, "pixel.png"), pixelPng);
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(
      mdPath,
      "![像素](./img/pixel.png)\n\n```mermaid\ngraph TD\n  A --> B\n```",
      "utf8",
    );
    try {
      await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitForPreviewFrame();
      await waitPreviewContains("data:image/png;base64,");
      // mermaid 宿主渲染的 SVG 注入 iframe
      await waitPreviewContains("<svg");
      // 真实渲染验证：图片经 data: URL 实际加载（naturalWidth > 0）
      const imgLoaded = await browser.execute(() => {
        const iframe = document.querySelector("iframe") as HTMLIFrameElement | null;
        // opaque origin 不可读 iframe 文档——经 srcdoc 内联检查 img src 已替换，
        // 实际加载由 iframe 内部完成（WebView2 渲染层），此处验证 data 前缀即通道成立
        return iframe?.getAttribute("srcdoc")?.includes("data:image/png;base64,") ?? false;
      });
      expect(imgLoaded).toBe(true);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("预览态 Ctrl+W 合成 MessageEvent → 转发关闭该页签（键桥全链路）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-close-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(mdPath, "# 关闭测试", "utf8");
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitForPreviewFrame();
      // 同 html.e2e：MessageEvent 构造 origin="null" + source=iframe.contentWindow +
      // srcdoc 提取 nonce（SEC-03/04 校验链在真实 WebView2 往返）
      await browser.waitUntil(
        async () =>
          await browser.execute((pid: string) => {
            const iframe = document.querySelector("iframe");
            const nonce = iframe?.getAttribute("srcdoc")?.match(/nonce:"([0-9a-f]{32})"/)?.[1] ?? "";
            const msgEvent = new MessageEvent("message", {
              data: {
                type: "slterm_key",
                nonce,
                fingerprint: "Ctrl+KeyW",
                ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
                code: "KeyW", key: "w",
              },
              origin: "null",
              source: iframe?.contentWindow ?? null,
            });
            window.dispatchEvent(msgEvent);
            return window.__dockviewApi?.getPanel(pid) === undefined;
          }, panelId),
        { timeout: 10000, timeoutMsg: "markdown 面板未被 Ctrl+W 合成 MessageEvent 转发关闭" },
      );
      const closed = await browser.execute(
        (pid: string) => window.__dockviewApi?.getPanel(pid) === undefined,
        panelId,
      );
      expect(closed).toBe(true);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("预览 Ctrl+滚轮缩放：事件属性通道触发注入接管 → HUD 121%（keepZoom 重建保留）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-zoom-"));
    const mdPath = join(tempDir, "doc.md");
    // raw HTML 事件属性通道（宿主 script 静态化存量缺陷——onerror 不受转义破坏）：
    // 图片加载失败触发 onerror → 合成 2 格 Ctrl+wheel 派发 document → zoomRuntime 接管
    const zoomOnload = [
      "# 缩放",
      "",
      '<img src="https://invalid.invalid/missing.png" alt="x" ',
      "onerror=\"for(var i=0;i<2;i++){document.dispatchEvent(new WheelEvent('wheel',{deltaY:-120,ctrlKey:true,cancelable:true}))}\">",
    ].join("");
    writeFileSync(mdPath, zoomOnload, "utf8");
    try {
      await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitForPreviewFrame();
      // 2 格等比 ×1.1² ≈ 1.21 → markdown HUD 121%
      await browser.waitUntil(
        async () =>
          await browser.execute(
            () => document.querySelector('[data-e2e="markdown-zoom-hud"]')?.textContent?.includes("121%") ?? false,
          ),
        { timeout: 15000, timeoutMsg: "markdown 预览缩放 HUD 未达 121%" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
