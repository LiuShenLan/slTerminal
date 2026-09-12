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
 * 域级 meta CSP 放行内联 script/style 下同样成立；本地缺失图片必然尝试加载 → error 稳定触发；
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
  activatePanel,
  clickInPanel,
  waitPreviewDocContains,
  switchToMainWindow,
  switchToPreviewWindow,
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

      // 切预览 → 预览窗口渲染管线产物（宿主页 iframe srcdoc 可读）。
      // CP-004 单宿主契约：clickInPanel 以本面板页签为锚点组内点击（残留面板
      // 切换条同 DOM 并存，全局 querySelector 首元素命中不可靠）
      expect(await clickInPanel(panelId, '[data-e2e="markdown-mode-preview"]')).toBe(true);
      await waitPreviewDocContains(panelId, "<h1>E2E 标题</h1>");
      await waitPreviewDocContains(panelId, "<table>");
      await waitPreviewDocContains(panelId, "hljs-keyword");
      await waitPreviewDocContains(panelId, "katex");
      await waitPreviewDocContains(panelId, "katex-display");
      // KaTeX 内联字体已装配（ADR-0018——产物在预览域渲染）
      await waitPreviewDocContains(panelId, "data:font/woff2;base64,");
      // KaTeX 字体真实加载锚点（TE-08 分支 b，2026-09-09 实证定案）：
      // 宿主页 FontFaceSet 不覆盖 iframe srcdoc 文档（实证 hostSize=0 且
      // fonts.check 对任意族名恒 true——对照组 NoSuchFontXyzQq 亦 true，语义
      // 失效；iframe opaque origin 宿主不可读）——加载态只能自 iframe 内取：
      // 注入的 fontProbe 段在 iframe 内 fonts.ready 后 check('12px "KaTeX_Main"')
      // 上行宿主桥 → PreviewFrame 收束写 window.__slterm_e2e_fontProbe
      // （E2E_ENABLED 门控；段仅 VITE_E2E 构建注入，生产零注入面）。
      // 前置声明锚点：字体族已在渲染文档内声明（防 check 对未声明族空真）。
      await waitPreviewDocContains(panelId, "KaTeX_Main");
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (l: string) =>
              (
                window as unknown as {
                  __slterm_e2e_fontProbe?: Record<string, boolean>;
                }
              ).__slterm_e2e_fontProbe?.[l] === true,
            previewWindowLabel(panelId),
          ),
        { timeout: 10000, timeoutMsg: "预览域 KaTeX 字体未真实加载（fontProbe 未达 true）" },
      );
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
      // 面板；面板卸载 → 预览窗口随之销毁。CP-004 单宿主契约：先显式激活本
      // 面板再派发（dockview 活跃面板不随 addPanel 归属——残留面板同宿主并存）
      expect(await activatePanel(panelId)).toBe(true);
      // 轮询派发 Ctrl+W 直到面板消失且连续 3 次采样保持消失（间隔 100ms）——
      // 多页组并存时关闭「页组唯一面板」触发 dockview 组移除 + 宿主页组同步链，
      // 存在瞬态抖动面；连续消失判定过滤抖动（单次采样判定会撞复活窗口）
      await browser.waitUntil(
        async () => {
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
              return pid;
            },
            panelId,
          );
          let gone = 0;
          for (let i = 0; i < 3; i++) {
            await browser.pause(100);
            const has = await browser.execute(
              (pid: string) => !!window.__dockviewApi?.getPanel(pid),
              panelId,
            );
            if (!has) gone += 1;
            else break;
          }
          return gone === 3;
        },
        { timeout: 15000, timeoutMsg: "markdown 面板未被主窗口 Ctrl+W 快捷键关闭" },
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
    // raw HTML 事件属性通道（预览域 meta CSP 放行内联 script——onerror 正常执行）：
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
      const panelId = await spawnMarkdownPanel(tempDir, mdPath);
      // 本面板组内容树内的可见 .cm-content 挂载（默认 edit 形态——CP-004 单宿主
      // 契约：残留面板 DOM 并存且隐藏页组可测矩形，锚定本面板过滤）
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (pid: string) => {
              const anchor = document.querySelector(`[data-e2e="tab-close-${pid}"]`);
              if (!anchor) return false;
              let el: HTMLElement | null = anchor.parentElement;
              while (el) {
                const cm = el.querySelector(".cm-content");
                if (cm && cm.getClientRects().length > 0) return true;
                el = el.parentElement;
              }
              return false;
            },
            panelId,
          ),
        { timeout: 15000, timeoutMsg: "md edit 编辑器未挂载" },
      );
      // 合成 Ctrl+wheel 一格（deltaY -120）到本面板 .cm-content——wheel 由
      // useCodeMirror 挂载于 CM 容器（capture），命中即调共享 editorFontSize
      // store setter（editorFontSize 为共享 store——只对本面板派发防污染残留面板字号）
      await browser.execute((pid: string) => {
        const anchor = document.querySelector(`[data-e2e="tab-close-${pid}"]`);
        if (!anchor) return;
        let el: HTMLElement | null = anchor.parentElement;
        while (el) {
          const cm = el.querySelector(".cm-content");
          if (cm && cm.getClientRects().length > 0) {
            cm.dispatchEvent(
              new WheelEvent("wheel", { deltaY: -120, ctrlKey: true, cancelable: true, bubbles: true }),
            );
            return;
          }
          el = el.parentElement;
        }
      }, panelId);
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

  it("主窗移动后预览窗口物理位置跟随（防复发：去重早退吞移动事件）", async () => {
    // 回归锚点（2026-09 预览错位 bug）：前端 syncNow 去重只比 CSS 视口矩形——
    // 主窗移动时矩形等值但物理基准已变，旧实现早退致预览窗停驻原位。
    // 断言物理位移传递：主窗 outer 移动 Δ → 预览窗同 Δ（无边框 outer=inner，
    // 与 scale 无关——scale=1 环境同样锁死去重早退根因）。
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-move-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(mdPath, "# 跟随", "utf8");
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath, "preview");
      await waitPreviewDocContains(panelId, "跟随");

      // 记录移动前主窗/预览窗物理 rect（getWindowRect 作用于当前窗口上下文）
      await switchToMainWindow();
      const mainBefore = await browser.getWindowRect();
      await switchToPreviewWindow(panelId);
      const previewBefore = await browser.getWindowRect();
      await switchToMainWindow();

      // 主窗物理位移 +150/+120（尺寸不变；最大化态由驱动先还原再移动）
      await browser.setWindowRect(
        mainBefore.x + 150,
        mainBefore.y + 120,
        mainBefore.width,
        mainBefore.height,
      );

      // onMoved → 50ms 节流强制 sync → 后端按新 inner 原点物理重定位
      await browser.waitUntil(
        async () => {
          await switchToPreviewWindow(panelId);
          const r = await browser.getWindowRect();
          await switchToMainWindow();
          return (
            Math.abs(r.x - (previewBefore.x + 150)) <= 4 &&
            Math.abs(r.y - (previewBefore.y + 120)) <= 4
          );
        },
        { timeout: 10000, timeoutMsg: "主窗移动后预览窗口未跟随（物理位置未同步）" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
      await switchToMainWindow();
    }
  });

  it("split 形态：预览窗口即刻出现且渲染，CM 编辑 pane 并存可见（防复发：split 右侧空白）", async () => {
    // 回归锚点（2026-09 split 空白 bug）：预览 pane 动态挂载（Allotment addView
    // 在父 effect，React 子 effect 先行）→ 首测 0×0 → 隐藏态不建窗，恢复依赖
    // 有缺陷的同步环（去重早退吞事件 + 200ms 轮询延迟）→ 右侧恒空白。
    // 修复 = 主窗事件 force-sync + 锚点 ResizeObserver 即时驱动——老代码本用例
    // waitPreviewDocContains 超时红。
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-md-split-"));
    const mdPath = join(tempDir, "doc.md");
    writeFileSync(mdPath, "# 分屏\n\n正文标记", "utf8");
    try {
      const panelId = await spawnMarkdownPanel(tempDir, mdPath);
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="markdown-mode-switcher"]')),
        { timeout: 15000, timeoutMsg: "markdown 切换条未出现" },
      );

      // 切 split → 预览窗口出现且内容渲染
      expect(await clickInPanel(panelId, '[data-e2e="markdown-mode-split"]')).toBe(true);
      await waitPreviewDocContains(panelId, "<h1>分屏</h1>", 15000);

      // CM 编辑 pane 并存可见（CP-037 恒挂载；本面板组内锚定过滤残留面板）
      await switchToMainWindow();
      const cmVisible = await browser.execute((pid: string) => {
        const anchor = document.querySelector(`[data-e2e="tab-close-${pid}"]`);
        if (!anchor) return false;
        let el: HTMLElement | null = anchor.parentElement;
        while (el) {
          const cm = el.querySelector(".cm-content");
          if (cm && cm.getClientRects().length > 0) return true;
          el = el.parentElement;
        }
        return false;
      }, panelId);
      expect(cmVisible).toBe(true);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
      await switchToMainWindow();
    }
  });
});
