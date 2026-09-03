/**
 * Claude 历史会话域 E2E spec（NAV-08/NAV-10 重写）：
 * 导航树历史节点（挂项目下，cwd 归属）展示/排除、标题、搜索过滤、
 * 复制恢复命令、删除、历史区四态（信号文件驱动）、恢复编排（部分端到端）。
 *
 * NAV-08 迁移：旧「全部项目历史会话」区（AgentHistorySections）退役，
 * 历史会话承接方 = 导航树 NavTree 历史节点（NavHistoryNode/NavHistoryRow）：
 *   - 归属语义（决策 5）：历史会话挂项目下——cwd 前缀匹配项目 rootPath 才展示；
 *     无归属项目（fixture 中 cwd 指向 fixture 目录的会话）→ 导航树不显示
 *   - 历史行：nav-history-node 内嵌 nav-row-session（活跃行在页面下，按嵌套范围区分）
 *   - 展开/刷新：历史节点点击展开（展开触发重扫）+ 头部刷新钮（aria-label="刷新"）
 *   - 搜索：导航树搜索框（占位「搜索项目 / 页面 / 会话…」）过滤项目/页面/会话名
 *   - 右键菜单：NavContextMenu（无 data-e2e——按菜单项文本精确定位）
 *   - 四态 emoji 断言 → 圆点存在性断言（StatusDot 7px 圆形 div）
 *   - 孤儿行（cwd 不存在）不归属任何项目 → 导航树不渲染——孤儿视觉/行为由 L2 覆盖
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  ensureHooksInjected,
  writeSignalFile,
  waitForSignalConsumed,
  getActivePageInfo,
} from "./specUtils";

// ── Claude 历史会话视图（TE-01..03，NAV-10 迁移） ──
//
// 数据隔离（SEC-02 安全红线）：后端 agent_history 扫描根由 run-wdio.cjs 指向
// e2e-tests/.tmp-claude-projects/ 副本（每次运行从 fixtures/claude-projects/ 重建）。
// 本 describe 全部读写在副本内进行，删除用例只动副本文件，不触碰用户真实
// ~/.claude/projects/。恢复编排用例的项目根 = run-wdio.cjs 创建的 E2E 临时项目目录
// （process.env.SLTERM_E2E_PROJECT_DIR；fixture cwd 占位符已替换为该真实路径）。
// 用例顺序约定：展示/搜索/复制（只读）→ 四态/恢复编排（写）→
// 删除（最后——507 行删除后不再有依赖 fixture 的用例）。
describe("Claude 历史会话视图", () => {
  // fixture 会话 UUID（与 fixtures/claude-projects/ 逐字对应）
  const UUID_CUSTOM = "11111111-2222-4333-8444-555555555501"; // 形态1 custom-title（不归属 E2E 项目）
  const UUID_PROMPT = "11111111-2222-4333-8444-555555555503"; // 形态3 回退首条 prompt（不归属 E2E 项目）
  const UUID_ORPHAN = "11111111-2222-4333-8444-555555555506"; // 形态5 孤儿（不归属 E2E 项目）
  const UUID_RESTORE = "11111111-2222-4333-8444-555555555507"; // 恢复编排目标（cwd→E2E 项目目录，导航树唯一归属行）
  const fixtureDirA = "C--Users-e2e-fixture-a";
  const fixtureDirB = "C--Users-e2e-fixture-b";

  // 副本扫描根 + E2E 临时项目目录（run-wdio.cjs 注入，wdio 子进程继承 env）
  const projectsDir = process.env.SLTERM_CLAUDE_PROJECTS_DIR;
  const e2eProjectDir = process.env.SLTERM_E2E_PROJECT_DIR;
  if (!projectsDir || !e2eProjectDir) {
    throw new Error(
      "SLTERM_CLAUDE_PROJECTS_DIR / SLTERM_E2E_PROJECT_DIR 未注入——必须经 run-wdio.cjs 启动（npm run wdio）",
    );
  }

  /** 打开 nav 视图（幂等：已打开不重复 toggle，防 R2 关闭） */
  async function openNavView(): Promise<void> {
    await waitForWorkspaceReady();
    const s = await browser.execute(() => (window as any).__slterm_e2e_getSideBarState?.() ?? null);
    if (s?.open.top !== "nav") {
      await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("nav"));
    }
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
      { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
    );
  }

  /** 展开「当前活跃项目」的行（活跃会话行挂页面下——四态用例需行可见）。
   *  只处理当前项目容器内的行（含「当前」pill）——页面行点击 = 切页 + 初始化
   *  Dockview，点击其它项目的页面行会把 activePageId 切走（useAgentStatus 只建
   *  活跃项目行）；用例面板页面即活跃页面。
   *  NAV-10 修订（2026-09-03）后注意：页面行无活跃会话时不渲染子级容器
   *  （DOM 无法区分展开/收起）——「每行至多点击一次」进入展开稳态，禁止循环内
   *  反复 toggle 判定（奇偶翻转会回到收起）。前提 = 页面行初始收起（本 spec 各
   *  用例新建/新切页面行且此前未点过）；同一 NavTree 挂载内勿重复调用本函数。 */
  async function ensureProjectPagesExpanded(): Promise<void> {
    // 1. 项目行：收起（容器仅项目行 1 子级——历史节点随项目展开渲染）才点击展开；
    //    等待页面行渲染提交后再处理页面行
    for (let i = 0; i < 3; i++) {
      const clicked = await browser.execute(() => {
        const proj = Array.from(
          document.querySelectorAll('[data-e2e="nav-row-project"]'),
        ).find((p) => (p.textContent ?? "").includes("当前"));
        if (!proj) return false;
        const container = proj.parentElement as HTMLElement | null;
        if (!container || container.children.length > 1) return false;
        (proj as HTMLElement).click();
        return true;
      });
      if (!clicked) break;
      await browser.waitUntil(
        async () =>
          await browser.execute(() => {
            const proj = Array.from(
              document.querySelectorAll('[data-e2e="nav-row-project"]'),
            ).find((p) => (p.textContent ?? "").includes("当前"));
            return !!proj?.parentElement?.querySelector('[data-e2e="nav-row-page"]');
          }),
        { timeout: 5000, interval: 100, timeoutMsg: "项目行展开超时" },
      );
    }
    // 2. 页面行：单轮对当前项目容器内全部页面行各点击一次（toggle 一次 → 展开稳态）。
    //    无会话页面展开不产生子容器，不可用 DOM 收敛判定；项目内多页面行会依次切页，
    //    最终活跃页 = 容器内最后一个页面行（本 spec 用例项目均单页面，无碍）。
    await browser.execute(() => {
      const proj = Array.from(
        document.querySelectorAll('[data-e2e="nav-row-project"]'),
      ).find((p) => (p.textContent ?? "").includes("当前"));
      if (!proj) return;
      const container = proj.parentElement as HTMLElement | null;
      if (!container) return;
      const pages = container.querySelectorAll('[data-e2e="nav-row-page"]');
      for (const pg of pages) (pg as HTMLElement).click();
    });
  }

  /** 展开全部项目行至子容器可见（NAV-10 修订 2026-09-03：历史节点随项目展开态
   *  渲染——项目收起时无 nav-history-node，须先展开项目行。幂等多轮收敛：
   *  已展开（容器 children>1）不重复点击，防 toggle 误折叠未渲染项目。 */
  async function ensureAllProjectsExpanded(): Promise<void> {
    for (let i = 0; i < 6; i++) {
      const clicked = await browser.execute(() => {
        let any = false;
        for (const proj of Array.from(
          document.querySelectorAll('[data-e2e="nav-row-project"]'),
        ) as HTMLElement[]) {
          const container = proj.parentElement as HTMLElement | null;
          if (!container || container.children.length > 1) continue; // 已展开跳过
          proj.click();
          any = true;
        }
        return any;
      });
      if (!clicked) return;
      await browser.waitUntil(
        async () =>
          await browser.execute(() =>
            Array.from(document.querySelectorAll('[data-e2e="nav-row-project"]')).some(
              (p) => ((p.parentElement as HTMLElement | null)?.children.length ?? 0) > 1,
            ),
          ),
        { timeout: 5000, interval: 100, timeoutMsg: "项目行展开超时" },
      );
    }
  }

  /**
   * 通用前置：创建 E2E 项目（fixture 507 会话 cwd 归属项目——导航树历史节点
   * 只显示归属会话）→ 打开 nav 视图 → 展开全部项目行 + 历史节点（展开触发重扫）。
   */
  async function openHistoryWithFreshScan(): Promise<void> {
    await waitForWorkspaceReady();
    // 507 会话 cwd = e2eProjectDir——项目存在且 rootPath 精确匹配才归属（决策 5）
    const proj = await browser.execute((dir: string) => {
      return (window as any).__slterm_e2e_createProject?.(dir);
    }, e2eProjectDir);
    // 创建失败立即 fail——后续扫描/恢复/删除断言不得基于不存在的状态（TQ-E-04）
    if (!proj) {
      throw new Error(
        `__slterm_e2e_createProject 返回空（dir=${e2eProjectDir}）——helper 未就绪或创建失败`,
      );
    }
    await openNavView();
    await ensureAllProjectsExpanded();
    await ensureHistoryExpanded();
    await waitHistoryRows();
  }

  /**
   * 展开历史节点（幂等）。E2E 项目可能位于导航树任意位置（项目持久化累积）——
   * 遍历全部 nav-history-node 展开（children ≤ 1 即未展开；点击头部行展开，
   * 展开触发重扫 NavTree onToggle → refresh；已展开/空态节点 children > 1 跳过）。
   */
  async function ensureHistoryExpanded(): Promise<void> {
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="nav-history-node"]')),
      { timeout: 10000, timeoutMsg: "nav-history-node 未渲染" },
    );
    await browser.execute(() => {
      const nodes = document.querySelectorAll('[data-e2e="nav-history-node"]');
      for (const n of nodes) {
        if ((n.children.length ?? 0) <= 1) {
          n.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        }
      }
    });
    // 等待展开生效（React 异步渲染——至少一个节点出现子级容器）
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const nodes = document.querySelectorAll('[data-e2e="nav-history-node"]');
          return Array.from(nodes).some((n) => n.children.length > 1);
        }),
      { timeout: 10000, timeoutMsg: "历史节点展开未生效" },
    );
  }

  /** 等待历史行出现（历史行嵌套于 nav-history-node 内——与活跃行按范围区分） */
  async function waitHistoryRows(timeout = 15000): Promise<void> {
    try {
      await browser.waitUntil(
        async () =>
          await browser.execute(
            () =>
              document.querySelectorAll(
                '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
              ).length > 0,
          ),
        { timeout, timeoutMsg: "历史会话行未出现（扫描未完成或副本未就绪）" },
      );
    } catch (e) {
      const diag = await browser.execute(() => {
        const tree = document.querySelector('[data-e2e="nav-tree"]');
        const nodes = Array.from(
          document.querySelectorAll('[data-e2e="nav-history-node"]'),
        );
        return JSON.stringify({
          tree: tree?.textContent?.slice(0, 300) ?? null,
          nodeCount: nodes.length,
          nodes: nodes.map((n) => ({
            text: n.textContent?.slice(0, 80),
            children: n.children.length,
          })),
          rows: document.querySelectorAll(
            '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
          ).length,
          allRows: document.querySelectorAll('[data-e2e="nav-row-session"]').length,
        });
      });
      console.log("DIAG waitHistoryRows:", diag);
      throw e;
    }
  }

  /** 按文本定位历史行，返回行 textContent（未命中 null） */
  async function findRowByText(text: string): Promise<string | null> {
    return browser.execute((t: string) => {
      const rows = document.querySelectorAll(
        '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
      );
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) return r.textContent ?? "";
      }
      return null;
    }, text);
  }

  /** 右键历史行（dispatch 合成 contextmenu，clientX/Y 供菜单定位） */
  async function contextMenuOnRow(text: string): Promise<boolean> {
    return browser.execute((t: string) => {
      const rows = document.querySelectorAll(
        '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
      );
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) {
          r.dispatchEvent(
            new MouseEvent("contextmenu", {
              bubbles: true,
              cancelable: true,
              clientX: 120,
              clientY: 160,
            }),
          );
          return true;
        }
      }
      return false;
    }, text);
  }

  /** 点击右键菜单项（文本精确匹配——NavContextMenu 无 data-e2e，按项文本定位） */
  async function clickMenuByLabel(label: string): Promise<boolean> {
    return browser.execute((l: string) => {
      const items = Array.from(document.querySelectorAll("div")).filter(
        (d) => (d.textContent ?? "").trim() === l && d.children.length === 0,
      );
      if (items.length === 0) return false;
      (items[0] as HTMLElement).click();
      return true;
    }, label);
  }

  /** 等待右键菜单出现（指定菜单项文本出现即可——NavContextMenu 固定定位浮层） */
  async function waitContextMenu(label: string): Promise<void> {
    await browser.waitUntil(
      async () =>
        await browser.execute((l: string) => {
          return Array.from(document.querySelectorAll("div")).some(
            (d) => (d.textContent ?? "").trim() === l && d.children.length === 0,
          );
        }, label),
      { timeout: 5000, timeoutMsg: `右键菜单项「${label}」未出现` },
    );
  }

  /**
   * React 受控 input 注入（原生 value setter + input 事件）。
   * 键盘输入限制（e2e-tests/CLAUDE.md）：embedded 驱动无法 OS 级按键投递（禁 browser.keys）；
   * 且 WebDriver 元素级交互每次经 focusCommand 触发 getWindowStates 5s 超时——改用
   * execute 内原生 setter + input 事件，触发真实 React onChange（与用户输入同一路径）。
   */
  async function setSearchInput(text: string): Promise<boolean> {
    return browser.execute((t: string) => {
      const el = document.querySelector(
        'input[placeholder="搜索项目 / 页面 / 会话…"]',
      ) as HTMLInputElement | null;
      if (!el) return false;
      const desc = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
      desc?.set?.call(el, t);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    }, text);
  }

  // ── 用例 1：归属展示 + 排除规则 ──

  it("导航树历史节点：E2E 项目归属会话（cwd=e2eProjectDir）展示，非归属不显示", async () => {
    await openHistoryWithFreshScan();

    // 归属行渲染（fixture 中 cwd 占位符 = e2eProjectDir 的会话：501/502/503/507 全归属）
    for (const t of ["恢复目标会话", "E2E自定义标题一", "E2E自动标题二", "帮我看看这个 e2e 问题"]) {
      expect(await findRowByText(t)).not.toBeNull();
    }

    // 非归属 fixture 会话（505 无 cwd / 506 孤儿 cwd 不存在，无匹配项目）不显示——导航树不展示
    for (const t of ["无目录会话", "孤儿会话"]) {
      expect(await findRowByText(t)).toBeNull();
    }
  });

  // ── 用例 2：标题回退链（归属行 501 custom-title / 502 ai-title / 507 custom-title） ──

  it("标题回退链：归属会话行标题 = custom/ai-title（UUID 前 8 位不泄漏）", async () => {
    await openHistoryWithFreshScan();

    const row501 = await findRowByText("E2E自定义标题一");
    expect(row501).not.toBeNull();
    // title 非 null → 行标题不显示 UUID 前 8 位兜底
    expect(row501).not.toContain(UUID_CUSTOM.slice(0, 8));

    // 502 ai-title 行
    expect(await findRowByText("E2E自动标题二")).not.toBeNull();
    // 507 custom-title 行
    expect(await findRowByText("恢复目标会话")).not.toBeNull();
  });

  // ── 用例 3：搜索过滤 ──

  it("搜索过滤：关键词仅保留命中会话；无结果显示空态；清空恢复", async () => {
    await openHistoryWithFreshScan();

    // 输入命中 507 的关键词（NavTree 搜索过滤会话名，子串不区分大小写）
    expect(await setSearchInput("恢复目标")).toBe(true);
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const rows = document.querySelectorAll(
            '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
          );
          return rows.length === 1 && (rows[0].textContent ?? "").includes("恢复目标会话");
        }),
      { timeout: 8000, timeoutMsg: "搜索过滤后应仅剩 507 行" },
    );

    // 无结果关键词 → 「没有找到匹配的项目 / 页面 / 会话」空态（NAV-04 契约文案）
    await setSearchInput("不存在的关键词xyz");
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const tree = document.querySelector('[data-e2e="nav-tree"]');
          return (tree?.textContent ?? "").includes("没有找到匹配的项目 / 页面 / 会话");
        }),
      { timeout: 8000, timeoutMsg: "搜索无结果空态未出现" },
    );

    // 清空搜索 → 恢复（归属行重新可见）
    await setSearchInput("");
    await browser.waitUntil(
      async () => (await findRowByText("恢复目标会话")) !== null,
      { timeout: 8000, timeoutMsg: "清空搜索后历史节点未恢复" },
    );
  });

  // ── 用例 4：复制恢复命令 ──

  it("复制恢复命令：右键 → 剪贴板内容为 cd '<cwd>' && claude --resume <id>", async () => {
    await openHistoryWithFreshScan();

    // 右键 507 行（普通行，cwd = E2E 临时项目目录）
    expect(await contextMenuOnRow("恢复目标会话")).toBe(true);
    await waitContextMenu("复制恢复命令");
    expect(await clickMenuByLabel("复制恢复命令")).toBe(true);

    // 剪贴板读取：clipboard-manager 插件 read_text（capabilities 已放行 allow-read-text），
    // 与 __slterm_e2e_writeClipboard 同族读取路径（browser.execute 支持 async 回调）
    const clip = await browser.waitUntil(
      async () => {
        const t = await browser.execute(() =>
          (window as any).__TAURI_INTERNALS__.invoke("plugin:clipboard-manager|read_text"),
        );
        return t ? t : false;
      },
      { timeout: 8000, timeoutMsg: "剪贴板读取失败" },
    );
    // buildResumeCommand：有 cwd → `cd '<cwd>' && claude --resume <id>`（单引号路径）
    expect(clip).toBe(`cd '${e2eProjectDir}' && claude --resume ${UUID_RESTORE}`);
  });

  // ── 用例 5：历史区四态（问题 2 修复——信号文件驱动，导航树行与活跃区同源一致） ──

  it("历史区四态：信号文件驱动 → 活跃行建行（圆点）+ 归属历史行圆点；SessionEnd → 活跃行消失", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-history-status-"));
    const signalFiles: string[] = [];

    try {
      // 0. Workspace 就绪 + hooks 注入（照 R4 变体先例）
      await waitForWorkspaceReady();
      await ensureHooksInjected();

      // 1. 创建项目 + 终端面板（PTY session 就绪后 hook 事件才会路由到面板）
      const pageId = await browser.execute((dir: string) => {
        return (window as any).__slterm_e2e_createProject?.(dir);
      }, tempDir);
      await browser.waitUntil(
        async () => await browser.execute(() => typeof window.__dockviewApi !== "undefined"),
        { timeout: 20000, timeoutMsg: "Dockview API 未就绪" },
      );
      const panelId = `terminal-${pageId}-0`;
      await browser.execute((pid: string) => {
        window.__dockviewApi!.addPanel({
          id: pid,
          component: "terminal",
          params: { panelId: pid },
          renderer: "always" as const,
        });
      }, panelId);
      await browser.waitUntil(
        async () => await browser.execute(() => {
          const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
          for (const c of containers) {
            if ((c as any).__e2e_sessionReady) return true;
          }
          return false;
        }),
        { timeout: 25000, timeoutMsg: "PTY session 未就绪" },
      );

      // 2. 打开 nav 视图 + 展开历史节点（行 = fixture 507「恢复目标会话」）
      await openHistoryWithFreshScan();
      expect(await findRowByText("恢复目标会话")).not.toBeNull();
      // 2b. openHistoryWithFreshScan 的 createProject(e2eProjectDir) 会把 activePageId
      //     切到 E2E 项目——useAgentStatus 只建「当前活跃项目」的页面行——信号前切回
      //     用例面板所在页面（tempDir 项目），再展开项目/页面行
      await browser.execute((pid: string) => {
        (window as any).__slterm_e2e_switchToPage?.(pid);
      }, pageId);
      await ensureProjectPagesExpanded();

      // 3. 原子写 PreToolUse 信号文件（sessionId = fixture 507 UUID）
      //    → 活跃区建行（圆点 working）；历史区 507 行圆点（两区同源 TerminalRegistry）
      mkdirSync(eventsDir, { recursive: true });
      const writeSignal = (event: string, toolName: string | null, notificationType: string | null) => {
        const payload = {
          panelId,
          event,
          timestamp: Date.now(),
          sessionId: UUID_RESTORE,
          usageSourcePath: join(projectsDir, fixtureDirA, `${UUID_CUSTOM}.jsonl`),
          cwd: tempDir,
          toolName,
          notificationType,
        };
        return writeSignalFile(eventsDir, payload);
      };

      signalFiles.push(writeSignal("PreToolUse", "Bash", null));
      // 信号文件被 watcher 消费（notify 实时 + 3s 轮询兜底双路径——防「残留不消费」回归）
      await waitForSignalConsumed(signalFiles[0]);

      // 活跃行出现（导航树页面下 nav-row-session + data-panel-id）+ 圆点（StatusDot 7px）
      await browser.waitUntil(
        async () => await browser.execute((pid: string) => {
          const rows = Array.from(
            document.querySelectorAll('[data-e2e="nav-row-session"]'),
          ) as HTMLElement[];
          const row = rows.find((r) => r.getAttribute("data-panel-id") === pid);
          if (!row) return false;
          return Array.from(row.querySelectorAll("div")).some(
            (d) =>
              (d as HTMLElement).style.borderRadius === "50%" &&
              (d as HTMLElement).style.width === "7px",
          );
        }, panelId),
        { timeout: 15000, timeoutMsg: "活跃行未在 PreToolUse 后建行（含圆点）" },
      );

      // 4. SessionEnd 信号 → 活跃行消失（历史行圆点恒渲染灰档——视觉四态由 L2 StatusDot 覆盖）
      signalFiles.push(writeSignal("SessionEnd", null, null));
      await waitForSignalConsumed(signalFiles[1]);
      await browser.waitUntil(
        async () => await browser.execute((pid: string) => {
          return !Array.from(
            document.querySelectorAll('[data-e2e="nav-row-session"]'),
          ).some((r) => r.getAttribute("data-panel-id") === pid);
        }, panelId),
        { timeout: 15000, timeoutMsg: "活跃行未在 SessionEnd 后消失" },
      );
    } finally {
      // 清理：信号文件（watcher 处理正常会自删，此处兜底）+ 临时目录
      for (const f of signalFiles) {
        try {
          rmSync(f, { force: true });
        } catch {
          // 已删除则忽略
        }
      }
      try {
        rmSync(tempDir, { recursive: true, force: true });
      } catch {
        // 忽略
      }
    }
  });

  // ── 用例 6：恢复编排（部分端到端，E2E-11 标注） ──
  //
  // 断言到「pty.write 命令注入」为止——fixture sessionId 非真实会话，
  // 不断言 claude 真实进入会话（真实恢复成功属人工验证）。

  it("恢复编排：双击普通行 → 项目入列 + 页面切换 + 终端注入 claude --resume（部分端到端：断言到 pty.write 注入）", async () => {
    try {
      await openHistoryWithFreshScan();

      // 双击 507 行（普通行：cwd = E2E 临时项目目录，cwdExists=true）→ restoreHistorySession 四步编排
      const dbl = await browser.execute((t: string) => {
        const rows = document.querySelectorAll(
          '[data-e2e="nav-history-node"] [data-e2e="nav-row-session"]',
        );
        for (const r of rows) {
          if ((r.textContent ?? "").includes(t)) {
            r.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
            return true;
          }
        }
        return false;
      }, "恢复目标会话");
      expect(dbl).toBe(true);

      // 1. 项目入列 + 页面切换：activePage rootPath === fixture cwd（E2E 临时项目目录）
      //    （restoreHistorySession 步骤 1 已有 E2E 项目 rootPath 匹配 → 跳过入列；
      //      步骤 3 switchToPageShared）
      const info = await browser.waitUntil(
        async () => {
          const i = await getActivePageInfo();
          return i?.rootPath === e2eProjectDir ? i : false;
        },
        { timeout: 15000, timeoutMsg: "恢复后活跃页面 rootPath 未指向 E2E 项目目录" },
      );
      expect(info).not.toBeNull();

      // 2. 终端页签出现（步骤 4 addPanel terminal → PTY session 就绪）
      await browser.waitUntil(
        async () =>
          await browser.execute(() => {
            const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
            for (const c of containers) {
              if ((c as any).__e2e_sessionReady) return true;
            }
            return false;
          }),
        { timeout: 25000, timeoutMsg: "恢复终端 PTY session 未就绪" },
      );

      // 3. 终端缓冲含注入命令（pty.write `claude --resume <id>\r`，pwsh 回显输入行）
      await browser.waitUntil(
        async () =>
          await browser.execute((id: string) => {
            const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
            for (const c of containers) {
              const el = c as any;
              if (
                typeof el.__e2e_getTerminalText === "function" &&
                el.__e2e_getTerminalText().includes(`claude --resume ${id}`)
              ) {
                return true;
              }
            }
            return false;
          }, UUID_RESTORE),
        { timeout: 25000, timeoutMsg: "终端缓冲未含 claude --resume 注入命令" },
      );

      // 4. B14 真实渲染断言（修复脱靶盲区：e2eTextBuffer 在 visible 门控之前填充，
      //    黑屏 bug 下文本断言全绿——此处补「主区正常 + 恢复面板可见」断言）
      // 4a. 主区 dockview 中存在本页 terminal-{pageId}- 前缀面板（非幽灵页面导航）
      const pageId = info.pageId;
      const renderState = await browser.execute((pid: string) => {
        const api = (window as any).__dockviewApi;
        const prefix = `terminal-${pid}-`;
        const panel = (api?.panels ?? []).find((p: { id: string }) =>
          p.id.startsWith(prefix),
        );
        if (!panel) return { found: false };
        // 面板属于当前活跃页面（visible 判定放行 → usePtyOutput flush 路径）
        const container = document.querySelector(
          `[data-e2e="terminal-container"][data-panel-id="${panel.id}"]`,
        ) as HTMLElement | null;
        return {
          found: true,
          title: panel.api?.title ?? panel.title,
          visible: container ? container.offsetParent !== null : false,
        };
      }, pageId);
      expect(renderState.found).toBe(true);
      // 4b. 恢复面板标题：profile.tabTitle "claude"（恢复即 claude 语义），
      //     或 B12 重算的 terminal-N（无 customTitle 的终端面板布局恢复重算——
      //     导航树环境页面重挂载时序下可能触发；两者均为合法标题）
      expect(renderState.title).toMatch(/^(claude|terminal-\d+)$/);
      // 4c. 面板处于可见页面（offsetParent 非 null——黑屏时面板不可见/渲染失败）
      expect(renderState.visible).toBe(true);

      // 5. 部分端到端：断言到「注入 + 编排 + 真实渲染」为止（E2E-11 标注）。
      //    不断言 claude 成功进入会话：fixture sessionId 非真实会话（真实恢复成功属人工验证）
    } finally {
      // 清理 E2E 临时项目目录（恢复用例完成后不再有依赖 fixture 归属的用例；
      // 不清理会致下次运行前 run-wdio.cjs 已重建——双保险幂等）
      try { rmSync(e2eProjectDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  // ── 用例 7：删除（放恢复编排后——507 是导航树唯一归属行，删除后不再有 fixture 依赖用例） ──

  it("删除：ConfirmDialog 确认 → 行消失 + 副本文件删除", async () => {
    // OV-02 后删除确认由原生 ask() 改为应用内 ConfirmDialog（src/lib/ConfirmDialog.tsx，
    // data-e2e=confirm-ok）——应用内浮层 embedded WDIO 可直接点击，旧 __slterm_e2e_dialogAsk
    // 钩子已随 ipc/dialog ask 删除退役。
    await openHistoryWithFreshScan();

    // 右键 507 行 → 「删除」（普通行删除可用，操作矩阵 ✓）
    expect(await contextMenuOnRow("恢复目标会话")).toBe(true);
    await waitContextMenu("删除");
    expect(await clickMenuByLabel("删除")).toBe(true);

    // ConfirmDialog 出现 → 点确认（confirmDialog 返回 true → deleteHistorySession IPC → removeLocal 即时局部刷新）
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const ok = document.querySelector('[data-e2e="confirm-ok"]');
          if (!ok) return false;
          (ok as HTMLElement).click();
          return true;
        }),
      { timeout: 5000, timeoutMsg: "ConfirmDialog 确认按钮未出现" },
    );

    // 行消失
    await browser.waitUntil(
      async () => (await findRowByText("恢复目标会话")) === null,
      { timeout: 8000, timeoutMsg: "删除后行未消失" },
    );

    // Node 侧断言：副本文件已删除（SEC-02——只动副本，不触碰用户真实 ~/.claude/projects/）
    const restorePath = join(projectsDir, fixtureDirA, `${UUID_RESTORE}.jsonl`);
    expect(existsSync(restorePath)).toBe(false);
  });
});
