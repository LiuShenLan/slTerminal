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
 * - CS-3 用例 ②（hub 分派 + 保存 cliId 透传）：设置中心 hooks 配置页（settings 组件
 *   + 深链 selectedPage="hooks"，SC-E2E-02 适配）选择行渲染 mockcli
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
 *
 * - 第三 describe「mockcli 历史链路（CP-041 L4：展示 + 双击恢复注入）」：数据
 *   隔离语义——mockcli provider 扫描根 = run-wdio.cjs 每次重建的
 *   e2e-tests/.tmp-mockcli-projects 副本（SLTERM_MOCKCLI_PROJECTS_DIR 注入），
 *   不触真实 ~/.claude/projects；fixture 会话 cwd = E2E 临时项目目录
 *   （SLTERM_E2E_PROJECT_DIR 注入，占位符 __E2E_PROJECT_DIR__ 复制时替换），
 *   归属导航树 E2E 项目历史节点。fixture 与 UUID 常量逐字对应
 *   fixtures/mockcli-projects/（601 会话）。
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
  getActivePageInfo,
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
      // 1. 打开 nav 视图（NAV-08：活跃会话行承接方 = 导航树）。幂等打开——盲 toggle
      //    在 nav 已打开时会关闭（R2），而套件起始 beforeSuite resetSettings 已把
      //    sideBar 置回 DEFAULT_OPEN（top:"nav"）→ 盲 toggle 令 nav-tree 10s 不渲染
      //    （2026-09-08 CP-041 探针定责：suite 起始 nav 恒开）。照 history/agent
      //    spec 的 openNavView 形态改状态判定后打开。
      const navState = await browser.execute(
        () => (window as any).__slterm_e2e_getSideBarState?.() ?? null,
      );
      if (navState?.open?.top !== "nav") {
        await browser.execute(() => {
          (window as any).__slterm_e2e_toggleSideView?.("nav");
        });
      }
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
        { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
      );
      // 展开「当前活跃项目」行到会话行可见（CP-028 aria-expanded 探针单次确定性，
      // 两段式——2026-09-08 定责修正）：页面行随项目展开才渲染（React 异步提交），
      // 单 execute 内「展开项目行后立即遍历页面行」会拿到空容器（页面行从未被点击，
      // aria 恒 false → 树节点展开超时）。先展开项目行 + 等页面行渲染，再对容器内
      // 页面行各点击一次（每行至多一次点击，无奇偶翻转窗口；前提 = 行初始收起且
      // 同一 NavTree 挂载内不重复展开）。
      await browser.execute(() => {
        const proj = Array.from(
          document.querySelectorAll('[data-e2e="nav-row-project"]'),
        ).find((p) => (p.textContent ?? "").includes("当前"));
        if (!proj) return;
        if (proj.getAttribute("aria-expanded") !== "true") {
          (proj as HTMLElement).click();
        }
      });
      await browser.waitUntil(
        async () =>
          await browser.execute(() => {
            const proj = Array.from(
              document.querySelectorAll('[data-e2e="nav-row-project"]'),
            ).find((p) => (p.textContent ?? "").includes("当前"));
            if (!proj) return false;
            if (proj.getAttribute("aria-expanded") !== "true") return false;
            return !!proj.parentElement?.querySelector(
              '[data-e2e="nav-row-page"]',
            );
          }),
        { timeout: 5000, interval: 100, timeoutMsg: "项目行展开超时" },
      );
      await browser.execute(() => {
        const proj = Array.from(
          document.querySelectorAll('[data-e2e="nav-row-project"]'),
        ).find((p) => (p.textContent ?? "").includes("当前"));
        const container = proj?.parentElement as HTMLElement | null;
        if (!container) return;
        for (const pg of Array.from(
          container.querySelectorAll('[data-e2e="nav-row-page"]'),
        )) {
          if (pg.getAttribute("aria-expanded") !== "true") {
            (pg as HTMLElement).click();
          }
        }
      });
      await browser.waitUntil(
        async () =>
          await browser.execute(() => {
            const proj = Array.from(
              document.querySelectorAll('[data-e2e="nav-row-project"]'),
            ).find((p) => (p.textContent ?? "").includes("当前"));
            if (!proj) return false;
            const container = proj.parentElement as HTMLElement | null;
            if (!container) return false;
            if (proj.getAttribute("aria-expanded") !== "true") return false;
            return Array.from(
              container.querySelectorAll('[data-e2e="nav-row-page"]'),
            ).every((pg) => pg.getAttribute("aria-expanded") === "true");
          }),
        { timeout: 5000, interval: 100, timeoutMsg: "树节点展开超时" },
      );

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
   * CS-3 用例 ②（hub 分派 + 保存 cliId 透传）：打开设置中心面板（hooks 配置页）→ 选择行渲染
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

      // 0b. 关闭前序用例遗留的设置面板（mocha retries:1 重跑时旧面板残留 →
      //     多面板并存让首匹配断言命中间态面板——先关后开保证唯一，照 hooks.e2e.ts 先例）
      await browser.execute(() => {
        for (const p of window.__dockviewApi!.panels) {
          if (p.component === "settings") p.api.close();
        }
      });

      // 1. 程序化打开设置中心面板（设置中心形态，SC-E2E-02：settings 组件 + 深链
      //    selectedPage="hooks"；hub 容器 = 选择行 + 编辑器槽）
      const panelId = "settings-e2e-mockcli-" + Date.now();
      await browser.execute((pid: string) => {
        window.__dockviewApi!.addPanel({
          id: pid,
          component: "settings",
          title: "设置",
          params: { panelId: pid, selectedPage: "hooks" },
        });
      }, panelId);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-config-panel"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooks 配置页未就绪" },
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
      // 回收本用例打开的设置面板（照 hooks.e2e.ts 先例——重跑/后续用例
      // 从零开始）
      try {
        await browser.execute(() => {
          for (const p of window.__dockviewApi!.panels) {
            if (p.component === "settings") p.api.close();
          }
        });
      } catch { /* 忽略 */ }
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe("mockcli 历史链路（CP-041 L4：展示 + 双击恢复注入）", () => {
  // fixture 会话 UUID / 标题（与 fixtures/mockcli-projects/ 逐字对应）
  const MOCK_FIXTURE_UUID = "11111111-2222-4333-8444-555555555601";
  const MOCK_FIXTURE_TITLE = "mockcli 恢复目标会话";

  // run-wdio.cjs 注入：mockcli provider 扫描根副本 + E2E 临时项目目录
  // （数据隔离语义：扫描根 = .tmp-mockcli-projects 副本，不触真实 ~/.claude）
  const mockProjectsDir = process.env.SLTERM_MOCKCLI_PROJECTS_DIR;
  const e2eProjectDir = process.env.SLTERM_E2E_PROJECT_DIR;

  /** 打开 nav 视图（幂等：已打开不重复 toggle，防 R2 关闭）——照 history.e2e.ts */
  async function openNavView(): Promise<void> {
    const s = await browser.execute(
      () => (window as any).__slterm_e2e_getSideBarState?.() ?? null,
    );
    if (s?.open.top !== "nav") {
      await browser.execute(() =>
        (window as any).__slterm_e2e_toggleSideView?.("nav"),
      );
    }
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
      { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
    );
  }

  /**
   * 展开全部项目行至子容器可见（单次确定性，CP-028 同口径）：aria-expanded
   * 探针——只点击 aria-expanded !== "true" 的项目行，每行至多一次点击后
   * waitUntil 全展开，无奇偶翻转窗口。历史节点收在项目展开容器内（NAV-10），
   * 项目行收起时无 nav-history-node，须先展开。
   */
  async function expandAllProjectRows(): Promise<void> {
    await browser.execute(() => {
      for (const proj of Array.from(
        document.querySelectorAll('[data-e2e="nav-row-project"]'),
      ) as HTMLElement[]) {
        if (proj.getAttribute("aria-expanded") !== "true") {
          proj.click();
        }
      }
    });
    await browser.waitUntil(
      async () =>
        await browser.execute(() =>
          Array.from(document.querySelectorAll('[data-e2e="nav-row-project"]')).every(
            (p) => p.getAttribute("aria-expanded") === "true",
          ),
        ),
      { timeout: 5000, interval: 100, timeoutMsg: "项目行展开超时" },
    );
  }

  /**
   * 轮询展开全部未展开历史节点（aria-expanded 探针 + dataset 点击标记防
   * React 提交竞态双 toggle）并返回含指定标题的历史行快照（未命中 null）。
   * 历史节点仅当项目 total>0 渲染——数据未到前无节点，轮询天然等待扫描落地。
   */
  async function findHistoryRow(
    title: string,
  ): Promise<{ text: string; hasMockIcon: boolean } | null> {
    return browser.execute((t: string) => {
      const nodes = document.querySelectorAll(
        '[data-e2e="nav-history-node"]',
      ) as NodeListOf<HTMLElement>;
      for (const n of nodes) {
        if (n.getAttribute("aria-expanded") !== "true" && !n.dataset.e2eClicked) {
          n.dataset.e2eClicked = "1";
          n.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        }
      }
      const rows = document.querySelectorAll(
        '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
      ) as NodeListOf<HTMLElement>;
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) {
          return {
            text: r.textContent ?? "",
            hasMockIcon: Array.from(r.querySelectorAll("img")).some(
              (i) => (i.getAttribute("src") ?? "").includes("/cli-icons/mockcli.png"),
            ),
          };
        }
      }
      return null;
    }, title);
  }

  /**
   * 等 mockcli fixture 行出现：先被动等（依赖既有扫描数据），超时点 nav 头
   * 「刷新」钮（triggerNow 手动重扫——与定时 tick 共用同一执行体）后再等一轮。
   * 展开历史节点不触发扫描（FE-19），刷新钮是显式重扫的唯一入口。
   */
  async function waitForMockRow(): Promise<{ text: string; hasMockIcon: boolean }> {
    for (let phase = 0; phase < 2; phase++) {
      if (phase === 1) {
        // 首轮超时 → 点 nav 头「刷新」钮（triggerNow 手动重扫，FE-19：展开
        // 历史节点不触发扫描，刷新钮 = 显式重扫唯一入口）后再等一轮
        const clicked = await browser.execute(() => {
          const btn = document.querySelector(
            '[aria-label="刷新"]',
          ) as HTMLElement | null;
          btn?.click();
          return btn !== null;
        });
        expect(clicked).toBe(true);
      }
      const row = await browser
        .waitUntil(
          async () => (await findHistoryRow(MOCK_FIXTURE_TITLE)) as
            | { text: string; hasMockIcon: boolean }
            | undefined,
          {
            timeout: 15000,
            interval: 200,
            timeoutMsg: `mockcli 历史行「${MOCK_FIXTURE_TITLE}」未出现（扫描未落地或副本未就绪）`,
          },
        )
        .catch(() => undefined);
      if (row) return row;
    }
    throw new Error(
      `mockcli 历史行「${MOCK_FIXTURE_TITLE}」未出现（刷新重扫后仍未落地）`,
    );
  }

  /** 通用前置：注册 mockcli + 建 E2E 项目（fixture cwd 归属）+ nav 展开 */
  async function openMockHistoryView(): Promise<void> {
    await waitForWorkspaceReady();
    await registerMockCliProfile();
    if (!mockProjectsDir || !e2eProjectDir) {
      throw new Error(
        "SLTERM_MOCKCLI_PROJECTS_DIR / SLTERM_E2E_PROJECT_DIR 未注入——必须经 run-wdio.cjs 启动",
      );
    }
    const proj = await browser.execute((dir: string) => {
      return (window as any).__slterm_e2e_createProject?.(dir);
    }, e2eProjectDir);
    // 创建失败立即 fail——后续展示/恢复断言不得基于不存在的状态（TQ-E-04）
    if (!proj) {
      throw new Error(
        `__slterm_e2e_createProject 返回空（dir=${e2eProjectDir}）——helper 未就绪或创建失败`,
      );
    }
    await openNavView();
    await expandAllProjectRows();
  }

  it("mockcli 历史条目展示：provider 打标条目渲染于导航树历史节点（cliId=mockcli）", async () => {
    await openMockHistoryView();

    // 断言：nav-history-node 内行文本含 fixture 标题 + 行内 img src 含
    // "/cli-icons/mockcli.png"（NavHistoryRow 按 session.cliId 查 profile.iconSrc，
    // MC-311——mockcli profile 已注册，claude fixture 行图标不匹配不误中）
    const row = await waitForMockRow();
    expect(row.text).toContain(MOCK_FIXTURE_TITLE);
    expect(row.hasMockIcon).toBe(true);
  });

  it("mockcli 双击恢复：恢复编排 → 终端注入 `mockcli --resume <id>`（部分端到端，E2E-11）", async () => {
    await openMockHistoryView();
    await waitForMockRow();

    // 双击 fixture 行（普通行：cwd = E2E 临时项目目录，cwdExists=true）→
    // restoreHistorySession 四步编排（doRestore：项目匹配复用 → 页面切换 →
    // addPanel terminal → pty.write buildRestoreInput 注入）
    const dbl = await browser.execute((t: string) => {
      const rows = document.querySelectorAll(
        '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
      ) as NodeListOf<HTMLElement>;
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) {
          r.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
          return true;
        }
      }
      return false;
    }, MOCK_FIXTURE_TITLE);
    expect(dbl).toBe(true);

    // 1. 页面切换：activePage rootPath === fixture cwd（E2E 临时项目目录——
    //    restoreHistorySession 步骤 3 switchToPageShared）
    const info = await browser.waitUntil(
      async () => {
        const i = await getActivePageInfo();
        return i?.rootPath === e2eProjectDir ? i : false;
      },
      { timeout: 15000, timeoutMsg: "恢复后活跃页面 rootPath 未指向 E2E 项目目录" },
    );
    expect(info).not.toBeNull();

    // 2. 终端容器就绪（步骤 4 addPanel terminal → PTY session spawn）
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const containers = document.querySelectorAll(
            '[data-e2e="terminal-container"]',
          );
          for (const c of containers) {
            if ((c as any).__e2e_sessionReady) return true;
          }
          return false;
        }),
      { timeout: 25000, timeoutMsg: "恢复终端 PTY session 未就绪" },
    );

    // 3. 终端缓冲含注入命令（pty.write `mockcli --resume <id>\r`，pwsh 回显
    //    输入行——buildRestoreInput 桩输出，helpers.ts：608）
    await browser.waitUntil(
      async () =>
        await browser.execute((id: string) => {
          const containers = document.querySelectorAll(
            '[data-e2e="terminal-container"]',
          );
          for (const c of containers) {
            const el = c as any;
            if (
              typeof el.__e2e_getTerminalText === "function" &&
              el.__e2e_getTerminalText().includes(`mockcli --resume ${id}`)
            ) {
              return true;
            }
          }
          return false;
        }, MOCK_FIXTURE_UUID),
      { timeout: 25000, timeoutMsg: "终端缓冲未含 mockcli --resume 注入命令" },
    );

    // 4. 部分端到端：断言到「注入 + 编排」为止（E2E-11 标注）——mockcli 非真实
    //    CLI，不断言真实会话进入（真实恢复成功属人工验证，与 claude 系用例同边界）
  });
});
