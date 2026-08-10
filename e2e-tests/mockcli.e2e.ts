/**
 * mockcli profile E2E 冒烟（Stage 07 AC-4 ① 的 L4 侧，spec 06 §7）：
 * 经 E2E helper（__slterm_e2e_registerMockCliProfile，E2E_ENABLED 门控内注册）
 * 把 mock 夹具 profile 注册进 CliProfileRegistry → 终端注入 OSC 133 C
 * （__e2e_writeToTerminal 直接写 xterm 缓冲，走真实 parser + useCommandDetection
 *  → matchByCommand 命中 mockcli profile）→ 页签标题 "mockcli"（profile.tabTitle）
 * + 16×16 logo（profile.iconSrc）+ 🟡 attention 指示；OSC 133 D 退出恢复
 * （标题还原 + logo/图标双清）。
 *
 * 注入定位：terminal-container 挂 data-panel-id（TerminalPanel E2E 锚点）——app 启动
 * 会恢复用户布局的多终端面板（实测 ~30 个），全局首匹配会注入到用户残留面板与断言
 * 对象不一致；单用例（C→D 全链一个 it）共享 setup 面板，AC-4 ① 断言语义完整保留。
 *
 * mockcli 是测试夹具而非真实 CLI——仅测试环境注册（E2E helper），生产二进制
 * 无此 profile（E2E_ENABLED 内联字面量门控红线，见 e2e-tests/CLAUDE.md）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPtySessionReady,
  createProject,
  addTerminalPanel,
} from "./specUtils";

describe("mockcli profile 冒烟（AC-4 ①：OSC 133 命中页签/logo）", () => {
  /** helper 注册 mockcli profile（register 幂等——同 id 覆盖，重复调用安全） */
  async function registerMockCliProfile(): Promise<void> {
    await browser.execute(() => {
      (window as any).__slterm_e2e_registerMockCliProfile?.();
    });
  }

  /** 读取面板页签参数（tabIcon/tabLogo，undefined 归一 null） */
  async function getTabParams(
    panelId: string,
  ): Promise<{ tabIcon: string | null; tabLogo: string | null }> {
    return browser.execute((pid: string) => {
      const params = window.__dockviewApi?.getPanel(pid)?.params ?? {};
      return {
        tabIcon: params.tabIcon === undefined ? null : (params.tabIcon as string),
        tabLogo: params.tabLogo === undefined ? null : (params.tabLogo as string),
      };
    }, panelId);
  }

  /**
   * 向终端注入 OSC 133 序列（__e2e_writeToTerminal → term.write → 真实 parser）。
   * 序列形态照 shell-integration.ps1：`ESC ] 133;<payload> BEL`；xterm.js 剥离
   * OSC 编号后 handler 收到 "C;mockcli" / "D;0"（useCommandDetection 解析口径）。
   * 按 data-panel-id 精确定位目标面板容器——app 启动会恢复用户布局的多终端面板
   * （实测 ~30 个），全局首匹配会注入到用户残留面板，与用例面板断言对象不一致。
   */
  async function writeOsc133(panelId: string, payload: string): Promise<boolean> {
    return browser.execute((pid: string, data: string) => {
      const container = document.querySelector(
        `[data-e2e="terminal-container"][data-panel-id="${pid}"]`,
      ) as any;
      if (container && typeof container.__e2e_writeToTerminal === "function") {
        container.__e2e_writeToTerminal(`\x1b]133;${data}\x07`);
        return true;
      }
      return false;
    }, panelId, payload);
  }

  /** 共享 setup：注册 mockcli + 项目 + 终端面板 + PTY 就绪 */
  async function setupTerminal(): Promise<{ panelId: string; tempDir: string }> {
    await waitForWorkspaceReady();
    await registerMockCliProfile();
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-mockcli-"));
    const pageId = await createProject(tempDir);
    await waitForDockviewApi();
    const panelId = `terminal-${pageId}-0`;
    await addTerminalPanel(panelId);
    await waitForPtySessionReady();
    return { panelId, tempDir };
  }

  /** 读取面板标题（api.title，面板不存在返回 null） */
  async function getPanelTitle(panelId: string): Promise<string | null> {
    return browser.execute((pid: string) => {
      return window.__dockviewApi?.getPanel(pid)?.api.title ?? null;
    }, panelId);
  }

  it("注册 mockcli → OSC 133 C 命中（标题 mockcli + logo + 🟡）→ OSC 133 D 恢复", async () => {
    const { panelId, tempDir } = await setupTerminal();
    try {
      // ── OSC 133 C：命令命中 mockcli profile ──
      // 循环注入直到生效：waitForPtySessionReady 可能命中用户布局残留面板的 ready 标志，
      // 用例面板挂载/OSC 133 handler 注册可能未完成——一次性注入的 OSC 序列在 handler
      // 注册前被解析丢弃；每轮重新注入，handler 就绪后即命中。
      await browser.waitUntil(
        async () => {
          await writeOsc133(panelId, "C;mockcli");
          return (await getPanelTitle(panelId)) === "mockcli";
        },
        {
          timeout: 20000,
          timeoutMsg: `面板 ${panelId} 标题未在 OSC 133 C 注入后变为 "mockcli"`,
        },
      );

      // 页签 logo = profile.iconSrc "/cli-icons/mockcli.png" + 🟡 attention 指示
      // （TerminalPanel handleTabStateChange → updateParameters 更新 params.tabLogo/tabIcon）
      await browser.waitUntil(
        async () => {
          const p = await getTabParams(panelId);
          return p.tabLogo === "/cli-icons/mockcli.png" && p.tabIcon === "🟡";
        },
        { timeout: 10000, timeoutMsg: "页签 logo/🟡 未在 OSC 133 C 后出现" },
      );

      // ── OSC 133 D：命令退出恢复 ──
      // D → 命令退出：标题还原（非 mockcli，回 originalTitleRef）+ tabLogo/tabIcon 双清 null
      // （同样循环注入：OSC 133 D 仅当命令运行中才触发恢复分支）
      await browser.waitUntil(
        async () => {
          await writeOsc133(panelId, "D;0");
          const title = await getPanelTitle(panelId);
          const p = await getTabParams(panelId);
          return title !== "mockcli" && p.tabLogo === null && p.tabIcon === null;
        },
        {
          timeout: 20000,
          timeoutMsg: "OSC 133 D 后页签未还原（标题/logo/图标未清）",
        },
      );
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
