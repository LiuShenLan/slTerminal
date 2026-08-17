/**
 * mockcli profile E2E（Stage 07 AC-4 ① + Stage 05 review-fix CS-3）：
 *
 * - 冒烟（AC-4 ① 的 L4 侧，spec 06 §7）：经 E2E helper
 *   （__slterm_e2e_registerMockCliProfile，E2E_ENABLED 门控内注册）把 mock 夹具
 *   profile 注册进 CliProfileRegistry → 终端注入 OSC 133 C（__e2e_writeToTerminal
 *   直接写 xterm 缓冲，走真实 parser + useCommandDetection → matchByCommand 命中
 *   mockcli profile）→ 页签标题 "mockcli"（profile.tabTitle）+ 16×16 logo
 *   （profile.iconSrc）+ 🟡 attention 指示；OSC 133 D 退出恢复（标题还原 + logo/
 *   图标双清）。
 * - CS-3 用例 ①（agent-event 注入）：Node 侧原子写信号文件（cliId="mockcli"，
 *   事件经桩 eventToStatus 恒等映射 working）→ 页签 ⚡ + 导航树活跃区建行
 *   （真实 watcher → agent-event → resolvePayloadCliId 三级解析 → 桩策略全链路真实）。
 * - CS-3 用例 ②（hub 分派 + 保存 cliId 透传）：hooksConfig 面板选择行渲染 mockcli
 *   按钮（hasConfigEditor=true 过滤命中）→ 点击 → mock 编辑器桩渲染
 *   （data-e2e="mockcli-config-editor"）→ 桩内保存触发真实 writeHooksConfig
 *   ("mockcli", ...) → 后端「未知 cliId: mockcli」错误透传展示。
 *
 * 注入定位：terminal-container 挂 data-panel-id（TerminalPanel E2E 锚点）——app 启动
 * 会恢复用户布局的多终端面板（实测 ~30 个），全局首匹配会注入到用户残留面板与断言
 * 对象不一致。
 *
 * mockcli 是测试夹具而非真实 CLI——仅测试环境注册（E2E helper），生产二进制
 * 无此 profile（E2E_ENABLED 内联字面量门控红线，见 e2e-tests/CLAUDE.md）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPtySessionReady,
  createProject,
  addTerminalPanel,
  writeSignalFile,
  waitForSignalConsumed,
  waitForPanelTabStatus,
} from "./specUtils";

// ── 共享 helper（两个 describe 共用） ──

/** helper 注册 mockcli profile（register 幂等——同 id 覆盖，重复调用安全） */
async function registerMockCliProfile(): Promise<void> {
  await browser.execute(() => {
    (window as any).__slterm_e2e_registerMockCliProfile?.();
  });
}

/** 读取面板页签参数（tabStatus/tabLogo——IC-03 后状态字段为 tabStatus，undefined 归一 null） */
async function getTabParams(
  panelId: string,
): Promise<{ tabStatus: string | null; tabLogo: string | null }> {
  return browser.execute((pid: string) => {
    const params = window.__dockviewApi?.getPanel(pid)?.params ?? {};
    return {
      tabStatus: params.tabStatus === undefined ? null : (params.tabStatus as string),
      tabLogo: params.tabLogo === undefined ? null : (params.tabLogo as string),
    };
  }, panelId);
}

/**
 * 等待本面板 PTY session 就绪（容器级 __e2e_sessionReady，data-panel-id 精确匹配）。
 * 与 specUtils.waitForPtySessionReady 的区别：后者全局首匹配——app 启动恢复的
 * 用户布局残留面板（实测 ~30 个）先就绪会被其命中，用例面板 spawn 可能未完成。
 * 本面板 spawn 未完成时注入 OSC 133 C 的后果（NAV-10 实证）：register 在 spawn
 * 成功后才写入 TerminalRegistry（useXterm doSpawn），此前 setAgentSession no-op
 * （不 notify，tabLogo 永不写）；spawn 完成后 resetCommandState 又把 tabStatus
 * 清 null——页签状态被 spawn 初始化吞掉，冒烟用例必败。故注入 OSC 前必须先等
 * 本面板就绪。
 */
async function waitForPanelPtyReady(panelId: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      await browser.execute((pid: string) => {
        const el = document.querySelector(
          `[data-e2e="terminal-container"][data-panel-id="${pid}"]`,
        ) as any;
        return !!el && el.__e2e_sessionReady === true;
      }, panelId),
    { timeout: 25000, timeoutMsg: `面板 ${panelId} PTY session 未就绪` },
  );
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

describe("mockcli profile 冒烟（AC-4 ①：OSC 133 命中页签/logo）", () => {
  it("注册 mockcli → OSC 133 C 命中（标题 mockcli + logo + 🟡）→ OSC 133 D 恢复", async () => {
    const { panelId, tempDir } = await setupTerminal();
    try {
      // 等本面板 PTY 就绪（setupTerminal 的 waitForPtySessionReady 可能命中用户
      // 布局残留面板——本面板 spawn 未完成时注入 OSC，spawn 完成后的
      // register/resetCommandState 会吞掉 tabStatus/tabLogo，见 helper 注释）
      await waitForPanelPtyReady(panelId);

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

      // 页签 logo = profile.iconSrc "/cli-icons/mockcli.png" + attention 状态圆点
      // （TerminalPanel handleTabStateChange → updateParameters 更新 params.tabLogo/tabStatus）
      await browser.waitUntil(
        async () => {
          const p = await getTabParams(panelId);
          return p.tabLogo === "/cli-icons/mockcli.png" && p.tabStatus === "attention";
        },
        { timeout: 10000, timeoutMsg: "页签 logo/attention 圆点未在 OSC 133 C 后出现" },
      );

      // ── OSC 133 D：命令退出恢复 ──
      // D → 命令退出：标题还原（非 mockcli，回 originalTitleRef）+ tabLogo/tabStatus 双清 null
      // （同样循环注入：OSC 133 D 仅当命令运行中才触发恢复分支）
      await browser.waitUntil(
        async () => {
          await writeOsc133(panelId, "D;0");
          const title = await getPanelTitle(panelId);
          const p = await getTabParams(panelId);
          return title !== "mockcli" && p.tabLogo === null && p.tabStatus === null;
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

describe("mockcli 关键路径（CS-3：agent-event 注入 + hub 分派/保存 cliId 透传）", () => {
  /**
   * CS-3 用例 ①（agent-event 注入）：注册 mockcli → 终端面板 → Node 侧原子写
   * 信号文件（cliId="mockcli"、事件 PreToolUse 经桩 eventToStatus 恒等映射
   * working）→ 断言页签 ⚡ + 导航树活跃区建行。全链路真实：真实 watcher
   * （lib.rs setup 启动）→ agent-event 广播 → resolvePayloadCliId 三级解析
   * （payload cliId 显式命中，不依赖 claude hooks 注入）→ cliProfileRegistry
   * 查 mockcli → 桩策略。
   */
  it("注册 mockcli → 信号文件（cliId=mockcli）→ 页签 ⚡ + 导航树活跃行建行", async () => {
    const { panelId, tempDir } = await setupTerminal();
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const signalFiles: string[] = [];
    try {
      // 1. 打开 nav 视图（NAV-08：活跃会话行承接方 = 导航树）
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("nav");
      });
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
        { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
      );
      // 展开「当前活跃项目」的行到会话行可见（含「当前」pill 的项目容器内——
      // 页面行点击 = 切页 + 初始化 Dockview，点击其它项目页面行会把 activePageId
      // 切走致信号建行被拒；多轮收敛：单轮内点击不触发 React 重渲染，判定失真）
      for (let i = 0; i < 6; i++) {
        const clicked = await browser.execute(() => {
          let any = false;
          const proj = Array.from(
            document.querySelectorAll('[data-e2e="nav-row-project"]'),
          ).find((p) => (p.textContent ?? "").includes("当前"));
          if (!proj) return false;
          const container = proj.parentElement as HTMLElement | null;
          if (!container) return false;
          if (container.children.length <= 2) {
            (proj as HTMLElement).click();
            any = true;
          }
          const pages = container.querySelectorAll(
            '[data-e2e="nav-row-page"]',
          );
          for (const pg of pages) {
            if ((pg.parentElement?.children.length ?? 0) <= 1) {
              (pg as HTMLElement).click();
              any = true;
            }
          }
          return any;
        });
        if (!clicked) break;
        await new Promise((r) => setTimeout(r, 350));
      }

      // 2. 确保信号目录存在 + 原子写信号文件（9 字段契约，cliId 显式 "mockcli"——
      //    与既有 claude 系用例的信号构造同构，仅 cliId 键不同）
      mkdirSync(eventsDir, { recursive: true });
      signalFiles.push(
        writeSignalFile(eventsDir, {
          panelId,
          event: "PreToolUse",
          timestamp: Date.now(),
          sessionId: "e2e-mockcli-event",
          usageSourcePath: "",
          cwd: tempDir,
          toolName: "Bash",
          notificationType: null,
          cliId: "mockcli",
        }),
      );

      // 3. 页签 ⚡：真实 watcher → agent-event → resolvePayloadCliId（payload cliId
      //    显式命中 mockcli）→ 桩 eventToStatus → working
      await waitForPanelTabStatus(panelId, "working", 15000);

      // 4. 导航树活跃行建行：nav-row-session 出现且 data-panel-id 匹配 + 行内圆点
      //    （StatusDot 7px 圆形 div——NAV-10 契约：⚡ 断言改圆点存在性断言；
      //    useAgentStatus 经 TerminalRegistry sessionChange 订阅建行）
      await browser.waitUntil(
        async () => {
          const state = await browser.execute((pid: string) => {
            const rows = Array.from(
              document.querySelectorAll('[data-e2e="nav-row-session"]'),
            ) as HTMLElement[];
            const row = rows.find((r) => r.getAttribute("data-panel-id") === pid);
            if (!row) return { exists: false };
            return {
              exists: true,
              hasDot: Array.from(row.querySelectorAll("div")).some(
                (d) =>
                  (d as HTMLElement).style.borderRadius === "50%" &&
                  (d as HTMLElement).style.width === "7px",
              ),
            };
          }, panelId);
          return state.exists && state.hasDot;
        },
        { timeout: 15000, timeoutMsg: "导航树活跃行未建行（含圆点）" },
      );

      // 5. 信号文件被 watcher 消费（消失——notify 实时 + 3s 轮询兜底双路径）
      await waitForSignalConsumed(signalFiles[0]);
    } finally {
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  /**
   * CS-3 用例 ②（hub 分派 + 保存 cliId 透传）：打开 hooksConfig 面板 → 选择行渲染
   * mockcli 按钮（hasConfigEditor=true 过滤命中）→ 点击 → mock 编辑器桩渲染
   * （data-e2e="mockcli-config-editor"）→ 桩内保存动作触发真实
   * writeHooksConfig("mockcli", ...) → 后端「未知 cliId: mockcli」错误透传展示
   * （mockcli 无后端 provider，错误即 cliId 全链携带的证据）。
   */
  it("hub 选择行 mockcli 按钮 → 桩编辑器渲染 → 桩保存 → 后端「未知 cliId: mockcli」错误透传", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-mockcli-hub-"));
    try {
      // 0. Workspace 就绪 + 注册 mockcli（register 幂等——重复调用安全）
      await waitForWorkspaceReady();
      await registerMockCliProfile();
      await waitForDockviewApi();

      // 0b. 关闭前序用例遗留的 hooksConfig 面板（mocha retries:1 重跑时旧面板残留 →
      //     多面板并存让首匹配断言命中间态面板——先关后开保证唯一，照 hooks.e2e.ts 先例）
      await browser.execute(() => {
        for (const p of window.__dockviewApi!.panels) {
          if (p.component === "hooksConfig") p.api.close();
        }
      });

      // 1. 程序化打开 hooksConfig 面板（hub 容器 = 选择行 + 编辑器槽）
      const panelId = "hooksConfig-e2e-mockcli-" + Date.now();
      await browser.execute((pid: string) => {
        window.__dockviewApi!.addPanel({
          id: pid,
          component: "hooksConfig",
          title: "Hooks 配置",
          params: { panelId: pid },
        });
      }, panelId);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-config-panel"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooksConfig 面板未就绪" },
      );

      // 2. 选择行渲染 mockcli 按钮（hasConfigEditor=true 过滤命中——claude + mockcli
      //    两枚按钮；data-e2e="hooks-cli-{id}" 契约）
      const mockcliBtn = await browser.execute(() => {
        const btn = document.querySelector(
          '[data-e2e="hooks-cli-mockcli"]',
        ) as HTMLButtonElement | null;
        return btn
          ? { exists: true, text: btn.textContent ?? "", disabled: btn.disabled }
          : { exists: false, text: "", disabled: true };
      });
      expect(mockcliBtn.exists).toBe(true);
      expect(mockcliBtn.text).toContain("mockcli");

      // 3. 点击 mockcli 按钮（程序化 .click()——不触发 focusin，规避面板根容器
      //    focus 重读竞态，照 hooks.e2e.ts 注释先例；mockcli 桩无 dirty，切换无
      //    确认弹窗）
      const clicked = await browser.execute(() => {
        const btn = document.querySelector(
          '[data-e2e="hooks-cli-mockcli"]',
        ) as HTMLButtonElement | null;
        btn?.click();
        return btn !== null;
      });
      expect(clicked).toBe(true);

      // 4. 断言 mock 编辑器桩渲染（data-e2e="mockcli-config-editor"——helpers.ts
      //    桩与 L2 桩同标记口径，KZ-7 双向分派断言的 L4 侧）
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="mockcli-config-editor"]'))) === true,
        { timeout: 10000, timeoutMsg: "mockcli 桩编辑器未渲染" },
      );

      // 5. 桩内保存动作 → 真实 writeHooksConfig("mockcli", ...) → 后端
      //    Validation「未知 cliId: mockcli」→ 错误经桩 setState 透传展示
      await browser.execute(() => {
        const btn = document.querySelector(
          '[data-e2e="mockcli-config-save"]',
        ) as HTMLButtonElement | null;
        btn?.click();
      });
      await browser.waitUntil(
        async () => {
          const text = await browser.execute(() => {
            const e = document.querySelector('[data-e2e="mockcli-config-error"]');
            return e ? (e.textContent ?? "") : null;
          });
          return text !== null && text.includes("未知 cliId") && text.includes("mockcli");
        },
        { timeout: 15000, timeoutMsg: "桩保存未透传后端「未知 cliId: mockcli」错误" },
      );
    } finally {
      // 回收本用例打开的 hooksConfig 面板（照 hooks.e2e.ts 先例——重跑/后续用例
      // 从零开始）
      try {
        await browser.execute(() => {
          for (const p of window.__dockviewApi!.panels) {
            if (p.component === "hooksConfig") p.api.close();
          }
        });
      } catch { /* 忽略 */ }
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
