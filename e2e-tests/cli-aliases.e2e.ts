/**
 * CLI 别名（cliAliases 段）L4 冒烟 spec（D9）：
 *
 * 全链路真实：设置中心「CLI 别名」页添加别名 cc → store 即时生效（chip 出现）→
 * 2s debounce 真实落盘（Node 侧读 settings.json cliAliases.claude 含 cc）→
 * 终端注入 OSC 133 C "C;cc"（__e2e_writeToTerminal → 真实 parser → matchByCommand
 * 经注册表别名快照命中 claude profile）→ 页签标题 "claude"（profile.tabTitle）+
 * 16×16 logo（/cli-icons/claude.png）+ 🟡 attention；OSC 133 D 清理还原。
 *
 * 半端到端边界（DOC-02 先例，照 mockcli.e2e.ts）：注入的 OSC payload 与
 * shell-integration.ps1 Enter hook 发射序列同构——ps1 → shell → OSC 的发射链
 * 由 mockcli/terminal e2e 既有覆盖面承接，本 spec 验证的是别名机制的增量面：
 * 设置 → 快照 → 匹配。别名命中与内置命中走同一 handler 分支，L2 已锁差异。
 *
 * 写盘还原：本 spec 会真实写 exe 同级 settings.json（SLTERM_DATA_DIR 隔离下 =
 * run-wdio 临时目录）——suite 级快照还原 + 用例内删除别名自净，双保险。
 */

import { $, expect, browser } from "@wdio/globals";
import { mkdtempSync, rmSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPtySessionReady,
  createProject,
  addTerminalPanel,
} from "./specUtils";

// ── Window 全局类型扩展（与 settings.e2e.ts 同口径） ──

declare global {
  interface Window {
    __dockviewApi?: any;
    __slterm_e2e_openSettings?: () => Promise<void>;
    __slterm_e2e_switchSettingsPage?: (id: string) => boolean;
    __slterm_e2e_getSettingsPanelState?: () => { selectedPage: string | null } | null;
    __slterm_e2e_closeAllSettingsPanels?: () => void;
  }
}

/** 后端 settings.json：exe 同级 / SLTERM_DATA_DIR 隔离（app_dir.rs 便携分发契约） */
const settingsJsonPath = join(
  process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug"),
  "settings.json",
);

// ── 页面内 helper（照 settings.e2e.ts / mockcli.e2e.ts 先例） ──

/** 等待设置面板挂载 */
async function waitForSettingsPanel(timeout = 15000): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await browser.execute(() => !!document.querySelector('[data-e2e="settings-panel"]'))) === true,
    { timeout, timeoutMsg: "设置面板未就绪" },
  );
}

/** 经 helper 打开设置中心 + 切到 CLI 别名页 */
async function openCliAliasesPage(): Promise<void> {
  await browser.execute(() => (window as any).__slterm_e2e_openSettings?.());
  await waitForSettingsPanel();
  const ok = await browser.execute(
    () => (window as any).__slterm_e2e_switchSettingsPage?.("cliAliases") ?? false,
  );
  expect(ok).toBe(true);
  await browser.waitUntil(
    async () =>
      (await browser.execute(
        () => !!document.querySelector('[data-e2e="settings-cli-aliases-page"]'),
      )) === true,
    { timeout: 10000, timeoutMsg: "CLI 别名配置页未渲染" },
  );
}

/** 读取面板标题与页签参数（tabStatus/tabLogo——undefined 归一 null） */
async function getPanelState(
  panelId: string,
): Promise<{ title: string | null; tabStatus: string | null; tabLogo: string | null }> {
  return browser.execute((pid: string) => {
    const panel = window.__dockviewApi?.getPanel(pid);
    const params = panel?.params ?? {};
    return {
      title: panel?.api.title ?? null,
      tabStatus: params.tabStatus === undefined ? null : (params.tabStatus as string),
      tabLogo: params.tabLogo === undefined ? null : (params.tabLogo as string),
    };
  }, panelId);
}

/** 等待本面板 PTY session 就绪（容器级 __e2e_sessionReady，data-panel-id 精确匹配——
    全局首匹配可能命中布局残留面板，见 mockcli.e2e.ts helper 注释） */
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

/** 向终端注入 OSC 133 序列（__e2e_writeToTerminal → term.write → 真实 parser，
    序列形态照 shell-integration.ps1：ESC ] 133;<payload> BEL） */
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

/** 关闭全部 settings 面板（重跑/清理隔离） */
async function closeSettingsPanels(): Promise<void> {
  await browser.execute(() => (window as any).__slterm_e2e_closeAllSettingsPanels?.());
}

/** 等待后端 settings.json 的 cliAliases.claude 别名集合满足条件（原子写中间态重试） */
async function waitForAliasOnDisk(
  predicate: (aliases: string[] | undefined) => boolean,
  timeout = 10000,
): Promise<void> {
  await browser.waitUntil(
    () => {
      try {
        if (!existsSync(settingsJsonPath)) return false;
        const root = JSON.parse(readFileSync(settingsJsonPath, "utf8")) as Record<string, unknown>;
        const section = root.cliAliases as { claude?: unknown } | undefined;
        const claude = section?.claude;
        return predicate(Array.isArray(claude) ? (claude as string[]) : undefined);
      } catch {
        return false;
      }
    },
    { timeout, timeoutMsg: "cliAliases.claude 磁盘条件未满足" },
  );
}

describe("CLI 别名（cliAliases 段，D9 冒烟）", function () {
  // 长链用例放宽 mocha 预算（suite 级，须在用例执行前生效——@wdio/utils executeAsync
  // 于用例体开始前采样 runnable 超时，体内 this.timeout() 无效，2026-09-08 实测）：
  // 真实手势每 $/click 触发 tauri-service 窗口态查询，查询通道在本 app 构建不可用 →
  // 每 focus 命令确定性 +5s，默认 60s 预算不足
  this.timeout(120000);
  // suite 级快照还原（真实写盘 exe 同级 settings.json——见文件头注释）
  let settingsSnapshot: { existed: boolean; content: string | null };

  before(() => {
    settingsSnapshot = {
      existed: existsSync(settingsJsonPath),
      content: existsSync(settingsJsonPath) ? readFileSync(settingsJsonPath, "utf8") : null,
    };
  });

  after(() => {
    try {
      if (settingsSnapshot.existed) {
        writeFileSyncSafe(settingsJsonPath, settingsSnapshot.content ?? "");
      } else {
        rmSync(settingsJsonPath, { force: true });
      }
    } catch (err) {
      console.warn("[cli-aliases.e2e] 还原 settings.json 失败:", err);
    }
  });

  it("添加别名 cc → 落盘 → 终端 OSC 133 C 命中别名（页签 claude + logo + 🟡）→ D 清理", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-cli-aliases-"));
    let pid: string | null = null;
    try {
      // ── 1. 打开 CLI 别名设置页（真实用户入口 + 注册表驱动分区渲染） ──
      await waitForWorkspaceReady();
      // 先建项目再等 Dockview API：空数据目录启动无默认项目/页面 → Dockview 不挂载、
      // __dockviewApi 恒 undefined（2026-09-08 CP-030 排查实证；agent/mockcli 均按
      // createProject 在前、waitForDockviewApi 在后的序排）——页面随项目创建而挂载
      const pageId = await createProject(tempDir);
      await waitForDockviewApi();
      await openCliAliasesPage();
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="cli-aliases-group-claude"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "CLI 别名页 claude 分区未渲染" },
      );

      // ── 2. 添加别名 cc（输入 + 添加按钮）→ chip 即时出现（内存态） ──
      await $('[data-e2e="cli-aliases-input-claude"]').setValue("cc");
      await $('[data-e2e="cli-aliases-add-claude"]').click();
      // 交互时序断言(CP-030 回归):真实指针序列(mousedown→input blur→mouseup→click)
      // 驱动——blur 不清空(CliAliasesPage blur 语义)+ 成功提交清空输入两语义同时落位。
      const inputAfterAdd = await $('[data-e2e="cli-aliases-input-claude"]');
      expect(await inputAfterAdd.getValue()).toBe("");
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="cli-aliases-alias-claude-cc"]'),
          )) === true,
        { timeout: 8000, timeoutMsg: "添加后别名 chip 未出现（store 未生效）" },
      );

      // ── 3. 2s debounce 真实落盘（store subscribe → save_settings → settings.json） ──
      await waitForAliasOnDisk(
        (aliases) => aliases !== undefined && aliases.includes("cc"),
        10000,
      );

      // ── 4. 终端面板：OSC 133 C 别名命中（注册表快照实时生效，无需重启） ──
      // CP-004 panelId 页前缀协议：直插 id 与 params.panelId 均为 "{pageId}:{localId}"
      // 形态——无前缀面板 pageOfPanelId=null，终端可见判定/面板归属链全部失效
      await closeSettingsPanels();
      pid = `${pageId}:terminal-${Date.now()}`;
      await addTerminalPanel(pid);
      await waitForPtySessionReady();
      await waitForPanelPtyReady(pid);

      // 循环注入直到生效（handler 注册/面板挂载竞态兜底，照 mockcli.e2e.ts 先例）
      await browser.waitUntil(
        async () => {
          await writeOsc133(pid!, "C;cc run --flag");
          const s = await getPanelState(pid!);
          return s.title === "claude";
        },
        { timeout: 20000, timeoutMsg: "OSC 133 C 别名 cc 未命中（标题未变 claude）" },
      );

      // 页签 logo = claude profile iconSrc + attention 状态圆点
      await browser.waitUntil(
        async () => {
          const s = await getPanelState(pid!);
          return s.tabLogo === "/cli-icons/claude.png" && s.tabStatus === "attention";
        },
        { timeout: 10000, timeoutMsg: "别名命中后 logo/attention 圆点未出现" },
      );

      // ── 5. OSC 133 D 清理还原（标题还原 + logo/状态双清） ──
      await browser.waitUntil(
        async () => {
          await writeOsc133(pid!, "D;0");
          const s = await getPanelState(pid!);
          return s.title !== "claude" && s.tabLogo === null && s.tabStatus === null;
        },
        { timeout: 20000, timeoutMsg: "OSC 133 D 后页签未还原（标题/logo/状态未清）" },
      );
    } finally {
      // ── 自净：删除别名 + 等待落盘清空（防残留污染后续 spec） ──
      try {
        await $('[data-e2e="cli-aliases-remove-claude-cc"]').click();
        await waitForAliasOnDisk(
          (aliases) => aliases === undefined || !aliases.includes("cc"),
          10000,
        ).catch(() => {});
      } catch { /* 忽略：面板可能已不在 */ }
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });
});

/** 写文件（catch 由调用方处理）——settings.json 由后端原子写（NamedTempFile），
    此处为测试还原——不覆盖 .bak（若存在）保持与后端恢复语义一致 */
function writeFileSyncSafe(p: string, content: string): void {
  writeFileSync(p, content, "utf8");
}
