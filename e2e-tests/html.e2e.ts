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
  activatePanel,
  clickInPanel,
  waitPreviewDocContains,
  waitForPreviewWindow,
  switchToMainWindow,
  previewWindowLabel,
} from "./specUtils";

/**
 * 等待窗口句柄「连续多次采样」不在集合（销毁验证诚实化——2026-09-08）：
 * destroy 为异步（注册表滞后瞬时残影），且修复 D 落定前「销毁后复活」僵尸
 * （迟到 sync 重建）会让窗口在 ~100ms 后再次入列——单次采样缺席可能命中
 * 瞬态误判通过；连续缺席采样（间隔 ~150ms，≥3 次）跨过复活窗口期，
 * 窗口稳定出列才算通过。
 */
async function waitWindowGoneFromHandles(label: string, timeout = 10000): Promise<void> {
  let absentStreak = 0;
  await browser.waitUntil(
    async () => {
      const present = (await browser.getWindowHandles()).includes(label);
      absentStreak = present ? 0 : absentStreak + 1;
      return absentStreak >= 3;
    },
    { timeout, interval: 150, timeoutMsg: `预览窗口未稳定出列（${label}）` },
  );
}

/**
 * 关闭 dockview 面板（重试直至面板消失，2026-09-08 实测语义）：CP-004 共享
 * 宿主下 dockview close 存在瞬态抖动面——面板挂载/页组最大化等布局 mutation
 * 未收敛时首轮 api.close 被吞（面板仍在，400ms 后重试即成功）；Ctrl+W 用例
 * 「轮询派发直到消失」同型自我修复（md 关闭用例注释同口径）。面板不存在时
 * 安全 no-op。返回面板是否已消失。
 */
async function closePanelRetryGone(panelId: string): Promise<boolean> {
  await switchToMainWindow();
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
 * 用例终局卫生（2026-09-08 修复 A）：预览窗口按产品语义保活常驻（面板卸载
 * 才销毁）——html spec 各缩放/字号用例自建 dockview 面板但从不关闭，spec
 * 结束残留 5 个预览 WebviewWindow；每 spec 新 WebDriver session 默认窗口 =
 * 后端 webview_windows() HashMap first()（无序），残留窗致后续 spec 会话落
 * 非 main 上下文（无 helpers → 探针/reset 级联失效，wdio.conf beforeSuite
 * 已归位 main 兜底）。面板关闭走 dockview api.close（各用例独立建项目/页，
 * close 安全）；面板不存在时 no-op。卫生清理失败不掩盖用例本体结果。
 * （模块顶层导出——缩放与字号两个 describe 共用，勿下移入 describe 作用域）
 */
async function closePanelAndWaitGone(panelId: string): Promise<void> {
  try {
    await closePanelRetryGone(panelId);
    await waitWindowGoneFromHandles(previewWindowLabel(panelId));
  } catch {
    /* 卫生清理失败不掩盖用例本体结果 */
  }
}

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
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath);
      // 等比 ×1.1³ ≈ 1.331 → "133%"
      await waitHudText("133%");
    } finally {
      // 面板关闭卫生——防残留预览窗口污染后续 spec 会话默认窗口（见 closePanelAndWaitGone）
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("点重置 → 下行复位 → 再次缩放基于 100% 重算（下行往返验证）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-html-zoom-"));
    const htmlPath = join(tempDir, "zoom.html");
    // 阶段 A：onload 立即 2 格 → 121%；阶段 B：3500ms 后 2 格（供重置后二次断言——
    // 迁移 webview 后首轮 HUD/重置链路耗时更长，B 需晚于重置完成）
    writeFileSync(htmlPath, stagedFixture(400, 2, 3500, 2), "utf8");
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath);
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
    let panelId: string | undefined;
    try {
      panelId = await spawnZoomPanel(tempDir, htmlPath, "script fixture");
      // 2 格 ×1.1² → 121%——宿主内联 <script> 不执行则永无缩放上行（超时失败）
      await waitHudText("121%");
    } finally {
      // 面板关闭卫生——防残留预览窗口污染后续 spec 会话默认窗口（见 closePanelAndWaitGone）
      if (panelId) await closePanelAndWaitGone(panelId);
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

      // 经 dockview 关闭面板（重试至面板消失——首轮 close 可能被布局 mutation
      // 瞬态吞掉，见 closePanelRetryGone）→ PreviewFrame 卸载 → 预览窗口销毁
      // （异步——轮询出列）。销毁验证须跨瞬态（诚实化，2026-09-08）：单次采样
      // 缺席可能命中 destroy 后注册表滞后瞬态——通过的是瞬态，窗口 100ms 后
      // 复活入列会再次出现；连续多次采样缺席才算稳定出列（修复 D 落定后复活
      // 不再发生，连续采样语义保留防误判）
      expect(await closePanelRetryGone(panelId)).toBe(true);
      await waitWindowGoneFromHandles(label);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ── SEC-01 防复发：页前缀协议 id 走真实 label 路径 ──
  // 生产 panelId = "{pageId}:{localId}"（openFile.ts:98-100 经 panelIdInPage
  // 产出）→ 预览 label = preview-<panelId> 必含 ":"；修复前后端 validate_label
  // 仅放行字母数字/_/-，预览链路四命令（preview_sync/close/render/pull）全被
  // Err(Validation) 静默拒绝——既有用例的裸 id 直注形态（各自测不同面）掩盖此
  // 缺陷。本用例以页前缀形态 id 经生产打开链路（addPanel，模拟 openFile 真实
  // 产出）建面板：label 含 ":" 能入列并推送内容 = validate_label 放行 ":" 的
  // 端到端证据（修复前此处 waitPreviewDocContains 必超时）。裸 id 用例保留不动。
  it("页前缀协议 panelId（{pageId}:{localId}）→ 预览链路全通（SEC-01 防复发）", async () => {
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
      // 预览窗口 label = preview-page-1:html-e2e-<ts>——validate_label 放行 ":"
      // 才可能入列并推送到内容
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
      // 切 edit 形态 → CM 挂载（工具条带内切换条——切换即预览窗口销毁）。
      // CP-004 单宿主契约：残留页组面板仍留 DOM 且 getClientRects > 0，旧
      // 「只点可见切换按钮」过滤失效——clickInPanel 以本面板页签为锚点，在
      // 本面板组内容树内点击（杜绝命中残留面板的切换条切错对象）
      await waitForPreviewWindow(panelId, 20000);
      const clicked = await clickInPanel(panelId, '[data-e2e="html-mode-edit"]');
      expect(clicked).toBe(true);
      // 预览窗口随 render 形态退出销毁（PreviewFrame 卸载）
      await browser.waitUntil(
        async () => !(await browser.getWindowHandles()).includes(previewWindowLabel(panelId)),
        { timeout: 10000, timeoutMsg: "切 edit 后预览窗口未销毁" },
      );
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
      // 面板关闭卫生——本用例预览窗口已随 edit 切换销毁，关闭面板防残留
      // dockview 面板 + 潜在预览窗口污染后续 spec（见 closePanelAndWaitGone）
      if (panelId) await closePanelAndWaitGone(panelId);
      rmSync(tempDir, { recursive: true, force: true });
      await switchToMainWindow();
    }
  });
});
