/**
 * 设置中心（F11）L4 spec（SC-E2E-02）：
 *
 * 覆盖设置中心面板全链路：配置钮打开/单例、选中配置页 params 持久化、套餐余量频率页
 * 真实后端落盘（plan_balance_set_interval 命令真实 invoke——L4 兜底 SC-BE-03
 * 「命令未注册被前端 catch 吞 = 测试全绿但运行时静默失败」盲区）、快捷键录制（合成
 * KeyboardEvent 全链路：录制态 → setBinding → 2s debounce 落盘）、切项目自动关闭、
 * 同项目切页保留、hooks 页迁入冒烟（设置中心内 CLI 选择行渲染）、dirty 切页守卫
 * （confirmDialog 取消不切换）、× 关闭 dirty 守卫（取消保留 / 确认关闭）。
 *
 * corrupted 警示条不做 L4（L2 覆盖 loadSettings mock；无沙箱外写坏文件通道——
 * 豁免登记归 Stage 07 test-inventory 同步）。
 *
 * 关键约定：
 * - 打开 = 活动栏配置钮真实点击（data-e2e="activity-btn-config"）或等价 helper
 *   __slterm_e2e_openSettings（SC-E2E-01 落地）；面板状态/计数经 helper 读
 *   __dockviewApi（活跃页面 api）settings- 前缀面板 params.selectedPage。
 * - 后端 settings.json 在 exe 同级（app_dir.rs 便携分发契约）——落盘断言直接
 *   Node 侧读文件（与 loadSettings 同一真值源），原子写中间态自动重试。
 * - 用例写盘（④ planBalance 段 / ⑥ keybindings 段 / ④ 假 env 注入 user 层
 *   ~/.claude/settings.json）由 suite before/after 快照还原，防污染用户数据
 *   （run-wdio.cjs 备份集合不覆盖 exe 同级 settings.json）。
 * - 余量刷新闭环（④）：假 env 注入 user 层 settings.json（SEC-18 假值占位符，
 *   deepseek URL 命中 QUERIES 匹配集）→ 提交 120 → refreshPlanBalance 真实 invoke →
 *   后端一轮拉取（假 token 必 401/超时 → merge_slot 占位行）→ 快照变化 emit →
 *   导航树余量 footer 渲染 plan-balance-row。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  createProject,
  addPage,
  getProjectIdForPage,
  switchToPageAndWait,
  getActivePageInfo,
} from "./specUtils";

// ── Window 全局类型扩展（E2E helper 由应用侧 helpers.ts 注入，SC-E2E-01） ──

declare global {
  interface Window {
    __dockviewApi?: any;
    __slterm_e2e_openSettings?: () => Promise<void>;
    __slterm_e2e_getSettingsPanelState?: () => { selectedPage: string | null; panelId: string } | null;
    __slterm_e2e_getSettingsPanelCount?: () => number;
    __slterm_e2e_switchSettingsPage?: (id: string) => boolean;
    __slterm_e2e_setSettingsDirty?: (panelId: string, dirty: boolean) => void;
    __slterm_e2e_closeAllSettingsPanels?: () => void;
    __slterm_e2e_setHooksConfigJson?: (text: string) => boolean;
    __slterm_e2e_getHooksConfigJson?: () => string | null;
    __slterm_e2e_getSideBarState?: () => {
      open: { top: string | null; bottom: string | null };
    } | null;
    __slterm_e2e_toggleSideView?: (id: string) => void;
  }
}

// ── 持久化文件路径（与后端契约同源） ──

/** 后端 settings.json：exe 同级（app_dir.rs 便携分发；wdio.conf application 同基准）。
    优先经 SLTERM_DATA_DIR 推导（跨 agent 契约，隔离测试数据目录），无 env 时回退
    exe 同级默认路径（直跑 wdio 兜底）。 */
const settingsJsonPath = join(
  process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug"),
  "settings.json",
);
/** user 层 ~/.claude/settings.json（plan_balance source.rs 余量来源契约） */
const claudeSettingsPath = join(homedir(), ".claude", "settings.json");

// ── 页面内 helper ──

/** 点击活动栏「配置」钮（真实用户入口——openSettings 编排） */
async function clickConfigButton(): Promise<void> {
  const clicked = await browser.execute(() => {
    const btn = document.querySelector(
      '[data-e2e="activity-btn-config"]',
    ) as HTMLButtonElement | null;
    btn?.click();
    return btn !== null;
  });
  expect(clicked).toBe(true);
}

/** 等待设置面板挂载（DOM 级，跨页面可查——隐藏页面板不卸载仍留 DOM） */
async function waitForSettingsPanel(timeout = 15000): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await browser.execute(() => !!document.querySelector('[data-e2e="settings-panel"]'))) === true,
    { timeout, timeoutMsg: "设置面板未就绪" },
  );
}

/** 经 helper 打开设置中心（等价配置钮点击；无项目 → toast 不打开） */
async function openSettingsCenter(): Promise<void> {
  await browser.execute(() => (window as any).__slterm_e2e_openSettings?.());
  await waitForSettingsPanel();
}

/** 关闭全部 settings 面板（遍历全部页面 api——含隐藏页面残留面板，
    活跃页面 __dockviewApi 清理不到，TE-03 泄漏根因；用例清理/重试隔离用） */
async function closeSettingsPanels(): Promise<void> {
  await browser.execute(() => (window as any).__slterm_e2e_closeAllSettingsPanels?.());
}

/** 经 DOM 点击左导航切配置页（settings-nav-{id}），等待 params.selectedPage 生效 */
async function switchSettingsPage(id: string): Promise<void> {
  const ok = await browser.execute(
    (pid: string) => (window as any).__slterm_e2e_switchSettingsPage?.(pid) ?? false,
    id,
  );
  expect(ok).toBe(true);
  await browser.waitUntil(
    async () => {
      const state = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      return state?.selectedPage === id;
    },
    { timeout: 10000, timeoutMsg: `配置页未切换到 ${id}` },
  );
}

/** React 受控 input 设值（原生 value setter + input 事件——与 testing-library fireEvent 同路径） */
async function setInputValue(sel: string, value: string): Promise<boolean> {
  return browser.execute((s: string, v: string) => {
    const input = document.querySelector(s) as HTMLInputElement | null;
    if (!input) return false;
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set;
    if (!setter) return false;
    setter.call(input, v);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  }, sel, value);
}

/** 触发失焦（React onBlur 经 focusout 委托——真实用户 blur 的原生事件序列） */
async function blurInput(sel: string): Promise<boolean> {
  return browser.execute((s: string) => {
    const input = document.querySelector(s) as HTMLInputElement | null;
    if (!input) return false;
    input.dispatchEvent(new Event("focusout", { bubbles: true }));
    return true;
  }, sel);
}

/** 确保导航树侧栏展开（余量 footer 渲染前置；默认 open.top="nav"，防御性检查） */
async function ensureNavOpen(): Promise<void> {
  const state = await browser.execute(
    () => (window as any).__slterm_e2e_getSideBarState?.() ?? null,
  );
  if (state?.open?.top !== "nav") {
    await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("nav"));
  }
  await browser.waitUntil(
    async () =>
      (await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]'))) === true,
    { timeout: 10000, timeoutMsg: "导航树未渲染" },
  );
}

// ── Node 侧文件 helper ──

/** 同步读后端 settings.json 原始内容（文件不存在/读取中间态返回 null） */
function readSettingsRaw(): string | null {
  try {
    return existsSync(settingsJsonPath) ? readFileSync(settingsJsonPath, "utf8") : null;
  } catch {
    return null;
  }
}

/** 等待后端 settings.json 满足条件（原子写中间态自动重试） */
async function waitForSettingsFile(
  predicate: (root: Record<string, unknown>) => boolean,
  timeout = 10000,
  timeoutMsg = "后端 settings.json 条件未满足",
): Promise<void> {
  await browser.waitUntil(
    () => {
      try {
        if (!existsSync(settingsJsonPath)) return false;
        const root = JSON.parse(readFileSync(settingsJsonPath, "utf8")) as Record<
          string,
          unknown
        >;
        return predicate(root);
      } catch {
        return false;
      }
    },
    { timeout, timeoutMsg },
  );
}

/** 写假值余量 env 到 user 层 settings.json（SEC-18 假值占位符；deepseek URL 命中
    QUERIES 匹配集 → 后端刷新后产出占位行 → 导航树余量 footer 可断言） */
function writeFakePlanEnv(): void {
  let root: Record<string, unknown>;
  try {
    root = JSON.parse(readFileSync(claudeSettingsPath, "utf8")) as Record<string, unknown>;
  } catch {
    root = {};
  }
  const env = (root.env ?? {}) as Record<string, unknown>;
  env.ANTHROPIC_BASE_URL = "https://api.deepseek.com/anthropic";
  env.ANTHROPIC_AUTH_TOKEN = "sk-test-e2e"; // 假值占位符（SEC-18，非真实凭据）
  root.env = env;
  writeFileSync(claudeSettingsPath, JSON.stringify(root, null, 2), "utf8");
}

describe("设置中心 (F11, SC-E2E-02)", () => {
  // 用例真实写盘两处：exe 同级 settings.json（④ planBalance 段 / ⑥ keybindings 段）、
  // user 层 ~/.claude/settings.json（④ 假 env）。run-wdio.cjs 备份集合不覆盖
  // exe 同级 settings.json——suite 级快照还原防污染用户数据。
  let settingsSnapshot: { existed: boolean; content: string | null };
  let claudeSnapshot: { existed: boolean; content: string | null };

  before(() => {
    settingsSnapshot = {
      existed: existsSync(settingsJsonPath),
      content: existsSync(settingsJsonPath) ? readFileSync(settingsJsonPath, "utf8") : null,
    };
    claudeSnapshot = {
      existed: existsSync(claudeSettingsPath),
      content: existsSync(claudeSettingsPath) ? readFileSync(claudeSettingsPath, "utf8") : null,
    };
  });

  after(() => {
    const restore = (snap: { existed: boolean; content: string | null }, p: string) => {
      try {
        if (snap.existed) {
          writeFileSync(p, snap.content ?? "", "utf8");
        } else {
          rmSync(p, { force: true });
        }
      } catch {
        // 还原失败仅告警（后续 spec 的 resetSettings debounce 也会覆写该文件）
        console.warn(`[settings.e2e] 还原 ${p} 失败`);
      }
    };
    restore(settingsSnapshot, settingsJsonPath);
    restore(claudeSnapshot, claudeSettingsPath);
  });

  /** 用例 ①：配置钮打开（设置面板存在 + 默认全局组第一页） */
  it("① 配置钮打开：设置面板挂载 + 默认全局组第一页（快捷键）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-open-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();

      // 活动栏「配置」钮真实点击 → openSettings 编排 → 设置面板
      await clickConfigButton();
      await waitForSettingsPanel();

      // 默认全局组第一页 = keybindings（pages.ts order 10 < planBalance order 20）
      const state = await browser.execute(() => {
        const page = document.querySelector('[data-e2e="settings-keybindings-page"]');
        return {
          kbPage: !!page,
          navKb: !!document.querySelector('[data-e2e="settings-nav-keybindings"]'),
          navPlanBalance: !!document.querySelector('[data-e2e="settings-nav-planBalance"]'),
          groupGlobal: !!document.querySelector('[data-e2e="settings-nav-group-global"]'),
        };
      });
      expect(state.kbPage).toBe(true);
      expect(state.navKb).toBe(true);
      expect(state.navPlanBalance).toBe(true);
      expect(state.groupGlobal).toBe(true);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ②：再点配置钮 → 单例（count=1） */
  it("② 再点配置钮 → 同页单例（count=1）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-single-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      // 清理前序用例遗留 → 从零开始
      await closeSettingsPanels();
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(0);

      await clickConfigButton();
      await waitForSettingsPanel();
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(1);

      // 再点 → openSettingsPanel 命中既有面板（focus 不新建）
      await clickConfigButton();
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
          )) === 1,
        { timeout: 8000, timeoutMsg: "再点配置钮后面板数量不等于 1（单例破坏）" },
      );
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(1);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ③：切页 → params.selectedPage 持久化（helper 读） */
  it("③ 切配置页 → params.selectedPage 持久化（helper 读）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-page-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();

      // DOM 点击左导航（settings-nav-planBalance）→ 壳 persistParams 写 params
      await switchSettingsPage("planBalance");
      const state = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      expect(state?.selectedPage).toBe("planBalance");
      // 右侧槽位渲染对应配置页
      expect(await browser.execute(
        () => !!document.querySelector('[data-e2e="settings-plan-balance-page"]'),
      )).toBe(true);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ④：频率页 120 失焦 → 真实后端落盘（loadSettings 读段 120）+ 余量刷新 */
  it("④ 频率页 120 失焦 → 真实后端落盘（settings.json planBalance=120）+ 余量刷新闭环", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-freq-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("planBalance");
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="settings-plan-balance-page"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "套餐余量配置页未渲染" },
      );

      // 假 env 注入（余量刷新闭环前置——见文件头注释）
      writeFakePlanEnv();

      // 设 120 → 失焦提交（React 受控 input 原生 setter + input/focusout 事件）
      expect(await setInputValue('[data-e2e="settings-plan-balance-input"]', "120")).toBe(true);
      expect(await blurInput('[data-e2e="settings-plan-balance-input"]')).toBe(true);

      // 真实后端落盘：plan_balance_set_interval → settings.rs 写通道 → exe 同级文件
      await waitForSettingsFile(
        (root) => {
          const pb = root.planBalance as { intervalSec?: unknown } | undefined;
          return pb?.intervalSec === 120;
        },
        10000,
        "planBalance.intervalSec 未在 10s 内落盘为 120（命令 invoke 失败或未注册）",
      );
      // 无行内红字 + 输入框规范化回显
      expect(await browser.execute(
        () => !!document.querySelector('[data-e2e="settings-plan-balance-error"]'),
      )).toBe(false);
      expect(await browser.execute(
        () => (document.querySelector('[data-e2e="settings-plan-balance-input"]') as HTMLInputElement | null)?.value ?? null,
      )).toBe("120");

      // 余量刷新闭环：提交成功 → refreshPlanBalance 真实 invoke → 后端一轮拉取 →
      // 假 token 必失败 → merge_slot 占位行 → 快照变化 emit → 导航树余量 footer 渲染
      await ensureNavOpen();
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="plan-balance-row"]'))) === true,
        { timeout: 15000, timeoutMsg: "余量刷新后导航树未渲染余量行（refresh_plan_balance 链路失效）" },
      );
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑤：频率页 5 → 行内红字 + 文件未变 */
  it("⑤ 频率页 5 → 行内红字 + 文件未变（非法值不提交不落盘）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-invalid-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("planBalance");
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="settings-plan-balance-page"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "套餐余量配置页未渲染" },
      );

      // 写前快照（前序用例写盘已 settle；原子写保证读到的必是完整内容；
      // null = 文件尚不存在——「未变」断言即仍不存在）
      const before = readSettingsRaw();

      // 非法值 5（< MIN_INTERVAL_SEC=10）→ 失焦 → 行内红字，不提交不 toast
      expect(await setInputValue('[data-e2e="settings-plan-balance-input"]', "5")).toBe(true);
      expect(await blurInput('[data-e2e="settings-plan-balance-input"]')).toBe(true);
      await browser.waitUntil(
        async () => {
          const text = await browser.execute(
            () => document.querySelector('[data-e2e="settings-plan-balance-error"]')?.textContent ?? null,
          );
          return text !== null && text.includes("10–3600 秒，默认 60");
        },
        { timeout: 8000, timeoutMsg: "非法提交未显示行内红字提示" },
      );

      // 文件未变（等待原子写中间态重试后比较原始内容）
      await browser.waitUntil(
        () => readSettingsRaw() === before,
        { timeout: 5000, timeoutMsg: "非法提交后 settings.json 内容发生变化（不应落盘）" },
      );
      expect(readSettingsRaw()).toBe(before);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑥：快捷键录制（dispatch 合成 KeyboardEvent Ctrl+Alt+KeyC）→ 生效键更新
      + 2s debounce 后落盘断言 */
  it("⑥ 快捷键录制 Ctrl+Alt+KeyC → 生效键更新 + 2s debounce 落盘", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-kb-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      // 新面板默认全局组第一页 = 快捷键页（params.selectedPage 未注入时壳回退——
      // 此时 params.selectedPage 为 undefined，不能用切页 helper 轮询该字段，
      // 直接等页面渲染）
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="settings-keybindings-page"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "快捷键配置页未渲染（默认页回退失败）" },
      );

      // 默认生效键 Ctrl+W（COMMAND_CATALOG global.closeTab）
      expect(await browser.execute(
        () => document.querySelector(
          '[data-e2e="kb-row-global.closeTab"] [data-e2e="kb-effective"]',
        )?.textContent ?? null,
      )).toBe("Ctrl+KeyW");

      // 点击行进入录制态
      await browser.execute(() => {
        const row = document.querySelector('[data-e2e="kb-row-global.closeTab"]') as HTMLElement | null;
        row?.click();
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="kb-row-global.closeTab"] [data-e2e="kb-recording-hint"]'),
          )) === true,
        { timeout: 8000, timeoutMsg: "录制态未进入（kb-recording-hint 未出现）" },
      );

      // dispatch 合成 KeyboardEvent（TE-17 先例）：Ctrl+Alt+KeyC 非保留键 → 合法写入
      await browser.execute(() => {
        const evt = new KeyboardEvent("keydown", {
          key: "c",
          code: "KeyC",
          ctrlKey: true,
          altKey: true,
          bubbles: true,
          cancelable: true,
        });
        window.dispatchEvent(evt);
      });

      // 生效键更新（setBinding → overrides → 行内高亮 + 默认键小字 + 复位钮）
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => document.querySelector(
              '[data-e2e="kb-row-global.closeTab"] [data-e2e="kb-effective"]',
            )?.textContent ?? null,
          )) === "Ctrl+Alt+KeyC",
        { timeout: 8000, timeoutMsg: "录制后生效键未更新为 Ctrl+Alt+KeyC" },
      );
      const row = await browser.execute(() => {
        const r = document.querySelector('[data-e2e="kb-row-global.closeTab"]');
        return {
          hasReset: !!r?.querySelector('[data-e2e="kb-reset"]'),
          defaultText: r?.querySelector('[data-e2e="kb-default"]')?.textContent ?? null,
          recordingGone: !r?.querySelector('[data-e2e="kb-recording-hint"]'),
        };
      });
      expect(row.hasReset).toBe(true);
      expect(row.defaultText).toBe("Ctrl+KeyW");
      expect(row.recordingGone).toBe(true);

      // 2s debounce 后落盘（stores/keybindings PERSIST_DEBOUNCE_MS）→ 后端 settings.json
      await waitForSettingsFile(
        (root) => {
          const kb = root.keybindings as Record<string, unknown> | undefined;
          return kb?.["global.closeTab"] === "Ctrl+Alt+KeyC";
        },
        10000,
        "keybindings 覆盖未在 10s 内落盘（2s debounce 或 save_settings 失败）",
      );
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑦：切项目 → 老面板关闭（count=0）→ 新项目配置钮 → pages[0] 打开 */
  it("⑦ 切项目 → 老面板自动关闭 → 新项目配置钮 → 新项目 pages[0] 打开", async () => {
    const dirA = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-projA-"));
    const dirB = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-projB-"));
    try {
      await waitForWorkspaceReady();
      await createProject(dirA);
      await waitForDockviewApi();
      await openSettingsCenter();
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(1);

      // 创建项目 B（helper 内部 setActivePage → 切项目自动关闭效应触发）
      const pageIdB = await createProject(dirB);
      // 老面板关闭（DOM 级断言——隐藏页面板不卸载仍留 DOM，api.close 才真正卸载）
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => document.querySelectorAll('[data-e2e="settings-panel"]').length,
          )) === 0,
        { timeout: 10000, timeoutMsg: "切项目后旧设置面板未自动关闭" },
      );
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(0);

      // 新项目配置钮 → openSettings 目标 = 活跃项目 B → B.pages[0] 打开
      await clickConfigButton();
      await waitForSettingsPanel();
      const info = await getActivePageInfo();
      expect(info?.pageId).toBe(pageIdB); // B 仅 1 页 = pages[0]
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(1);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(dirA, { recursive: true, force: true }); } catch { /* 忽略 */ }
      try { rmSync(dirB, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑧：同项目切页 → 面板保留（挂载不卸载 + 选中页 params 持久化） */
  it("⑧ 同项目切页 → 设置面板保留（挂载 + params.selectedPage 持久化）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-sameproj-"));
    try {
      await waitForWorkspaceReady();
      const pageIdA = await createProject(tempDir);
      await waitForDockviewApi();
      const projectId = (await getProjectIdForPage(pageIdA)) ?? "";
      expect(projectId).not.toBe("");
      const pageIdB = await addPage(projectId, "page2", tempDir);
      if (!pageIdB) throw new Error("addPage 返回 null（页面数上限或项目缺失）");

      await openSettingsCenter();
      await switchSettingsPage("planBalance");

      // 面板 id 契约 SC-FE-02：settings-{pageId}（切页前后恒为 page1 的面板）
      const panelId = `settings-${pageIdA}`;
      const stateBefore = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      expect(stateBefore?.panelId).toBe(panelId);
      expect(stateBefore?.selectedPage).toBe("planBalance");

      // 同项目内切页（activePageId 变化但项目不变 → 自动关闭效应不触发）
      await switchToPageAndWait(pageIdB);
      // 面板仍挂载（隐藏页面板不卸载——DOM 级断言，活跃 api 已指向 page2 读不到）
      expect(await browser.execute(
        () => document.querySelectorAll('[data-e2e="settings-panel"]').length,
      )).toBe(1);

      // 切回 page1 → helper 读 params.selectedPage 仍为 planBalance（随布局 JSON 持久化）
      await switchToPageAndWait(pageIdA);
      const state = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      expect(state?.selectedPage).toBe("planBalance");
      expect(state?.panelId).toBe(panelId);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑨：hooks 页迁入冒烟（设置中心内 CLI 选择行渲染） */
  it("⑨ hooks 页迁入冒烟：设置中心内 CLI 选择行渲染", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-hooksmoke-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("hooks");

      // HooksSettingsPage 根容器保留 data-e2e="hooks-config-panel"（SC-FE-05 决策）
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-config-panel"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooks 配置页未就绪" },
      );
      // CLI 选择行：claude 按钮（logo 16×16 + displayName）+ 编辑器槽
      const row = await browser.execute(() => {
        const panel = document.querySelector('[data-e2e="hooks-config-panel"]');
        const btn = panel?.querySelector('[data-e2e="hooks-cli-claude"]');
        const imgs = panel
          ? Array.from(panel.querySelectorAll<HTMLImageElement>('img[src="/cli-icons/claude.png"]'))
          : [];
        return {
          hasPanel: !!panel,
          hasBtn: !!btn,
          btnText: btn?.textContent ?? "",
          logoCount: imgs.length,
        };
      });
      expect(row.hasPanel).toBe(true);
      expect(row.hasBtn).toBe(true);
      expect(row.btnText).toContain("claude");
      expect(row.logoCount).toBeGreaterThan(0);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑩：hooks 页 dirty → 切配置页 → confirm 弹窗 → 取消 → 未切换 */
  it("⑩ hooks 页 dirty → 切配置页 confirm 取消 → 未切换且修改保留", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-dirty-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("hooks");
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-config-panel"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooks 配置页未就绪" },
      );
      // JSON 模式编辑器就绪（默认 JSON 模式；JsonMode 挂载点 data-e2e 随迁移保留）
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-json-editor"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooks JSON 编辑器未就绪" },
      );

      // 注入合法 hooks JSON → 编辑器 dirty → 壳 dirty 圆点（settings-nav-dirty-hooks）
      const hooksJson = JSON.stringify({
        PreToolUse: [
          {
            matcher: "Bash",
            hooks: [{ type: "command", command: "echo e2e-dirty-marker" }],
          },
        ],
      });
      const injected = await browser.execute(
        (text: string) => (window as any).__slterm_e2e_setHooksConfigJson?.(text) === true,
        hooksJson,
      );
      expect(injected).toBe(true);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="settings-nav-dirty-hooks"]'))) === true,
        { timeout: 8000, timeoutMsg: "hooks 页 dirty 圆点未出现（dirty 未上报壳）" },
      );

      // 切配置页（planBalance）→ dirty 守卫 → confirmDialog
      await browser.execute(() => {
        const nav = document.querySelector(
          '[data-e2e="settings-nav-planBalance"]',
        ) as HTMLElement | null;
        nav?.click();
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="confirm-dialog"]'))) === true,
        { timeout: 8000, timeoutMsg: "切页确认弹窗未出现（dirty 守卫未触发）" },
      );

      // 取消 → 弹窗关闭、未切换、修改保留
      await browser.execute(() => {
        const btn = document.querySelector('[data-e2e="confirm-cancel"]') as HTMLElement | null;
        btn?.click();
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !document.querySelector('[data-e2e="confirm-dialog"]'))) === true,
        { timeout: 8000, timeoutMsg: "确认弹窗未关闭" },
      );
      const state = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      expect(state?.selectedPage).toBe("hooks");
      expect(await browser.execute(
        () => !!document.querySelector('[data-e2e="hooks-config-panel"]'),
      )).toBe(true);
      const doc = await browser.execute(
        () => (window as any).__slterm_e2e_getHooksConfigJson?.() ?? null,
      );
      expect(doc).toContain("e2e-dirty-marker");
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 ⑪：× 关闭 dirty 守卫（后门置 dirty → × → confirm 弹窗 → cancel 面板保留
      → 再 × → ok 面板关闭；DefaultTab 拦截经 dirtyRegistry 真值源） */
  it("⑪ × 关闭 dirty 守卫：confirm-cancel 面板保留 / confirm-ok 面板关闭", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-settings-close-dirty-"));
    try {
      await waitForWorkspaceReady();
      const pageId = await createProject(tempDir);
      await waitForDockviewApi();
      // 兜底清理前序用例/重试残留的 settings 面板（含隐藏页面——全局遍历，
      // 防泄漏污染本用例 DOM 计数断言）
      await closeSettingsPanels();
      await openSettingsCenter();
      const panelId = `settings-${pageId}`; // 面板 id 契约 SC-FE-02：settings-{pageId}

      // 后门 helper 已注入 + 直接置 dirty（绕过真实编辑——dirtyRegistry 真值源）
      expect(await browser.execute(
        () => typeof (window as any).__slterm_e2e_setSettingsDirty === "function",
      )).toBe(true);
      await browser.execute(
        (pid: string) => (window as any).__slterm_e2e_setSettingsDirty?.(pid, true),
        panelId,
      );

      // 点 ×（FE-04 契约选择器 data-e2e="tab-close-{panelId}"）→ dirty 守卫 → confirm 弹窗
      const clicked = await browser.execute((pid: string) => {
        const btn = document.querySelector(`[data-e2e="tab-close-${pid}"]`) as HTMLElement | null;
        btn?.click();
        return btn !== null;
      }, panelId);
      expect(clicked).toBe(true);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="confirm-dialog"]'))) === true,
        { timeout: 8000, timeoutMsg: "× 关闭 dirty 面板未弹确认框（守卫未触发）" },
      );

      // 取消 → 弹窗关闭、面板保留（dirty 未清——取消不丢修改）
      await browser.execute(() => {
        const btn = document.querySelector('[data-e2e="confirm-cancel"]') as HTMLElement | null;
        btn?.click();
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !document.querySelector('[data-e2e="confirm-dialog"]'))) === true,
        { timeout: 8000, timeoutMsg: "确认弹窗未关闭" },
      );
      // 面板保留断言按 panelId 精确归属（不用全局 DOM 计数——隐藏页面残留
      // 面板会污染全量计数，TE-03；本用例开头已兜底清理，归属断言隔离泄漏）
      const kept = await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelState?.() ?? null,
      );
      expect(kept?.panelId).toBe(panelId);
      expect(await browser.execute(
        () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
      )).toBe(1);

      // 再点 × → confirm-ok → 面板关闭（壳卸载清 dirty）
      await browser.execute((pid: string) => {
        const btn = document.querySelector(`[data-e2e="tab-close-${pid}"]`) as HTMLElement | null;
        btn?.click();
      }, panelId);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="confirm-dialog"]'))) === true,
        { timeout: 8000, timeoutMsg: "第二次 × 未弹确认框" },
      );
      await browser.execute(() => {
        const btn = document.querySelector('[data-e2e="confirm-ok"]') as HTMLElement | null;
        btn?.click();
      });
      // 关闭断言走活跃页面 api 计数（全局 DOM 计数受隐藏页面残留面板干扰，
      // 不可用；本用例面板在活跃页面上，stateCount 归零即关闭成功）
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => (window as any).__slterm_e2e_getSettingsPanelCount?.() ?? 0,
          )) === 0,
        { timeout: 8000, timeoutMsg: "confirm-ok 后设置面板未关闭" },
      );
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });
});
