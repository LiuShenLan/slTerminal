/**
 * Claude 历史会话视图域 E2E spec（E2E-09 拆分 + E2E-11 恢复编排标注）：
 * 展示/排除规则、标题回退链、搜索过滤、复制恢复命令、孤儿行、
 * 删除、历史区四态（信号文件驱动）、恢复编排（部分端到端）。
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

// ── Claude 历史会话视图（TE-01..03） ──
//
// 数据隔离（SEC-02 安全红线）：后端 agent_history 扫描根由 run-wdio.cjs 指向
// e2e-tests/.tmp-claude-projects/ 副本（每次运行从 fixtures/claude-projects/ 重建）。
// 本 describe 全部读写在副本内进行，删除用例只动副本文件，不触碰用户真实
// ~/.claude/projects/。恢复编排用例的项目根 = run-wdio.cjs 创建的 E2E 临时项目目录
// （process.env.SLTERM_E2E_PROJECT_DIR；fixture cwd 占位符已替换为该真实路径）。
// 用例顺序约定：展示/搜索/复制/孤儿（只读）→ 删除（写副本）→
// 恢复编排（最后——finally 删除 E2E 临时项目目录，故其后不再有依赖 fixture 的用例）。
describe("Claude 历史会话视图", () => {
  // fixture 会话 UUID（与 fixtures/claude-projects/ 逐字对应）
  const UUID_CUSTOM = "11111111-2222-4333-8444-555555555501"; // 形态1 custom-title
  const UUID_PROMPT = "11111111-2222-4333-8444-555555555503"; // 形态3 回退首条 prompt
  const UUID_ORPHAN = "11111111-2222-4333-8444-555555555506"; // 形态5 孤儿
  const UUID_RESTORE = "11111111-2222-4333-8444-555555555507"; // 恢复编排目标（cwd→E2E 项目目录）
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

  /** 打开 agent-status 视图（幂等：已打开不重复 toggle，防 R2 关闭） */
  async function openAgentStatusView(): Promise<void> {
    await waitForWorkspaceReady();
    const s = await browser.execute(() => (window as any).__slterm_e2e_getSideBarState?.() ?? null);
    if (s?.open.top !== "agent-status") {
      await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("agent-status"));
    }
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="agent-status-view"]')),
      { timeout: 10000, timeoutMsg: "agent-status 视图未渲染" },
    );
  }

  /**
   * 确保「全部项目历史会话」区展开（幂等：收起才点击；已展开不动）。
   * 展开态检测（IC-05 箭头 chevron 化后无 ▶/▼ 文本）：区容器子节点数 > 1
   * （1 = 仅标题栏，2 = 标题栏 + 内容区）。
   */
  async function ensureAllSectionExpanded(): Promise<void> {
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="agent-history-section-all"]')),
      { timeout: 10000, timeoutMsg: "agent-history-section-all 未渲染" },
    );
    const expanded = await browser.execute(() => {
      const section = document.querySelector('[data-e2e="agent-history-section-all"]');
      return (section?.children.length ?? 0) > 1;
    });
    if (!expanded) {
      await browser.execute(() => {
        const section = document.querySelector('[data-e2e="agent-history-section-all"]');
        section?.firstElementChild?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      });
    }
  }

  /**
   * 展开全部项目区所有组（问题 3 修复：组默认收起——行操作用例需组内行可见）。
   * 组展开态检测（IC-05 箭头 chevron 化后无 ▼ 文本）：组标题的父容器子节点数 > 1
   * （1 = 仅标题栏，2 = 标题栏 + 组内行容器）；收起组点击展开。
   * 时序：展开 all 区后 React 异步渲染组（dispatchEvent 非 React 事件系统，
   * setState 为异步批处理）——先轮询组渲染，再点击展开，再等行出现（展开生效）。
   */
  async function ensureAllGroupsExpanded(): Promise<void> {
    await browser.waitUntil(
      async () =>
        await browser.execute(
          () =>
            document.querySelectorAll('[data-e2e="agent-history-group"]').length >
            0,
        ),
      { timeout: 10000, timeoutMsg: "历史分组未渲染" },
    );
    await browser.execute(() => {
      const groups = document.querySelectorAll('[data-e2e="agent-history-group"]');
      for (const g of groups) {
        const parent = g.parentElement;
        const expanded = (parent?.children.length ?? 0) > 1;
        if (!expanded) {
          g.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        }
      }
    });
    // 等待展开生效（React 异步渲染组内行）
    await browser.waitUntil(
      async () =>
        await browser.execute(
          () =>
            document.querySelectorAll('[data-e2e="agent-history-row"]').length >
            0,
        ),
      { timeout: 10000, timeoutMsg: "历史组展开后行未出现" },
    );
  }

  /**
   * 点击刷新触发重扫（ClaudeHistorySections 的 scanTriggeredRef 仅首次展开自动 scan，
   * 之后靠刷新按钮——每个用例点刷新保证读到副本磁盘最新，跨用例删除不残留旧列表）
   */
  async function refreshHistory(): Promise<void> {
    await browser.execute(() => {
      const btn = document.querySelector('[data-e2e="agent-history-refresh"]') as HTMLElement | null;
      btn?.click();
    });
  }

  /** 等待历史行出现（刷新后重扫完成）；超时附带 DOM 诊断 */
  async function waitRows(timeout = 15000): Promise<void> {
    try {
      await browser.waitUntil(
        async () =>
          await browser.execute(() => document.querySelectorAll('[data-e2e="agent-history-row"]').length > 0),
        { timeout, timeoutMsg: "历史会话行未出现（扫描未完成或副本未就绪）" },
      );
    } catch (e) {
      const diag = await browser.execute(() => {
        const all = document.querySelector('[data-e2e="agent-history-section-all"]');
        const current = document.querySelector('[data-e2e="agent-history-section-current"]');
        return JSON.stringify({
          all: all?.textContent ?? null,
          current: current?.textContent ?? null,
          groups: document.querySelectorAll('[data-e2e="agent-history-group"]').length,
          rows: document.querySelectorAll('[data-e2e="agent-history-row"]').length,
        });
      });
      console.log("DIAG waitRows:", diag);
      throw e;
    }
  }

  /**
   * 通用前置：打开视图 → 展开全部区 → 刷新 → 展开所有组 → 等行。
   * 顺序约束：组展开必须在 refreshHistory（重扫）之后——scan 的 loading 分支会卸载重挂
   * HistorySessionList（expandedGroups 为组件内 state，重挂后重置为默认收起），
   * 先展开再刷新会导致行随重挂消失。
   */
  async function openAllSectionWithFreshScan(): Promise<void> {
    await openAgentStatusView();
    await ensureAllSectionExpanded();
    await refreshHistory();
    await ensureAllGroupsExpanded();
    await waitRows();
  }

  /** 按文本定位历史行，返回行 textContent（未命中 null） */
  async function findRowByText(text: string): Promise<string | null> {
    return browser.execute((t: string) => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) return r.textContent ?? "";
      }
      return null;
    }, text);
  }

  /** 右键历史行（dispatch 合成 contextmenu，clientX/Y 供菜单定位；事件来源合成与既有 dblclick 用例同属先例） */
  async function contextMenuOnRow(text: string): Promise<boolean> {
    return browser.execute((t: string) => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
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

  /** 点击右键菜单项（文本精确匹配，返回是否命中） */
  async function clickMenuByLabel(label: string): Promise<boolean> {
    return browser.execute((l: string) => {
      const menu = document.querySelector('[data-e2e="agent-history-menu"]');
      if (!menu) return false;
      const items = menu.querySelectorAll(":scope > div");
      for (const item of items) {
        if ((item.textContent ?? "").trim() === l) {
          (item as HTMLElement).click();
          return true;
        }
      }
      return false;
    }, label);
  }

  /**
   * React 受控 input 注入（原生 value setter + input 事件）。
   * 键盘输入限制（e2e-tests/CLAUDE.md）：embedded 驱动无法 OS 级按键投递（禁 browser.keys）；
   * 且 WebDriver 元素级交互（$().setValue()）每次经 focusCommand 触发 getWindowStates 5s 超时
   * （P3-TE-18 注释先例，60s mocha 预算内不可接受）——改用 execute 内原生 setter + input 事件，
   * 触发真实 React onChange（与用户输入同一路径）。
   */
  async function setInputValue(selector: string, text: string): Promise<boolean> {
    return browser.execute((s: string, t: string) => {
      const el = document.querySelector(s) as HTMLInputElement | null;
      if (!el) return false;
      const desc = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
      desc?.set?.call(el, t);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    }, selector, text);
  }

  /** 等待右键菜单出现 */
  async function waitContextMenu(): Promise<void> {
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="agent-history-menu"]')),
      { timeout: 5000, timeoutMsg: "右键菜单未出现" },
    );
  }

  // ── 用例 1：展开 → 列表展示 + 排除规则 ──

  it("展开「全部项目历史会话」→ fixture 6 条会话行展示，agent-*/非 UUID/subagents 不出现", async () => {
    await openAllSectionWithFreshScan();

    // fixture 7 形态中 6 条应展示（501/502/503/505/506/507）；
    // agent-misc.jsonl（形态6 平铺）、not-a-uuid.jsonl（非 UUID）、
    // 504/subagents/agent-child.jsonl（形态7 子目录）均应被扫描排除
    const rowCount = await browser.execute(
      () => document.querySelectorAll('[data-e2e="agent-history-row"]').length,
    );
    expect(rowCount).toBe(6);

    // 排除文件内容不出现在全部区
    const allText = await browser.execute(() => {
      const section = document.querySelector('[data-e2e="agent-history-section-all"]');
      return section?.textContent ?? "";
    });
    expect(allText).not.toContain("agent 平铺会话");
    expect(allText).not.toContain("子代理会话");
    expect(allText).not.toContain("agent-misc");

    // 6 条会话标题/提示均在列表
    for (const t of [
      "E2E自定义标题一",
      "E2E自动标题二",
      "帮我看看这个 e2e 问题",
      "无目录会话",
      "孤儿会话",
      "恢复目标会话",
    ]) {
      expect(await findRowByText(t)).not.toBeNull();
    }
  });

  // ── 用例 2：标题回退链 ──

  it("标题回退链：custom-title / ai-title / 首条 prompt 三会话行各显示预期标题", async () => {
    await openAllSectionWithFreshScan();

    // 形态1：含 custom-title 行 → 标题取 custom-title
    expect(await findRowByText("E2E自定义标题一")).not.toBeNull();
    // 形态2：含 ai-title 行 → 标题取 ai-title（赢 summary）
    expect(await findRowByText("E2E自动标题二")).not.toBeNull();
    // 形态3：无标题行 → 回退首条可见 prompt（isMeta/数组/< 开头/空白干扰行均被跳过）
    const promptRow = await findRowByText("帮我看看这个 e2e 问题");
    expect(promptRow).not.toBeNull();
    // 干扰行内容不泄漏为标题/预览
    const allText = await browser.execute(() => {
      const section = document.querySelector('[data-e2e="agent-history-section-all"]');
      return section?.textContent ?? "";
    });
    expect(allText).not.toContain("<command-name>");
    // title 非 null → 行标题不显示 UUID 前 8 位兜底
    expect(promptRow).not.toContain(UUID_PROMPT.slice(0, 8));
  });

  // ── 用例 3：搜索过滤 ──

  it("搜索过滤：关键词仅保留匹配行；无结果显示「无匹配的会话」", async () => {
    await openAllSectionWithFreshScan();

    // 输入唯一命中 501 的关键词（matchesSearch 匹配标题 + firstPrompt，大小写不敏感）
    expect(await setInputValue('[data-e2e="agent-history-search"]', "E2E自定义")).toBe(true);
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
          return rows.length === 1 && (rows[0].textContent ?? "").includes("E2E自定义标题一");
        }),
      { timeout: 8000, timeoutMsg: "搜索过滤后应仅剩 501 行" },
    );

    // 无结果关键词 → 「无匹配的会话」提示（全部区空态文案）
    await setInputValue('[data-e2e="agent-history-search"]', "不存在的关键词xyz");
    await browser.waitUntil(
      async () =>
        await browser.execute(() => {
          const section = document.querySelector('[data-e2e="agent-history-section-all"]');
          return (section?.textContent ?? "").includes("无匹配的会话");
        }),
      { timeout: 8000, timeoutMsg: "搜索无结果提示未出现" },
    );

    // 清空搜索 → 恢复 6 行（删除用例尚未执行，行数仍为 fixture 全集）
    await setInputValue('[data-e2e="agent-history-search"]', "");
    await browser.waitUntil(
      async () =>
        await browser.execute(() => document.querySelectorAll('[data-e2e="agent-history-row"]').length === 6),
      { timeout: 8000, timeoutMsg: "清空搜索后未恢复 6 行" },
    );
  });

  // ── 用例 4：复制恢复命令 ──

  it("复制恢复命令：右键 → 剪贴板内容为 cd '<cwd>' && claude --resume <id>", async () => {
    await openAllSectionWithFreshScan();

    // 右键 507 行（普通行，cwd = E2E 临时项目目录）
    expect(await contextMenuOnRow("恢复目标会话")).toBe(true);
    await waitContextMenu();
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

  // ── 用例 5：孤儿行 ──

  it("孤儿行孤儿标记展示 + 双击无反应（无新面板/无页面切换）", async () => {
    await openAllSectionWithFreshScan();

    // 形态5（cwd 指向不存在路径）→ 孤儿行显示 IconClose 孤儿标记（IC-08：✗ 字符清除）
    const orphanRowHasMark = await browser.execute(() => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
      for (const r of rows) {
        if ((r.textContent ?? "").includes("孤儿会话")) {
          return !!r.querySelector('[data-e2e="agent-history-orphan"]');
        }
      }
      return false;
    });
    expect(orphanRowHasMark).toBe(true);
    // 普通行（cwd 存在）不显示孤儿标记
    const normalRowHasMark = await browser.execute(() => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
      for (const r of rows) {
        if ((r.textContent ?? "").includes("恢复目标会话")) {
          return !!r.querySelector('[data-e2e="agent-history-orphan"]');
        }
      }
      return false;
    });
    expect(normalRowHasMark).toBe(false);

    // 双击前快照：活跃页面 + 全部面板 id 集合
    const snapshot = (): Promise<{ pageId: string | null; panels: string }> =>
      browser.execute(() => {
        const info = (window as any).__slterm_e2e_getActivePageInfo?.() ?? null;
        const panelIds: string[] = [];
        const api = (window as any).__dockviewApi;
        for (const g of api?.groups ?? []) {
          for (const p of g.panels ?? []) panelIds.push(p.id);
        }
        return { pageId: info?.pageId ?? null, panels: panelIds.sort().join(",") };
      });
    const before = await snapshot();

    // 双击孤儿行 → 分派矩阵：孤儿 → 无操作（不新建面板、不切页面）
    const dbl = await browser.execute((t: string) => {
      const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
      for (const r of rows) {
        if ((r.textContent ?? "").includes(t)) {
          r.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
          return true;
        }
      }
      return false;
    }, "孤儿会话");
    expect(dbl).toBe(true);

    // E2E-10 负面断言：双击孤儿行不应产生任何副作用（不新建面板、不切页面）。
    // 负面语义（"持续不发生"）无法用 waitUntil 轮询表达，故用短轮询多次快照比对：
    // 4 轮 × 200ms（共 800ms，覆盖编排窗口），每轮快照必须与双击前完全一致。
    for (let i = 0; i < 4; i++) {
      await new Promise((r) => setTimeout(r, 200));
      const snap = await snapshot();
      expect(snap.pageId).toBe(before.pageId);
      expect(snap.panels).toBe(before.panels);
    }
  });

  // ── 用例 6：删除（重命名功能已整体移除——问题 7 修复，原用例 6/7 顺延） ──

  it("删除：ask 确认（E2E 钩子）→ 行消失 + 副本文件删除", async () => {
    // ask 弹窗处理（执行期决策点）：embedded WDIO 无法操作原生对话框；JS 侧 patch
    // `window.__TAURI_INTERNALS__.invoke` 不可行（Tauri 2 双层锁死，描述符探针实测
    // writable/configurable 全 false）；@wdio/tauri-service 的 browser.tauri.mock 在
    // embedded 模式无 core.invoke 通道（"Tauri core.invoke not available after 5s
    // timeout"）。故用 src/ipc/dialog.ts 的 E2E 钩子（E2E_ENABLED 门控，生产 tree-shake）：
    // 设置 window.__slterm_e2e_dialogAsk=true 等效用户点确认。真实原生弹窗交互属人工验收。
    await browser.execute(() => {
      (window as any).__slterm_e2e_dialogAsk = true;
    });

    await openAllSectionWithFreshScan();

    // 右键孤儿行 → 「删除」（孤儿行删除可用，操作矩阵 ✓）
    expect(await contextMenuOnRow("孤儿会话")).toBe(true);
    await waitContextMenu();
    expect(await clickMenuByLabel("删除")).toBe(true);

    // 行消失（ask 拦截返回 true → deleteHistorySession IPC → removeLocal 即时局部刷新）
    await browser.waitUntil(
      async () => (await findRowByText("孤儿会话")) === null,
      { timeout: 8000, timeoutMsg: "删除后行未消失" },
    );

    // Node 侧断言：副本文件已删除（SEC-02——只动副本，不触碰用户真实 ~/.claude/projects/）
    const orphanPath = join(projectsDir, fixtureDirB, `${UUID_ORPHAN}.jsonl`);
    expect(existsSync(orphanPath)).toBe(false);

    // 清理钩子（不泄漏到后续用例）
    await browser.execute(() => {
      delete (window as any).__slterm_e2e_dialogAsk;
    });
  });

  // ── 用例 7：历史区四态（问题 2 修复——信号文件驱动，历史区与活跃区同源一致） ──

  it("历史区四态：信号文件驱动 → 历史区行显示与活跃区一致的四态 emoji（⚡→✅→消失）", async () => {
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

      // 2. 打开 agent-status 视图 + 展开全部项目历史会话（行 = fixture 501「E2E自定义标题一」）
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });
      await openAllSectionWithFreshScan();
      expect(await findRowByText("E2E自定义标题一")).not.toBeNull();

      // 3. 原子写 PreToolUse 信号文件（sessionId = fixture 501 UUID；usageSourcePath 指向副本）
      //    → 活跃区建行 ⚡；历史区 501 行 ⚡（两区同源 TerminalRegistry，问题 2）
      mkdirSync(eventsDir, { recursive: true });
      const writeSignal = (event: string, toolName: string | null, notificationType: string | null) => {
        const payload = {
          panelId,
          event,
          timestamp: Date.now(),
          sessionId: UUID_CUSTOM,
          usageSourcePath: join(projectsDir, fixtureDirA, `${UUID_CUSTOM}.jsonl`),
          cwd: tempDir,
          toolName,
          notificationType,
        };
        return writeSignalFile(eventsDir, payload);
      };

      signalFiles.push(writeSignal("PreToolUse", "Bash", null));
      // 信号文件被 watcher 消费（notify 实时 + 3s 轮询兜底双路径——防「残留不消费」回归，
      // win10 实证 33 残留根因）
      await waitForSignalConsumed(signalFiles[0]);
      // 两区均出现 ⚡（working）
      await browser.waitUntil(
        async () => await browser.execute(() => {
          const active = document.querySelector('[data-e2e="agent-status-row"]');
          return active?.textContent?.includes("⚡") ?? false;
        }),
        { timeout: 15000, timeoutMsg: "agent-status-row 未在 PreToolUse 后含 ⚡" },
      );
      await browser.waitUntil(
        async () => (await findRowByText("E2E自定义标题一"))?.includes("⚡") ?? false,
        { timeout: 15000, timeoutMsg: "历史区 501 行未在 PreToolUse 后含 ⚡（四态未同步）" },
      );

      // 4. Stop 信号 → 两区均变 ✅（done），一致
      signalFiles.push(writeSignal("Stop", null, null));
      // 第二个信号文件同样被消费（轮询兜底连续生效）
      await waitForSignalConsumed(signalFiles[1]);
      await browser.waitUntil(
        async () => await browser.execute(() => {
          const active = document.querySelector('[data-e2e="agent-status-row"]');
          return active?.textContent?.includes("✅") ?? false;
        }),
        { timeout: 15000, timeoutMsg: "agent-status-row 未在 Stop 后含 ✅" },
      );
      await browser.waitUntil(
        async () => (await findRowByText("E2E自定义标题一"))?.includes("✅") ?? false,
        { timeout: 15000, timeoutMsg: "历史区 501 行未在 Stop 后含 ✅（四态未同步）" },
      );

      // 5. SessionEnd 信号 → 活跃区行消失 + 历史区 501 行标记消失（⚡ 集合重算）
      signalFiles.push(writeSignal("SessionEnd", null, null));
      await browser.waitUntil(
        async () => await browser.execute(() => !document.querySelector('[data-e2e="agent-status-row"]')),
        { timeout: 15000, timeoutMsg: "agent-status-row 未在 SessionEnd 后消失" },
      );
      await browser.waitUntil(
        async () => {
          const row = await findRowByText("E2E自定义标题一");
          return row !== null && !row.includes("⚡") && !row.includes("✅");
        },
        { timeout: 15000, timeoutMsg: "历史区 501 行标记未在 SessionEnd 后清除" },
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

  // ── 用例 8：恢复编排（最后——finally 删除 E2E 临时项目目录） ──
  //
  // 部分端到端（E2E-11 标注）：断言到「pty.write 命令注入」为止——
  // fixture sessionId 非真实会话，不断言 claude 真实进入会话（真实恢复成功属人工验证）。

  it("恢复编排：双击普通行 → 项目入列 + 页面切换 + 终端注入 claude --resume（部分端到端：断言到 pty.write 注入）", async () => {
    try {
      await openAllSectionWithFreshScan();

      // 双击 507 行（普通行：cwd = E2E 临时项目目录，cwdExists=true）→ restoreHistorySession 四步编排
      const dbl = await browser.execute((t: string) => {
        const rows = document.querySelectorAll('[data-e2e="agent-history-row"]');
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
      //    （restoreHistorySession 步骤 1 无匹配项目 → addProject(rootPath=cwd)；步骤 3 switchToPageShared）
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
      // 4b. 恢复面板标题 = profile.tabTitle "claude"（恢复即 claude 语义）
      expect(renderState.title).toBe("claude");
      // 4c. 面板处于可见页面（offsetParent 非 null——黑屏时面板不可见/渲染失败）
      expect(renderState.visible).toBe(true);

      // 5. 部分端到端：断言到「注入 + 编排 + 真实渲染」为止（E2E-11 标注）。
      //    不断言 claude 成功进入会话：fixture sessionId 非真实会话（真实恢复成功属人工验证）
    } finally {
      // 清理 E2E 临时项目目录（该用例是最后一个依赖 fixture 的用例；不清理会致
      // 下次运行前 run-wdio.cjs 已重建——双保险幂等）
      try { rmSync(e2eProjectDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });
});
