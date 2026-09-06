/**
 * Markdown 面板域 E2E spec（S6）：默认 edit + 形态切换、渲染管线产物
 * （标题/表格/hljs/KaTeX/相对图片 data: URL/mermaid SVG）、Ctrl+W 合成转发
 * 关闭、预览 Ctrl+滚轮缩放（事件属性通道触发注入接管）。
 *
 * 【面板限定查询】单 session 跨用例共享 app：前序用例的面板留在活跃页
 * （createProject 不切换页面）——document.querySelector 首元素会被旧面板
 * 遮蔽（iframe/HUD 错位即 flaky 根因实证）。本 spec 一律以唯一文件名
 * （doc-<ts>.md，title 属性含完整路径）定位本面板 iframe，HUD 取其父容器。
 *
 * 半端到端边界（e2e-tests/CLAUDE.md DOC-02）：键盘为合成 MessageEvent；
 * md 内缩放触发走 raw HTML 事件属性（`<img onerror>`——宿主 <script> 被
 * escapeScriptClose 静态化为存量缺陷，事件属性不受转义破坏且 CSP 放行；
 * 本地缺失图片必然尝试加载（img 无 preload 语义）→ error 稳定触发；
 * raw img 相对 src 进资源收集 → 读取失败回退原 src（缺口语义）→ iframe
 * 内加载缺失文件触发 error——通道闭环实证。audio 通道不可靠（无 preload
 * 不触发加载）故弃用。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { waitForWorkspaceReady, waitForDockviewApi, createProject } from "./specUtils";

/** 建项目并打开 markdownviewer 面板；返回 panelId + 唯一文件名（title 定位键） */
async function spawnMarkdownPanel(
  projectDir: string,
  mdPath: string,
  fileName: string,
  viewMode?: string,
): Promise<string> {
  await waitForWorkspaceReady();
  await createProject(projectDir);
  await waitForDockviewApi();
  const panelId = `e2e-md-${Date.now()}-${fileName}`;
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

/** 在页面内按唯一文件名定位本面板 PreviewFrame iframe（title 含文件路径） */
function frameQuery(fileName: string): string {
  return `Array.from(document.querySelectorAll("iframe")).find(f => (f.getAttribute("title") ?? "").includes(${JSON.stringify(fileName)}))`;
}

/** 等待本面板 iframe 出现（预览渲染完成） */
async function waitForPreviewFrame(fileName: string): Promise<void> {
  await browser.waitUntil(
    async () => await browser.execute((q: string) => eval(q) != null, frameQuery(fileName)),
    { timeout: 20000, timeoutMsg: `markdown 预览 iframe 未渲染（${fileName}）` },
  );
}

/** 等待本面板 iframe srcdoc 包含期望文本 */
async function waitPreviewContains(fileName: string, text: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      await browser.execute(
        (args: { q: string; t: string }) => (eval(args.q) as HTMLIFrameElement | null)?.getAttribute("srcdoc")?.includes(args.t) ?? false,
        { q: frameQuery(fileName), t: text },
      ),
    { timeout: 20000, timeoutMsg: `预览 srcdoc 未包含 ${text}` },
  );
}

/**
 * 读 HUD 文本。悬浮区收敛后 HUD 在面板根 FloatingArea（iframe 的祖先更上层，
 * 不在 frame.parentElement 内）——全局查询；残留面板 HUD 恒隐藏（visible 才渲染）
 * 不在 DOM，缩放用例中全局唯一激活无歧义（html.e2e 同风格）。
 */
async function readPanelHud(_fileName: string): Promise<string | null> {
  return browser.execute(
    () => document.querySelector('[data-e2e="markdown-zoom-hud"]')?.textContent ?? null,
  );
}

describe("Markdown 面板三形态", () => {
  it("默认形态 edit：切换条三态；切 preview 渲染产物完整（标题/表格/hljs/KaTeX）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-"));
    const fileName = `doc-${Date.now()}.md`;
    const mdPath = join(tempDir, fileName);
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
      await spawnMarkdownPanel(tempDir, mdPath, fileName);
      // 默认 edit：切换条三态 + 无本面板 iframe
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
      expect(await browser.execute((q: string) => eval(q) == null, frameQuery(fileName))).toBe(true);

      // 切预览 → 本面板 iframe 渲染管线产物
      await browser.execute(() => {
        document.querySelector<HTMLButtonElement>('[data-e2e="markdown-mode-preview"]')?.click();
      });
      await waitForPreviewFrame(fileName);
      await waitPreviewContains(fileName, "<h1>E2E 标题</h1>");
      await waitPreviewContains(fileName, "<table>");
      await waitPreviewContains(fileName, "hljs-keyword");
      await waitPreviewContains(fileName, "katex");
      await waitPreviewContains(fileName, "katex-display");
      // KaTeX 内联字体已装配（ADR-0018）
      await waitPreviewContains(fileName, "data:font/woff2;base64,");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("相对图片经沙箱通道 data: URL 内联 + mermaid 宿主渲染 SVG", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-img-"));
    const fileName = `doc-${Date.now()}.md`;
    const pixelPath = join(tempDir, "pixel.png");
    writeFileSync(
      pixelPath,
      Buffer.from(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
        "base64",
      ),
    );
    expect(existsSync(pixelPath)).toBe(true);
    const mdPath = join(tempDir, fileName);
    writeFileSync(
      mdPath,
      "![像素](./pixel.png)\n\n```mermaid\ngraph TD\n  A --> B\n```",
      "utf8",
    );
    try {
      await spawnMarkdownPanel(tempDir, mdPath, fileName, "preview");
      await waitForPreviewFrame(fileName);
      // 图片经沙箱通道 data: URL 内联（缺口语义不出现）；mermaid SVG 注入
      await waitPreviewContains(fileName, "data:image/png;base64,");
      await waitPreviewContains(fileName, "<svg");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("预览态 Ctrl+W 合成 MessageEvent → 转发关闭该页签（键桥全链路）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-close-"));
    const fileName = `doc-${Date.now()}.md`;
    const mdPath = join(tempDir, fileName);
    writeFileSync(mdPath, "# 关闭测试", "utf8");
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, fileName, "preview");
      await waitForPreviewFrame(fileName);
      // 同 html.e2e：MessageEvent origin="null" + source=iframe.contentWindow + srcdoc nonce
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (args: { q: string; pid: string }) => {
              const frame = eval(args.q) as HTMLIFrameElement | null;
              const nonce = frame?.getAttribute("srcdoc")?.match(/nonce:"([0-9a-f]{32})"/)?.[1] ?? "";
              const msgEvent = new MessageEvent("message", {
                data: {
                  type: "slterm_key",
                  nonce,
                  fingerprint: "Ctrl+KeyW",
                  ctrlKey: true, shiftKey: false, altKey: false, metaKey: false,
                  code: "KeyW", key: "w",
                },
                origin: "null",
                source: frame?.contentWindow ?? null,
              });
              window.dispatchEvent(msgEvent);
              return window.__dockviewApi?.getPanel(args.pid) === undefined;
            },
            { q: frameQuery(fileName), pid: panelId },
          ),
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

  it("预览 Ctrl+滚轮缩放：事件属性通道触发注入接管 → 本面板 HUD 121%", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-zoom-"));
    const fileName = `doc-${Date.now()}.md`;
    const mdPath = join(tempDir, fileName);
    // raw HTML 事件属性通道（宿主 script 静态化存量缺陷——onerror 不受转义破坏）：
    // 本地缺失图片必然尝试加载（img 无 preload 语义）→ error 稳定触发 →
    // 合成 2 格 Ctrl+wheel → zoomRuntime 接管 → 上行 → 本面板 HUD。
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
      await spawnMarkdownPanel(tempDir, mdPath, fileName, "preview");
      await waitForPreviewFrame(fileName);
      // 2 格等比 ×1.1² ≈ 1.21 → 本面板 markdown HUD 121%
      await browser.waitUntil(
        async () => (await readPanelHud(fileName))?.includes("121%") ?? false,
        { timeout: 20000, timeoutMsg: "markdown 预览缩放 HUD 未达 121%" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
