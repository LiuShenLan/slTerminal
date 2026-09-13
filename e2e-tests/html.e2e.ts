/**
 * HTML 面板域 E2E spec（E2E-09 拆分；ADR-0021 预览回迁主窗 DOM 后重写）：
 * 主窗口快捷键关闭链路、Ctrl+滚轮缩放（注入运行时执行于宿主 iframe 内
 * 嵌套 srcdoc 文档；HUD/工具条带在主窗 DOM）、宿主内联 <script> 执行
 * （CP-031 原 :87 skip 用例恢复）、面板关闭/形态切换的宿主 iframe 生命周期。
 *
 * 驱动契约（ADR-0021 + spike Q4 裁决）：预览渲染于主窗内跨源沙箱 iframe
 * （data-e2e="preview-frame-<panelId>"）——embedded driver frame 内 execute
 * 全灭（e2e-tests/CLAUDE.md 外部坑登记），内容断言走主窗 E2E 探针全局
 * （__slterm_e2e_previewDoc/__slterm_e2e_iframeLoaded，specUtils
 * waitPreviewDocContains 封装）；HUD/面板断言直读主窗 DOM。
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
  activatePanel,
  clickInPanel,
  waitPreviewDocContains,
  waitForPreviewFrame,
  waitForPreviewFrameGone,
} from "./specUtils";

/**
 * 关闭 dockview 面板（重试直至面板消失，2026-09-08 实测语义）：CP-004 共享
 * 宿主下 dockview close 存在瞬态抖动面——面板挂载/页组最大化等布局 mutation
 * 未收敛时首轮 api.close 被吞（面板仍在，400ms 后重试即成功）；Ctrl+W 用例
 * 「轮询派发直到消失」同型自我修复（md 关闭用例注释同口径）。面板不存在时
 * 安全 no-op。返回面板是否已消失。
 */
async function closePanelRetryGone(panelId: string): Promise<boolean> {
  for (let attempt = 0; attempt < 20; attempt++) {
    const gone = await browser.execute((pid: string) => {
      const panel = window.__dockviewApi?.getPanel(pid);
      if (!panel) return true;
      panel.api.close();
      return false;
    }, panelId);
    if (gone) return true;
    await browser.pause(300);
  }
  // 重试耗尽——末轮 close 后最终确认一次（面板可能已消失）
  return browser.execute((pid: string) => !window.__dockviewApi?.getPanel(pid), panelId);
}

/**
 * 用例终局卫生：面板关闭 + 宿主 iframe 移除确认（预览 = 主窗 DOM，随面板
 * 卸载同步移除——残留即 React 卸载链缺陷信号）。面板关闭走 dockview api.close
 * （各用例独立建项目/页，close 安全）；面板不存在时 no-op。卫生清理失败不
 * 掩盖用例本体结果。（模块顶层导出——缩放与字号两个 describe 共用，勿下移
 * 入 describe 作用域）
 */
async function closePanelAndWaitGone(panelId: string): Promise<void> {
  try {
    await closePanelRetryGone(panelId);
    await waitForPreviewFrameGone(panelId);
  } catch {
    /* 卫生清理失败不掩盖用例本体结果 */
  }
}

describe("HTML 面板主窗口快捷键关闭（Ctrl+W）", () => {
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

      // 等待渲染内容推送并经桥加载完成（探针组合）
      await waitPreviewDocContains(panelId, "e2e html");

      // 主窗口合成 Ctrl+W（ShortcutRegistry window capture 消费 → global.closeTab
      // → 活跃面板关闭——CP-004 单宿主契约：多页组面板并存时 dockview 活跃面板
      // 不随 addPanel 归属，先显式激活本面板再派发，杜绝关闭残留面板）
      expect(await activatePanel(panelId)).toBe(true);
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
 * 背景：预览渲染于主窗内跨源沙箱宿主 iframe 的嵌套 srcdoc 文档（ADR-0021，
 * 宿主页 = 自定义协议域——域级 CSP 由宿主页 meta 承载，SEC-02：内联
 * script/style 与 img/font data: 放行），fixture 内联事件属性/脚本真实执行；
 * 注入运行时（zoomRuntime wheel capture）先于 fixture 注册（注入段插于
 * </head> 前）→ 合成 WheelEvent → zoom → 上行（经宿主桥 relay）→ 主窗 HUD
 * （工具条带）。embedded 驱动无法投递 OS 滚轮——fixture 自派发。
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

  /** 建项目并打开 htmlviewer 面板，等待预览内容渲染（探针组合）；返回 panelId */
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
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath);
      // 等比 ×1.1³ ≈ 1.331 → "133%"
      await waitHudText("133%");
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("点重置 → 下行复位 → 再次缩放基于 100% 重算（下行往返验证）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    // 阶段 A：onload 立即 2 格 → 121%；阶段 B：3500ms 后 2 格（供重置后二次断言——
    // B 需晚于重置完成）
    writeFileSync(htmlPath, stagedFixture(400, 2, 3500, 2), "utf8");
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath);
      // 轮 1：2 格 → 121%
      await waitHudText("121%");
      // 点重置（主窗 HUD 按钮 → 下行经宿主桥 relay → iframe 归 1，真实往返）
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
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("iframe 不重建时缩放会话保留（后续缩放基于上次值叠加）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    // 阶段 A：onload 立即 2 格 → 121%；阶段 B：2200ms 后 1 格
    writeFileSync(htmlPath, stagedFixture(400, 2, 2200, 1), "utf8");
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath);
      await waitHudText("121%");
      // 文档存活时闭包 zoom 保留：+1 格 → ×1.1 → 133%（若 iframe 重建归 1 则 110%）
      await waitHudText("133%");
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ── 宿主内联 <script> 真实执行（CP-031 消亡判定三的 e2e 通道）──
  // S10-② 前：injectScript escapeScriptClose 把宿主 `</script>` 全部转义 →
  // 宿主 script 吞到 EOF 永不执行（存量缺陷，原 :87 skip 空壳登记的根因）；
  // 且宿主文档继承主窗 CSP（'unsafe-inline' 依赖）。S10-② 后：宿主 <script> 段
  // 不经字符串转义进入渲染文档（escapeScriptClose 消亡），且渲染于预览域
  // （自定义协议宿主页 iframe，无全局 CSP——域级 meta CSP 放行内联 script）——内联 <script> 真实可执行。
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
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath, "script fixture");
      // 2 格 ×1.1² → 121%——宿主内联 <script> 不执行则永无缩放上行（超时失败）
      await waitHudText("121%");
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ── keyfwd 收窄转发（ADR-0021/D2）真实往返 ──
  // 预览内容 iframe 内 keydown（表单焦点除外）→ slterm_keyfwd 上行（经宿主桥
  // relay）→ 主窗 ShortcutRegistry global context 解析消费。fixture 内联
  // <script> 在预览域真实执行（CP-031）——脚本合成 keydown 即触发内容侧
  // keyForward 段捕获上行，链路 = 真实 OS 键入的等价物（半端到端边界内最强
  // 形态：embedded 驱动无法投递 OS 键）。
  it("keyfwd：预览内合成 Ctrl+W → 上行主窗 global.closeTab 消费（面板关闭）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-keyfwd-"));
    const htmlPath = join(tempDir, "keyfwd.html");
    // 延时派发（等 activatePanel 落定——keyfwd 消费的是上行时点的活跃面板，
    // 1500ms 窗口保 activatePanel 必先完成；注入段插 </head> 前已先注册）
    const scriptBody =
      `setTimeout(function(){document.dispatchEvent(new KeyboardEvent('keydown',` +
      `{ctrlKey:true,code:'KeyW',key:'w',bubbles:true,cancelable:true}))},1500)`;
    writeFileSync(
      htmlPath,
      `<!DOCTYPE html><html><head><title>keyfwd-fixture</title></head>` +
        `<body><h1>keyfwd fixture</h1><script>${scriptBody}</script></body></html>`,
      "utf8",
    );
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath, "keyfwd fixture");
      // 预览内 keydown（target=document，非表单）→ keyfwd 上行 → 主窗
      // resolve(ev,"global") → global.closeTab → 活跃面板关闭。先显式激活
      // 本面板（CP-004 单宿主契约：活跃面板不随 addPanel 归属）
      expect(await activatePanel(panelId)).toBe(true);
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (pid: string) => window.__dockviewApi?.getPanel(pid) === undefined,
            panelId,
          ),
        { timeout: 10000, timeoutMsg: "预览内 keyfwd Ctrl+W 未触发面板关闭" },
      );
      panelId = undefined; // 面板已被 keyfwd 链路关闭——免卫生清理
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("keyfwd 负面：焦点在表单元素（input）不转发——面板保持打开", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-keyfwd-form-"));
    const htmlPath = join(tempDir, "keyfwd-form.html");
    // input.focus() 后于 input 上派发 Ctrl+W——keyForward 段见 target 表单跳过
    // 转发；若误转发面板将被关闭（反向判定）
    const scriptBody =
      `setTimeout(function(){var i=document.getElementById('f');i.focus();` +
      `i.dispatchEvent(new KeyboardEvent('keydown',` +
      `{ctrlKey:true,code:'KeyW',key:'w',bubbles:true,cancelable:true}))},1500)`;
    writeFileSync(
      htmlPath,
      `<!DOCTYPE html><html><head><title>keyfwd-form-fixture</title></head>` +
        `<body><h1>keyfwd form fixture</h1><input id="f" type="text">` +
        `<script>${scriptBody}</script></body></html>`,
      "utf8",
    );
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath, "keyfwd form fixture");
      expect(await activatePanel(panelId)).toBe(true);
      // 等派发窗口期过后断言面板仍在（1500ms 派发 + 链路余量，取 3.5s）
      await browser.pause(3500);
      const alive = await browser.execute(
        (pid: string) => window.__dockviewApi?.getPanel(pid) !== undefined,
        panelId,
      );
      expect(alive).toBe(true);
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("关闭 HTML 面板 → 宿主 iframe 从主窗 DOM 移除（ADR-0021 生命周期）", async () => {
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
      await waitForPreviewFrame(panelId);

      // 经 dockview 关闭面板（重试至面板消失——首轮 close 可能被布局 mutation
      // 瞬态吞掉，见 closePanelRetryGone）→ PreviewFrame 卸载 → 宿主 iframe
      // 随 React 卸载同步移除（预览 = 主窗 DOM，无独立窗口生命周期）
      expect(await closePanelRetryGone(panelId)).toBe(true);
      await waitForPreviewFrameGone(panelId);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ── 页前缀协议 panelId（{pageId}:{localId}）链路用例 ──
  // 生产 panelId = "{pageId}:{localId}"（openFile.ts 经 panelIdInPage 产出）——
  // ADR-0021 后 panelId 承载于宿主 iframe data-e2e 属性与探针全局键（含 ":"），
  // 旧独立窗口的 validate_label 校验面整体消亡。本用例以页前缀形态 id 经生产
  // 打开链路（addPanel，模拟 openFile 真实产出）建面板：data-e2e 属性选择器
  // 可定位 + 探针键可读 + 内容渲染完成 = 含 ":" id 的端到端链路证据。
  it("页前缀协议 panelId（{pageId}:{localId}）→ 预览链路全通", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-pageprefix-"));
    const htmlPath = join(tempDir, "page-prefix.html");
    writeFileSync(htmlPath, "<h1>page-prefix fixture</h1>", "utf8");
    let panelId: string | undefined;
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      // 页前缀协议形态（openFile.ts 真实产出同构："{pageId}:{localId}"）
      panelId = "page-1:html-e2e-" + Date.now();
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
      // data-e2e 属性含 ":" 可定位（属性选择器引号内合法）+ 探针键同形可读
      await waitForPreviewFrame(panelId);
      await waitPreviewDocContains(panelId, "page-prefix fixture");
    } finally {
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe("HTML 面板 edit 态 Ctrl+滚轮字号", () => {
  it("render→edit 切换后：合成 WheelEvent → .cm-scroller 字号 14→15（EditorPanel 同语义）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-fontsize-"));
    const htmlPath = join(tempDir, "page.html");
    writeFileSync(htmlPath, "<h1>e2e 字号</h1>", "utf8");
    let panelId: string | undefined;
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      panelId = "e2e-html-fs-" + Date.now();
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
      // 切 edit 形态 → CM 挂载（工具条带内切换条——切换即宿主 iframe 移除）。
      // CP-004 单宿主契约：残留页组面板仍留 DOM 且 getClientRects > 0，旧
      // 「只点可见切换按钮」过滤失效——clickInPanel 以本面板页签为锚点，在
      // 本面板组内容树内点击（杜绝命中残留面板的切换条切错对象）
      await waitForPreviewFrame(panelId, 20000);
      const clicked = await clickInPanel(panelId, '[data-e2e="html-mode-edit"]');
      expect(clicked).toBe(true);
      // 宿主 iframe 随 render 形态退出移除（PreviewFrame 卸载）
      await waitForPreviewFrameGone(panelId);
      await browser.waitUntil(
        async () =>
          await browser.execute(
            (pid: string) => {
              // 本面板内容树内的 CM（跨用例残留面板 CM 同样留 DOM——锚定过滤）
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
        { timeout: 15000, timeoutMsg: "html edit 编辑器未挂载" },
      );
      await browser.execute((pid: string) => {
        // 合成 Ctrl+wheel 到本面板 CM（锚定同上——editorFontSize 为共享 store，
        // 只对本面板内容派发防污染其它面板字号断言）
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
        { timeout: 10000, timeoutMsg: "html 编辑 Ctrl+滚轮未生效（字号未 14→15）" },
      );
    } finally {
      // 面板关闭卫生——本用例宿主 iframe 已随 edit 切换移除，关闭面板防残留
      // dockview 面板污染后续 spec（见 closePanelAndWaitGone）
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
