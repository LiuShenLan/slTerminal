/**
 * HTML 面板域 E2E spec（E2E-09 拆分，S10-② 预览迁独立 webview 后重写）：
 * 主窗口快捷键关闭链路、Ctrl+滚轮缩放（注入运行时执行于预览窗口宿主页内
 * sandbox iframe；HUD/工具条带在主窗口）、宿主内联 <script> 执行（CP-031
 * 原 :87 skip 用例恢复）。
 *
 * 驱动契约（spike 结论 + e2e-tests/CLAUDE.md「多 webview WDIO 可达性」节）：
 * 预览窗口句柄 = label = preview-<panelId>——内容断言 switchToWindow 后经
 * 宿主页读 iframe srcdoc；HUD/面板断言在主窗口；execute-first，用例结束
 * 前一律切回 main。
 *
 * 半端到端边界（DOC-02）：embedded 驱动无法投递 OS 键/滚轮——键盘走主窗口
 * 合成 keydown dispatch（ShortcutRegistry capture 真实消费，editor.e2e
 * Ctrl+S 先例）；缩放走 fixture 文档内合成 WheelEvent 派发（注入运行时接管）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  createProject,
  waitPreviewDocContains,
  waitForPreviewWindow,
  switchToMainWindow,
  previewWindowLabel,
} from "./specUtils";

describe("HTML 面板主窗口快捷键关闭（Ctrl+W）", () => {
  // S10-②：预览内容迁独立 webview（focusable=false，焦点恒在主窗口）——键盘
  // 不跨窗口，旧「iframe 内 postMessage 转发」通道退役（CP-013）。本用例经
  // 主窗口合成 keydown（ShortcutRegistry capture 真实消费，editor.e2e 先例）
  // 验证 global.closeTab → closeTabGuarded 关闭活跃面板全链路（真实二进制）。
  it("主窗口合成 Ctrl+W keydown → 关闭该 HTML 页签", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-"));
    const htmlPath = join(tempDir, "page.html");
    writeFileSync(htmlPath, "<h1>e2e html</h1>", "utf8");

    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();

      const panelId = "e2e-html-" + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: "htmlviewer",
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: htmlPath },
      );

      // 等待渲染内容推送并渲染（预览窗口就绪）
      await waitPreviewDocContains(panelId, "e2e html");

      // 主窗口合成 Ctrl+W（ShortcutRegistry window capture 消费 → global.closeTab
      // → 活跃面板关闭——html 面板为活跃面板）
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
        { timeout: 10000, timeoutMsg: "HTML 面板未被主窗口 Ctrl+W 快捷键关闭" },
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
});

/**
 * HTML 面板 Ctrl+滚轮缩放（注入接管 + 瞬态 HUD）E2E。
 *
 * 背景：预览渲染于独立 webview 宿主页内 sandbox iframe（S10-②，ADR-0019）——
 * 宿主页域（自定义协议）无全局 CSP，fixture 内联事件属性/脚本真实执行；
 * 注入运行时（zoomRuntime wheel capture）先于 fixture 注册（注入段插于
 * </head> 前）→ 合成 WheelEvent → zoom → 上行 → 主窗 HUD（工具条带）。
 * embedded 驱动无法投递 OS 滚轮——fixture 自派发（行为同旧 iframe 通道）。
 */
describe("HTML 面板 Ctrl+滚轮缩放", () => {
  /**
   * 生成带两段定时派发的 fixture：阶段 A 于 onload 立即派 N 格、阶段 B ms 后派 M 格。
   * （等比缩放 ×1.1/格：1 格 → 110%、2 格 → 121%、3 格 → 133%）
   */
  function stagedFixture(msA: number, nA: number, msB: number, nB: number): string {
    const stepExpr = (n: number): string =>
      `for(var i=0;i<${n};i++){document.dispatchEvent(new WheelEvent('wheel',` +
      `{deltaY:-120,ctrlKey:true,cancelable:true}))}`;
    const onload =
      `${stepExpr(nA)};` +
      `setTimeout(function(){${stepExpr(nB)}},${msB})`;
    return (
      `<!DOCTYPE html><html><head><title>zoom-fixture</title></head>` +
      `<body onload="${onload}">` +
      `<h1>zoom fixture</h1></body></html>`
    );
  }

  /** 建项目并打开 htmlviewer 面板，等待预览窗口内容渲染；返回 panelId */
  async function spawnZoomPanel(
    projectDir: string,
    htmlPath: string,
    marker = "zoom fixture",
  ): Promise<string> {
    await waitForWorkspaceReady();
    await createProject(projectDir);
    await waitForDockviewApi();
    const panelId = "e2e-html-zoom-" + Date.now();
    await browser.execute(
      (args: { pid: string; path: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid,
          component: "htmlviewer",
          params: { panelId: args.pid, filePath: args.path },
        });
      },
      { pid: panelId, path: htmlPath },
    );
    await waitPreviewDocContains(panelId, marker);
    return panelId;
  }

  /** 读主窗 HUD 气泡文本（无气泡返回 null） */
  async function readHudText(): Promise<string | null> {
    return browser.execute(() => {
      const el = document.querySelector('[data-e2e="html-zoom-hud"]');
      return el && el.textContent ? el.textContent : null;
    });
  }

  /** 等待主窗 HUD 文本包含期望百分比（等比 ×1.1：2 格 121%、3 格 133%） */
  async function waitHudText(expected: string): Promise<void> {
    await browser.waitUntil(
      async () => (await readHudText())?.includes(expected) ?? false,
      { timeout: 12000, timeoutMsg: `HUD 文本未达到 ${expected}` },
    );
  }

  it("fixture 合成 Ctrl+wheel ×3 → 注入接管缩放 → HUD 133%", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    writeFileSync(htmlPath, stagedFixture(400, 3, 99999, 0), "utf8");
    try {
      await spawnZoomPanel(tempDir, htmlPath);
      // 等比 ×1.1³ ≈ 1.331 → "133%"
      await waitHudText("133%");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("点重置 → 下行复位 → 再次缩放基于 100% 重算（下行往返验证）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    // 阶段 A：onload 立即 2 格 → 121%；阶段 B：3500ms 后 2 格（供重置后二次断言——
    // 迁移 webview 后首轮 HUD/重置链路耗时更长，B 需晚于重置完成）
    writeFileSync(htmlPath, stagedFixture(400, 2, 3500, 2), "utf8");
    try {
      await spawnZoomPanel(tempDir, htmlPath);
      // 轮 1：2 格 → 121%
      await waitHudText("121%");
      // 点重置（主窗 HUD 按钮 → 下行事件 → 宿主 → iframe 归 1，真实 WebView2 往返）
      await browser.execute(() => {
        document.querySelector<HTMLButtonElement>('[data-e2e="html-zoom-reset"]')?.click();
      });
      // 主窗立即隐藏
      await browser.waitUntil(
        async () => (await readHudText()) === null,
        { timeout: 5000, timeoutMsg: "重置后 HUD 未消失" },
      );
      // 轮 2：下行成功（iframe 内归 1）→ 再次 121%；下行失败则从 1.21 继续 → 146%
      await waitHudText("121%");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("iframe 不重建时缩放会话保留（后续缩放基于上次值叠加）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    // 阶段 A：onload 立即 2 格 → 121%；阶段 B：2200ms 后 1 格
    writeFileSync(htmlPath, stagedFixture(400, 2, 2200, 1), "utf8");
    try {
      await spawnZoomPanel(tempDir, htmlPath);
      await waitHudText("121%");
      // 文档存活时闭包 zoom 保留：+1 格 → ×1.1 → 133%（若 iframe 重建归 1 则 110%）
      await waitHudText("133%");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ── 宿主内联 <script> 真实执行（CP-031 消亡判定三的 e2e 通道）──
  // S10-② 前：injectScript escapeScriptClose 把宿主 `</script>` 全部转义 →
  // 宿主 script 吞到 EOF 永不执行（存量缺陷，原 :87 skip 空壳登记的根因）；
  // 且宿主文档继承主窗 CSP（'unsafe-inline' 依赖）。S10-② 后：宿主 <script> 段
  // 不经字符串转义进入渲染文档（escapeScriptClose 消亡），且渲染于独立预览域
  // （自定义协议宿主页 iframe，无全局 CSP）——内联 <script> 真实可执行。
  // 本用例 = 原 :87 skip 用例恢复为真实断言：fixture 以 <script>（非事件属性）
  // 派发合成滚轮 → zoom 上行 → 主窗 HUD 出现即证明宿主内联 <script> 已执行。
  it("宿主内联 <script> 在预览中真实执行（CP-031，原 :87 skip 恢复）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-script-"));
    const htmlPath = join(tempDir, "script.html");
    // 无任何内联事件属性——缩放派发只能来自 <script> 段执行
    const scriptBody =
      `setTimeout(function(){for(var i=0;i<2;i++){document.dispatchEvent(` +
      `new WheelEvent('wheel',{deltaY:-120,ctrlKey:true,cancelable:true}))}},300)`;
    writeFileSync(
      htmlPath,
      `<!DOCTYPE html><html><head><title>script-fixture</title></head>` +
        `<body><h1>script fixture</h1><script>${scriptBody}</script></body></html>`,
      "utf8",
    );
    try {
      await spawnZoomPanel(tempDir, htmlPath, "script fixture");
      // 2 格 ×1.1² → 121%——宿主内联 <script> 不执行则永无缩放上行（超时失败）
      await waitHudText("121%");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("关闭 HTML 面板 → 预览窗口销毁出列（驱动句柄收缩）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-close-"));
    const htmlPath = join(tempDir, "close.html");
    writeFileSync(htmlPath, "<h1>close fixture</h1>", "utf8");
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      const panelId = "e2e-html-close-" + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: "htmlviewer",
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: htmlPath },
      );
      await waitPreviewDocContains(panelId, "close fixture");
      const label = previewWindowLabel(panelId);
      expect((await browser.getWindowHandles()).includes(label)).toBe(true);

      // 经 dockview 关闭面板 → PreviewFrame 卸载 → 预览窗口销毁（异步——轮询出列）
      await browser.execute((pid: string) => {
        window.__dockviewApi?.getPanel(pid)?.api.close();
      }, panelId);
      await browser.waitUntil(
        async () => !(await browser.getWindowHandles()).includes(label),
        { timeout: 10000, timeoutMsg: "关闭面板后预览窗口未出列" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe("HTML 面板 edit 态 Ctrl+滚轮字号", () => {
  it("render→edit 切换后：合成 WheelEvent → .cm-scroller 字号 14→15（EditorPanel 同语义）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-fontsize-"));
    const htmlPath = join(tempDir, "page.html");
    writeFileSync(htmlPath, "<h1>e2e 字号</h1>", "utf8");
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      const panelId = "e2e-html-fs-" + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: "htmlviewer",
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: htmlPath },
      );
      // 切 edit 形态 → CM 挂载（工具条带内切换条——切换即预览窗口销毁）
      await waitForPreviewWindow(panelId, 20000);
      await browser.waitUntil(
        async () =>
          await browser.execute(
            () =>
              Array.from(
                document.querySelectorAll<HTMLButtonElement>('[data-e2e="html-mode-edit"]'),
              ).some((el) => el.getClientRects().length > 0),
          ),
        { timeout: 15000, timeoutMsg: "html 切换条未出现" },
      );
      // 只点「可见」切换按钮——跨用例同页面残留面板（display:none）会遮蔽
      // querySelector 首元素（markdown.e2e 头注释实证；隐藏元素 click 无效）
      await browser.execute(() => {
        const btn = Array.from(
          document.querySelectorAll<HTMLButtonElement>('[data-e2e="html-mode-edit"]'),
        ).find((el) => el.getClientRects().length > 0);
        btn?.click();
      });
      // 预览窗口随 render 形态退出销毁（PreviewFrame 卸载）
      await browser.waitUntil(
        async () => !(await browser.getWindowHandles()).includes(previewWindowLabel(panelId)),
        { timeout: 10000, timeoutMsg: "切 edit 后预览窗口未销毁" },
      );
      await browser.waitUntil(
        async () =>
          await browser.execute(
            () =>
              Array.from(document.querySelectorAll(".cm-content")).some(
                (el) => el.getClientRects().length > 0,
              ),
          ),
        { timeout: 15000, timeoutMsg: "html edit 编辑器未挂载" },
      );
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
        { timeout: 10000, timeoutMsg: "html 编辑 Ctrl+滚轮未生效（字号未 14→15）" },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
      await switchToMainWindow();
    }
  });
});
