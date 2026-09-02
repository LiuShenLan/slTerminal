/**
 * 后台定时任务（F12）L4 spec（E2E-02/E2E-03）：
 *
 * 覆盖后台定时任务设置页全链路（E2E-02 用例 A-D）与定时刷新真实 tick 生效（E2E-03
 * 用例 E/F）：
 * - 用例 A：配置钮 → 设置面板 → 「后台定时任务」页 → 两行齐备 + 勾选/频率默认值。
 * - 用例 B：planBalance 行频率改 15 失焦 → 真实后端落盘（background_tasks_set_config
 *   命令真实 invoke——L4 兜底「命令未注册被前端 catch 吞 = 测试全绿但运行时静默失败」
 *   盲区）→ 规范化回显 + 无红字。
 * - 用例 C：勾选禁用 planBalance → footer 隐藏（事件驱动）+ 磁盘 enabled=false；
 *   重新启用 → footer 重显（最后快照保留）+ 磁盘 enabled=true。
 * - 用例 D：sessionRefresh 行输 1（< 2 下限）→ 行内红字「2–300 秒」+ 文件未变。
 * - 用例 E：sessionRefresh 频率改 2s → Node 侧往 SLTERM_CLAUDE_PROJECTS_DIR 副本写
 *   归属 E2E 项目的新会话 jsonl → 真实 tick 扫描 → 导航树历史节点计数 pill N+1
 *   （全程无手动刷新点击——调度器定时执行体驱动，规格 §1 单一执行体）。
 * - 用例 F：禁用 sessionRefresh → 新会话不自动出现；重新启用 → 立即一轮 → 计数 +1
 *   （基线取 pill 收敛值——E 删会话后遗留 tick 重扫未落地会读到陈旧值，R2a 竞态修复）。
 * - 用例 G（tick 失败静默）不写 spec：无可控故障注入通道（需后端扫描故障注入，
 *   无沙箱内通道）——降级为调度器 L2 用例 + 人工观察，豁免登记由 Stage 06 DOC-02
 *   完成（checklist E2E-03）。
 *
 * 关键约定（照 settings.e2e.ts / history.e2e.ts 先例）：
 * - 后端 settings.json 在 exe 同级（app_dir.rs 便携分发契约）——落盘断言直接
 *   Node 侧读文件（与 loadSettings 同一真值源），原子写中间态自动重试。
 * - 用例写盘（B/C/E/F backgroundTasks 段 + C 假 env 注入 user 层 ~/.claude/settings.json）
 *   由 suite before/after 快照还原，防污染用户数据。
 * - 会话数据隔离（SEC-02）：用例 E/F 只写 run-wdio.cjs 重建的 claude-projects 副本
 *   （SLTERM_CLAUDE_PROJECTS_DIR），新会话 cwd = SLTERM_E2E_PROJECT_DIR（E2E 项目
 *   根目录）→ 导航树归属（决策 5：cwd 前缀匹配项目 rootPath）；用例 finally 删除
 *   写入文件 + 专用目录，防污染后续 history spec 的副本断言。
 * - helper 复用 settings.e2e.ts / history.e2e.ts 既有模式（假 env 注入 /
 *   waitForSettingsFile / 计数 pill 读法），不另造轮子。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { waitForWorkspaceReady, waitForDockviewApi, createProject } from "./specUtils";

// ── Window 全局类型扩展（E2E helper 由应用侧 helpers.ts 注入） ──

declare global {
  interface Window {
    __slterm_e2e_openSettings?: () => Promise<void>;
    __slterm_e2e_getSettingsPanelState?: () => { selectedPage: string | null; panelId: string } | null;
    __slterm_e2e_switchSettingsPage?: (id: string) => boolean;
    __slterm_e2e_closeAllSettingsPanels?: () => void;
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

// ── 会话副本 env（run-wdio.cjs 注入；缺 env 直接 fail——绝不回落真实目录） ──

const projectsDir = process.env.SLTERM_CLAUDE_PROJECTS_DIR;
const e2eProjectDir = process.env.SLTERM_E2E_PROJECT_DIR;
if (!projectsDir || !e2eProjectDir) {
  throw new Error(
    "SLTERM_CLAUDE_PROJECTS_DIR / SLTERM_E2E_PROJECT_DIR 未注入——必须经 run-wdio.cjs 启动（npm run e2e）",
  );
}

/** E2E-03 用例专用会话目录名（副本内新目录——后端扫描仅按目录遍历，不反解码目录名） */
const TICK_DIR_NAME = "E2E-tick-sessions";
/** E2E-03 写入的会话 UUID（36 位 v4 形态——is_uuid_filename 校验，与 fixture 不冲突） */
const UUID_TICK_1 = "22222222-3333-4444-8555-666666666601"; // 用例 E 新会话
const UUID_TICK_2 = "22222222-3333-4444-8555-666666666602"; // 用例 F 禁用期新会话
const UUID_TICK_3 = "22222222-3333-4444-8555-666666666603"; // 用例 F 启用后新会话

// ── 页面内 helper（照 settings.e2e.ts 提取复用） ──

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

/** 关闭全部 settings 面板（含隐藏页面残留面板——TE-03 泄漏根因，用例清理/重试隔离用） */
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

/** 打开 nav 视图（幂等：已打开不重复 toggle——照 history.e2e.ts openNavView） */
async function openNavView(): Promise<void> {
  await waitForWorkspaceReady();
  const s = await browser.execute(() => (window as any).__slterm_e2e_getSideBarState?.() ?? null);
  if (s?.open?.top !== "nav") {
    await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("nav"));
  }
  await browser.waitUntil(
    async () => await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
    { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
  );
}

/** 点击任务行勾选（仅当当前态与目标不一致才点——防重试残留/竞态重复翻转）。
    返回 checkbox 是否存在（缺失提前失败而非隐性超时）。 */
async function clickEnabledCheckbox(taskId: string, wantChecked: boolean): Promise<boolean> {
  return browser.execute((tid: string, want: boolean) => {
    const cb = document.querySelector(
      `[data-e2e="settings-background-tasks-enabled-${tid}"]`,
    ) as HTMLInputElement | null;
    if (!cb) return false;
    if (cb.checked !== want) cb.click();
    return true;
  }, taskId, wantChecked);
}

/** 等待勾选 DOM 态变为期望值（提交成功 → 返回清单 → setTasks 行更新后才反映） */
async function waitForCheckboxState(taskId: string, checked: boolean): Promise<void> {
  await browser.waitUntil(
    async () =>
      await browser.execute((tid: string, want: boolean) => {
        const cb = document.querySelector(
          `[data-e2e="settings-background-tasks-enabled-${tid}"]`,
        ) as HTMLInputElement | null;
        return cb !== null && cb.checked === want;
      }, taskId, checked),
    { timeout: 8000, timeoutMsg: `勾选态未变为 ${checked}` },
  );
}

/** 等待后台定时任务配置页渲染（settings-background-tasks-page 根容器） */
async function waitForTasksPageRender(): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await browser.execute(
        () => !!document.querySelector('[data-e2e="settings-background-tasks-page"]'),
      )) === true,
    { timeout: 10000, timeoutMsg: "后台定时任务配置页未渲染" },
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

/**
 * 读 E2E 项目历史节点计数 pill。
 * 定位：nav-row-project 文本同时含项目名（SLTERM_E2E_PROJECT_DIR 末段）与「当前」
 * pill（E/F 用例中 E2E 项目恒为活跃项目——创建后无切页动作）→ 其兄弟
 * nav-history-node（NavTree 结构：项目容器 = 项目行 + 历史节点同级）→ 头部行
 * 末位 span 即计数 pill。双条件锚定防重试残留的同名旧项目误读。无节点返回 null。
 */
async function readHistoryCountPill(): Promise<number | null> {
  const projName = e2eProjectDir.split(/[/\\]/).pop() ?? "";
  return browser.execute((name: string) => {
    const rows = document.querySelectorAll('[data-e2e="nav-row-project"]');
    for (const r of rows) {
      const text = r.textContent ?? "";
      if (text.includes(name) && text.includes("当前")) {
        const node = r.parentElement?.querySelector('[data-e2e="nav-history-node"]');
        if (!node) return null;
        const headRow = node.children[0];
        const spans = headRow ? Array.from(headRow.querySelectorAll("span")) : [];
        const pill = spans[spans.length - 1];
        if (!pill) return null;
        const v = Number(pill.textContent ?? "");
        return Number.isFinite(v) ? v : null;
      }
    }
    return null;
  }, projName);
}

/**
 * 等待历史计数 pill 收敛到稳定值（R2a 基线竞态修复）。
 * E 用例 finally 删除会话 601 后，E 结束→F 开始仅 ~200ms（< 遗留 scheduler 的 2s
 * tick 周期），「删除后重扫」未落地，pill 仍持陈旧值——若此时直接取基线会得到错误
 * n，启用后断言 n+1 永不可达。收敛判据：轮询间隔约 1s、同值持续 ≥3s（覆盖一个
 * tick 周期 + 扫描余量）才判收敛，总上限 6s——「连续两次相同」不足以排除陈旧值
 * （两次读取可都落在重扫前）。超时抛错（基线不可靠则后续断言必败，提前报因）。
 */
async function waitForHistoryPillSettled(timeoutMs = 6000): Promise<number> {
  const start = Date.now();
  let runValue: number | null = null;
  let runStart = 0;
  while (Date.now() - start < timeoutMs) {
    const v = await readHistoryCountPill();
    if (v !== null && v > 0) {
      if (runValue !== null && v === runValue) {
        if (Date.now() - runStart >= 3000) return v; // 同值持续 ≥3s → 收敛
      } else {
        runValue = v;
        runStart = Date.now();
      }
    } else {
      runValue = null; // 节点未渲染/读不到 → 重置连续段
    }
    await browser.pause(1000);
  }
  throw new Error(
    `历史计数 pill 未在 ${timeoutMs}ms 内收敛（陈旧基线未落地重扫，基线不可靠）`,
  );
}

/**
 * 写一条归属 E2E 项目的新会话 jsonl 到副本（照 history.e2e.ts fixture 507 形态：
 * summary 首行 + user 行 cwd=项目路径——cwd 归属判定在内容不在目录名）。
 * JSON.stringify 自动转义 Windows 反斜杠（与 run-wdio.cjs 占位符替换口径一致）。
 */
function writeTickSession(uuid: string, title: string): string {
  const dir = join(projectsDir, TICK_DIR_NAME);
  mkdirSync(dir, { recursive: true });
  const lines = [
    JSON.stringify({ type: "summary", summary: title, leafUuid: "x" }),
    JSON.stringify({ type: "user", cwd: e2eProjectDir, message: { content: "F12 定时刷新端到端新会话" } }),
    "// [E2E-03] 用例写入的定时刷新会话（cwd = E2E 项目目录 → 导航树归属）",
  ];
  const p = join(dir, `${uuid}.jsonl`);
  writeFileSync(p, lines.join("\n") + "\n", "utf8");
  return p;
}

describe("后台定时任务 (F12, E2E-02/E2E-03)", () => {
  // 用例真实写盘两处：exe 同级 settings.json（B/C/E/F backgroundTasks 段）、
  // user 层 ~/.claude/settings.json（C 假 env）。run-wdio.cjs 备份集合不覆盖
  // exe 同级 settings.json——suite 级快照还原防污染用户数据（照 settings.e2e.ts 先例）。
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
        console.warn(`[background-tasks.e2e] 还原 ${p} 失败`);
      }
    };
    restore(settingsSnapshot, settingsJsonPath);
    restore(claudeSnapshot, claudeSettingsPath);
  });

  /** 用例 A：页渲染与两行齐备（勾选默认 true、频率默认 10/3） */
  it("A 页渲染与两行齐备：planBalance/sessionRefresh 行 + 默认勾选与频率", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-bt-a-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();

      // 两行齐备（data-e2e 契约：settings-background-tasks-row-{taskId}）
      const rows = await browser.execute(() => ({
        planBalance: !!document.querySelector(
          '[data-e2e="settings-background-tasks-row-planBalance"]',
        ),
        sessionRefresh: !!document.querySelector(
          '[data-e2e="settings-background-tasks-row-sessionRefresh"]',
        ),
      }));
      expect(rows.planBalance).toBe(true);
      expect(rows.sessionRefresh).toBe(true);

      // 勾选默认 true（注册表 enabled 默认）+ 频率输入默认 10 / 3（registry 默认）
      const defaults = await browser.execute(() => {
        const checked = (sel: string) =>
          (document.querySelector(sel) as HTMLInputElement | null)?.checked ?? null;
        const value = (sel: string) =>
          (document.querySelector(sel) as HTMLInputElement | null)?.value ?? null;
        return {
          planBalanceChecked: checked('[data-e2e="settings-background-tasks-enabled-planBalance"]'),
          sessionRefreshChecked: checked(
            '[data-e2e="settings-background-tasks-enabled-sessionRefresh"]',
          ),
          planBalanceInterval: value(
            '[data-e2e="settings-background-tasks-interval-planBalance"]',
          ),
          sessionRefreshInterval: value(
            '[data-e2e="settings-background-tasks-interval-sessionRefresh"]',
          ),
        };
      });
      expect(defaults.planBalanceChecked).toBe(true);
      expect(defaults.sessionRefreshChecked).toBe(true);
      expect(defaults.planBalanceInterval).toBe("10");
      expect(defaults.sessionRefreshInterval).toBe("3");
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 B：planBalance 行频率改 15 失焦 → 真实后端落盘 → 规范化回显 + 无红字 */
  it("B 改频率端到端生效：planBalance 15 失焦 → backgroundTasks.planBalance.intervalSec=15 落盘 + 回显", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-bt-b-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();

      // 设 15（合法区间 10–3600）→ 失焦提交（React 受控 input 原生 setter + focusout）
      expect(
        await setInputValue('[data-e2e="settings-background-tasks-interval-planBalance"]', "15"),
      ).toBe(true);
      expect(
        await blurInput('[data-e2e="settings-background-tasks-interval-planBalance"]'),
      ).toBe(true);

      // 真实后端落盘：background_tasks_set_config → settings.rs 写通道 → exe 同级文件
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { planBalance?: { intervalSec?: unknown } }
            | undefined;
          return bt?.planBalance?.intervalSec === 15;
        },
        10000,
        "backgroundTasks.planBalance.intervalSec 未在 10s 内落盘为 15（命令 invoke 失败或未注册）",
      );
      // 输入框规范化回显 15 + 无行内红字
      expect(
        await browser.execute(
          () => (document.querySelector('[data-e2e="settings-background-tasks-interval-planBalance"]') as HTMLInputElement | null)?.value ?? null,
        ),
      ).toBe("15");
      expect(
        await browser.execute(
          () => !!document.querySelector('[data-e2e="settings-background-tasks-error-planBalance"]'),
        ),
      ).toBe(false);
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 C：勾选禁用 planBalance → footer 隐藏（事件驱动）；重新启用 → footer 重显 */
  it("C 勾选禁用 planBalance → 余量 footer 隐藏；重新启用 → footer 重显（最后快照保留）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-bt-c-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();

      // 前置：假 env 注入（余量来源契约——见文件头注释）+ 手动刷新（频率提交 →
      // afterCommitted → refreshPlanBalance 真实 invoke → 占位行）使 plan-balance-row 出现
      writeFakePlanEnv();
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();
      expect(
        await setInputValue('[data-e2e="settings-background-tasks-interval-planBalance"]', "20"),
      ).toBe(true);
      expect(
        await blurInput('[data-e2e="settings-background-tasks-interval-planBalance"]'),
      ).toBe(true);
      await ensureNavOpen();
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="plan-balance-row"]'))) === true,
        { timeout: 15000, timeoutMsg: "假 env 注入 + 手动刷新后导航树未渲染余量行（refresh_plan_balance 链路失效）" },
      );

      // 取消勾选（enabled=false）→ 事件驱动隐藏：footer 整块不渲染（快照保留）
      expect(await clickEnabledCheckbox("planBalance", false)).toBe(true);
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { planBalance?: { enabled?: unknown } }
            | undefined;
          return bt?.planBalance?.enabled === false;
        },
        10000,
        "backgroundTasks.planBalance.enabled 未落盘为 false",
      );
      await waitForCheckboxState("planBalance", false);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="plan-balance-row"]'))) === false,
        { timeout: 10000, timeoutMsg: "禁用后余量行未消失（footer 未感知 enabled=false）" },
      );

      // 重新勾选（enabled=true）→ 磁盘回 true + footer 重显最后快照（不重拉也显）
      expect(await clickEnabledCheckbox("planBalance", true)).toBe(true);
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { planBalance?: { enabled?: unknown } }
            | undefined;
          return bt?.planBalance?.enabled === true;
        },
        10000,
        "backgroundTasks.planBalance.enabled 未落盘回 true",
      );
      await waitForCheckboxState("planBalance", true);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="plan-balance-row"]'))) === true,
        { timeout: 10000, timeoutMsg: "重新启用后余量行未重显（footer 未感知 enabled=true）" },
      );
    } finally {
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /** 用例 D：sessionRefresh 行输 1（< 2 下限）→ 行内红字「2–300 秒」+ 文件未变 */
  it("D 非法频率行内红字不落盘：sessionRefresh 1 → 红字「2–300 秒」+ settings.json 未变", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-bt-d-"));
    try {
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();

      // 写前快照（前序用例写盘已 settle；原子写保证读到的必是完整内容；
      // null = 文件尚不存在——「未变」断言即仍不存在）
      const before = readSettingsRaw();

      // 非法值 1（< sessionRefresh 下限 2）→ 失焦 → 行内红字（DTO 无 default 字段，
      // 提示只写范围），不提交不 toast
      expect(
        await setInputValue('[data-e2e="settings-background-tasks-interval-sessionRefresh"]', "1"),
      ).toBe(true);
      expect(
        await blurInput('[data-e2e="settings-background-tasks-interval-sessionRefresh"]'),
      ).toBe(true);
      await browser.waitUntil(
        async () => {
          const text = await browser.execute(
            () => document.querySelector('[data-e2e="settings-background-tasks-error-sessionRefresh"]')?.textContent ?? null,
          );
          return text !== null && text.includes("2–300 秒");
        },
        { timeout: 8000, timeoutMsg: "非法提交未显示行内红字提示" },
      );

      // 文件未变（等待原子写中间态重试后比较原始内容）——磁盘无 sessionRefresh 子键
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

  /** 用例 E：定时刷新自动出现新会话（真实 tick 生效断言——全程无手动刷新点击） */
  it("E 定时刷新自动出现新会话：改 2s → Node 写 jsonl → 历史计数 pill N+1（无手动刷新）", async () => {
    const tickDir = join(projectsDir, TICK_DIR_NAME);
    const written: string[] = [];
    try {
      await waitForWorkspaceReady();
      // 1. 创建 E2E 项目（cwd = SLTERM_E2E_PROJECT_DIR——新会话归属它）
      await createProject(e2eProjectDir);
      await openNavView();

      // 2. 打开导航树确认历史计数 N（fixture 归属会话；等首轮扫描完成 pill 出现）
      let n = 0;
      await browser.waitUntil(
        async () => {
          const v = await readHistoryCountPill();
          if (v !== null && v > 0) {
            n = v;
            return true;
          }
          return false;
        },
        { timeout: 20000, timeoutMsg: "E2E 项目历史计数 pill 未出现（扫描未完成）" },
      );

      // 3. 设置中心把 sessionRefresh 频率改 2s（磁盘断言落盘 → 调度器 timer 重启）
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();
      expect(
        await setInputValue('[data-e2e="settings-background-tasks-interval-sessionRefresh"]', "2"),
      ).toBe(true);
      expect(
        await blurInput('[data-e2e="settings-background-tasks-interval-sessionRefresh"]'),
      ).toBe(true);
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { sessionRefresh?: { intervalSec?: unknown } }
            | undefined;
          return bt?.sessionRefresh?.intervalSec === 2;
        },
        10000,
        "backgroundTasks.sessionRefresh.intervalSec 未落盘为 2",
      );

      // 4. Node 侧往副本写归属本项目的新会话 jsonl（照 history.e2e.ts fixture 形态）
      written.push(writeTickSession(UUID_TICK_1, "E2E定时刷新新会话"));

      // 5. 等待真实 tick（2×interval + 余量）→ 计数 pill 变 N+1——调度器定时执行体
      //    扫描（force=true 绕过后端缓存），无任何手动刷新点击
      await browser.waitUntil(
        async () => (await readHistoryCountPill()) === n + 1,
        { timeout: 20000, timeoutMsg: `定时刷新后历史计数 pill 未变为 ${n + 1}（tick 未生效）` },
      );
    } finally {
      // 清理写入的会话文件 + 专用目录（副本污染会波及后续 history spec）
      for (const p of written) {
        try { rmSync(p, { force: true }); } catch { /* 忽略 */ }
      }
      try { rmSync(tickDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
    }
  });

  /** 用例 F：禁用 sessionRefresh → 新会话不自动出现；重新启用 → 立即一轮 → 计数 +1
      （E2E 项目与 2s 间隔由 E 用例建立——同 spec 共享 store/磁盘，不重复创建防
      同 rootPath 双项目） */
  it("F 禁用 sessionRefresh → 新会话不自动出现；启用 → 立即出现（计数 +1）", async () => {
    const tickDir = join(projectsDir, TICK_DIR_NAME);
    const written: string[] = [];
    try {
      await waitForWorkspaceReady();
      await openNavView();
      // E2E 项目由 E 用例创建（同 spec 内 store 累积）。基线必须取 pill 收敛值
      // （R2a）：E 用例 finally 删除 601 后遗留 scheduler 2s tick 的重扫尚未落地
      // （E 结束→F 开始仅 ~200ms），立即读会命中陈旧值 5（真实 4）→ 基线错误 →
      // 启用后 n+1 断言永不可达。收敛后 n = 4（fixture 归属会话数，601 已删）。
      const n = await waitForHistoryPillSettled();

      // 1. 禁用 sessionRefresh（取消勾选 → applyConfig 停 timer）→ 磁盘 enabled=false
      await openSettingsCenter();
      await switchSettingsPage("backgroundTasks");
      await waitForTasksPageRender();
      await waitForCheckboxState("sessionRefresh", true); // 初始勾选态（E 用例遗留）
      expect(await clickEnabledCheckbox("sessionRefresh", false)).toBe(true);
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { sessionRefresh?: { enabled?: unknown } }
            | undefined;
          return bt?.sessionRefresh?.enabled === false;
        },
        10000,
        "backgroundTasks.sessionRefresh.enabled 未落盘为 false",
      );
      await waitForCheckboxState("sessionRefresh", false);

      // 2. 再写一个归属本项目的新会话 jsonl
      written.push(writeTickSession(UUID_TICK_2, "E2E禁用期新会话"));

      // 3. 等 2×interval（2s×2）+ 余量 → 计数不变（timer 已停，无 tick 扫描）
      await browser.pause(8000);
      expect(await readHistoryCountPill()).toBe(n);

      // 4. 重新勾选启用 → applyConfig 禁用→启用立即一轮 → 计数 +1
      expect(await clickEnabledCheckbox("sessionRefresh", true)).toBe(true);
      await waitForSettingsFile(
        (root) => {
          const bt = root.backgroundTasks as
            | { sessionRefresh?: { enabled?: unknown } }
            | undefined;
          return bt?.sessionRefresh?.enabled === true;
        },
        10000,
        "backgroundTasks.sessionRefresh.enabled 未落盘回 true",
      );
      await waitForCheckboxState("sessionRefresh", true);
      // 禁用→启用立即一轮扫描 = 4 fixture + 602 = 5 = n+1（基线 n 已收敛为 4）
      await browser.waitUntil(
        async () => (await readHistoryCountPill()) === n + 1,
        { timeout: 20000, timeoutMsg: `重新启用后历史计数 pill 未变为 ${n + 1}` },
      );
    } finally {
      for (const p of written) {
        try { rmSync(p, { force: true }); } catch { /* 忽略 */ }
      }
      try { rmSync(tickDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
      try { await closeSettingsPanels(); } catch { /* 忽略 */ }
    }
  });
});
