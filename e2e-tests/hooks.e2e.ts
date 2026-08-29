/**
 * hooks 域 E2E spec（E2E-09 拆分 + E2E-06 新用例）：
 * 注入/卸载/状态、信号文件驱动页签状态圆点、真实 hook reporter 链路（E2E-06）、
 * hooks 配置面板 hub 用例（P3-TE-18 保存链路 + D-14 Stage 06：选择行渲染/注入三态）。
 */

import { expect, browser } from "@wdio/globals";
import {
  mkdtempSync, mkdirSync, writeFileSync, readFileSync, statSync, rmSync, existsSync, readdirSync,
} from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPtySessionReady,
  createProject,
  addTerminalPanel,
  ensureHooksInjected,
  writeSignalFile,
  waitForSignalConsumed,
  waitForPanelTabStatus,
  defaultEventsDir,
  withProjectAndTerminal,
} from "./specUtils";

describe("hooks 状态可视化", () => {
  /**
   * 用例 1：注入后查询状态为 "injected"。
   * 调用 __slterm_e2e_injectHooks → getHookInjectionStatus → 断言 status。
   */
  it("注入后状态为 injected", async () => {
    // 0. 等待 Workspace 就绪
    await waitForWorkspaceReady();

    // 1. 先查询当前状态（可能已经是 injected，因为前序测试可能已注入）
    let preStatus: any = null;
    try {
      preStatus = await browser.execute(() =>
        (window as any).__slterm_e2e_getHookInjectionStatus?.(),
      );
    } catch { /* 首次查询可能因未注入而失败，忽略 */ }

    // 2. 如果尚未注入，则调用注入
    if (!preStatus || preStatus.status !== "injected") {
      await browser.execute(() => (window as any).__slterm_e2e_injectHooks?.());
    }

    // 3. 轮询查询注入状态（注入是 spawn_blocking 异步的，需要等文件落盘）
    const status = await browser.waitUntil(
      async () => {
        const s = await browser.execute(() =>
          (window as any).__slterm_e2e_getHookInjectionStatus?.(),
        );
        if (s && (s.status === "injected" || s.status === "outdated")) return s;
        return false;
      },
      { timeout: 15000, timeoutMsg: "hooks 注入未在 15s 内完成" },
    );

    expect(status).toBeDefined();
    expect(status.status).toBe("injected");
    expect(status.version).toBeGreaterThan(0);
  });

  /**
   * 用例 2：Node 端写信号文件 → 页签 tabStatus working → SessionEnd → 清空。
   *
   * 查询方式：面板参数 params.tabStatus（IC-03 改造：tabIcon emoji 字段退役，
   * 状态写入 tabStatus（AgentStatus 字符串）；DefaultTab 渲染 StatusDot 圆点）。
   */
  it("信号文件驱动页签状态圆点流转", async () => {
    const eventsDir = defaultEventsDir();
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-hooks-"));
    const signalFiles: string[] = [];

    try {
      // 0a. 等待 Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 创建测试项目
      await createProject(tempDir);

      // 0d. 等待 Dockview API 就绪
      await waitForDockviewApi();

      // 1. 创建终端面板（唯一 panelId）
      const panelId = "e2e-hooks-term-" + Date.now();
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 4. 写 UserPromptSubmit 信号文件（原子 rename：.tmp → .json）
      const submitPayload = {
        panelId,
        event: "UserPromptSubmit",
        timestamp: Date.now(),
        sessionId: "e2e",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      };
      signalFiles.push(writeSignalFile(eventsDir, submitPayload));

      // 5. 轮询本面板页签参数 tabStatus === "working"（UserPromptSubmit → working）。
      //    经 getPanel(params.tabStatus) 精确断言单面板，免疫其他面板页签状态污染
      //    （如 Agent Status R2 用例无 SessionEnd 信号、页签 ⚡ 滞留的场景）；
      //    DefaultTab 的 emoji DOM 渲染由 L2 workspace-defaulttab 覆盖。
      await waitForPanelTabStatus(panelId, "working", 15000);

      // 6. 写 SessionEnd 信号文件
      const endPayload = {
        panelId,
        event: "SessionEnd",
        timestamp: Date.now(),
        sessionId: "e2e",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      };
      signalFiles.push(writeSignalFile(eventsDir, endPayload));

      // 7. 轮询本面板页签参数 tabStatus 回落 null（SessionEnd → active=false 清状态）
      await waitForPanelTabStatus(panelId, null, 15000);
    } finally {
      // 清理信号文件
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      // 清理临时目录
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /**
   * 用例 3（E2E-06）：真实 hook reporter 链路——真实执行
   * `node ~/.slterminal/hooks/slterm-hook-reporter.js`（stdin 写 JSON +
   * SLTERM_PANEL_ID env），断言：
   * ① 脚本 exit 0（C10 正常路径）；
   * ② 信号文件真实产生（脚本写 hooks-events/）且被 watcher 消费（消失）；
   * ③ 页签状态圆点随事件流转（⚡ 出现）；
   * ④ 非法 JSON 输入 → 脚本仍 exit 0（C10 守卫，D7）且不产生信号文件。
   *
   * 与既有用例（Node 侧直接写 .json 绕过脚本）互补：本用例覆盖
   * stdin 解析、SLTERM_PANEL_ID 路由、信号文件命名/原子写全链路。
   */
  it("真实 hook reporter 链路：node 脚本 stdin 上报 → 信号文件产生/消费 + C10 exit 0", async () => {
    const { panelId, tempDir, cleanup } = await withProjectAndTerminal({ hooks: true });
    const signalFiles: string[] = [];

    try {
      // 1. 注入完成 → 脚本已落盘（agent_hooks_inject 写入的固定路径）
      const scriptPath = join(homedir(), ".slterminal", "hooks", "slterm-hook-reporter.js");
      expect(existsSync(scriptPath)).toBe(true);

      // 2. 真实执行脚本：stdin 写 JSON（hook 事件契约字段）+ SLTERM_PANEL_ID env
      const payload = {
        hook_event_name: "PreToolUse",
        session_id: "e2e-reporter-" + Date.now(),
        transcript_path: "",
        cwd: tempDir,
        tool_name: "Bash",
        notification_type: null,
      };
      const run = spawnSync(process.execPath, [scriptPath], {
        input: JSON.stringify(payload),
        encoding: "utf8",
        env: { ...process.env, SLTERM_PANEL_ID: panelId },
        timeout: 15000,
      });
      // ① C10 正常路径：exit 0
      expect(run.status).toBe(0);

      // 3. 信号文件产生：hooks-events/ 下出现 panelId 匹配的 .json（脚本命名：
      //    <timestamp>_<safePanelId>_<event>_<rnd>.json，先 .tmp 再 rename）
      const eventsDir = defaultEventsDir();
      const safeId = panelId.replace(/[^a-zA-Z0-9_-]/g, "_");
      const signalFile = await browser.waitUntil(
        async () => {
          const files = readdirSync(eventsDir).filter(
            (f) => f.endsWith(".json") && f.includes(`_${safeId}_PreToolUse_`),
          );
          return files.length > 0 ? join(eventsDir, files[0]) : false;
        },
        { timeout: 10000, timeoutMsg: "reporter 脚本未产生 PreToolUse 信号文件" },
      );
      signalFiles.push(signalFile as string);

      // 4. 信号文件被 watcher 消费（消失——notify 实时 + 3s 轮询兜底双路径）
      await waitForSignalConsumed(signalFile as string);

      // 5. 页签状态圆点随事件流转（PreToolUse → ⚡）——经本面板 tabStatus 参数精确断言
      //    （同用例 2 语义：免疫其他面板页签状态污染）
      await waitForPanelTabStatus(panelId, "working", 15000);

      // 6. 非法 JSON 输入 → 脚本仍 exit 0（C10 守卫：任何代码路径恒 0）且不产生新信号文件
      const beforeCount = readdirSync(eventsDir).filter((f) => f.endsWith(".json")).length;
      const bad = spawnSync(process.execPath, [scriptPath], {
        input: "not-valid-json{{{",
        encoding: "utf8",
        env: { ...process.env, SLTERM_PANEL_ID: panelId },
        timeout: 15000,
      });
      expect(bad.status).toBe(0);
      // 信号目录文件数不增加（非法 JSON 路径静默退出，不写文件）
      const afterCount = readdirSync(eventsDir).filter((f) => f.endsWith(".json")).length;
      expect(afterCount).toBe(beforeCount);
    } finally {
      // 清理信号文件（watcher 消费后自删，此处兜底）+ 临时目录
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      cleanup();
    }
  });
});

// ── hooks 配置面板保存链路（P3-TE-18） ──
//
// 场景：tempdir 项目 → 打开设置中心面板（settings 组件，深链 selectedPage="hooks"，
// SC-E2E-02 适配）→ 切 project 层 → JSON 模式经
// __slterm_e2e_setHooksConfigJson 注入合法 hooks 配置 → 点击保存 →
// 断言 <tempdir>/.claude/settings.json 真实写盘。
// 断言三件事：① mtime 更新；② hooks 内容正确（写入的事件/handler 存在，且
// 预置的旧 hooks 被整体替换）；③ merge 保留——预置的 permissions/env/$schema
// 原样保留（验证后端 read-modify-write，P3-BE-03）。
// 安全：全程只写 tempdir 项目的 project 层，不碰真实 ~/.claude/settings.json（C13-9）。
// hub 面板（Stage 06 起）：面板 = 顶部 CLI 选择行 + 编辑器槽，claude 编辑器内容整体
// 下移一层（行为零改动）；本组用例断言随 hub 结构同步——选择行渲染（单 CLI 也有
// 选择行 + claude logo/displayName）、保存链路经 hub、注入按钮三态经 hub、
// data-e2e="hooks-restart-hint" 断言保留。
//
// 按钮交互统一走 browser.execute 程序化 .click()，不用 WebDriver 真实点击——两个根因：
// 1) 面板根容器 onFocus（React focusin）触发轻量重读 reload() → setLoading(true) →
//    面板内容整体换成"加载中"占位（DOM 移除）。真实点击的 mousedown 先聚焦按钮 →
//    focusin → 重读 → 按钮在 mouseup 前被移出 DOM → click 事件丢失 → onClick 永不
//    触发（实测复现：切层点击后编辑区短暂消失又恢复 user 层内容，project 层从未加载）。
//    程序化 .click() 不移动焦点 → 无 focusin → 无此竞态（与编辑器 Ctrl+S 用例
//    合成 focusin + keydown 同属"事件来源合成"的既有先例）。
// 2) embedded 驱动对 focusCommand（findElement/$/elementClick 等）每次先调
//    getWindowStates 超时 5s（已知无害噪声）——全部改用 execute 轮询可免除约 30s
//    固定开销，把用例控制在 mocha 60s 预算内。
//
// 时序设计（规避外部 value 同步竞态）：
// - 面板挂载先读 user 层（初始文档也是 "{}"），无法用 "{}" 判断 project 层已加载——
//   预置文件 hooks 子树含唯一 marker，等待文档出现 marker 才注入，确保外部 value
//   同步 effect 不会在注入后覆盖文本并重置 dirty。
describe("hooks 配置面板保存链路 (P3-TE-18)", () => {
  it("project 层 JSON 模式写入 hooks 配置 → 保存真实写盘且 merge 保留其他字段", async () => {
    // 0. Node 侧：tempdir 项目 + 预置 .claude/settings.json
    //    （hooks 子树含 preseed marker 供"project 层已加载"等待；permissions/env/$schema 供 merge 断言）
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-hookscfg-"));
    const settingsDir = join(tempDir, ".claude");
    const settingsPath = join(settingsDir, "settings.json");
    mkdirSync(settingsDir, { recursive: true });
    const preseed = {
      $schema: "https://json.schemastore.org/claude-code-settings.json",
      permissions: { allow: ["Bash", "Edit"] },
      env: { FOO: "bar" },
      hooks: {
        PostToolUse: [{ hooks: [{ type: "command", command: "echo preseed-marker" }] }],
      },
    };
    writeFileSync(settingsPath, JSON.stringify(preseed, null, 2), "utf8");
    const mtimeBefore = statSync(settingsPath).mtimeMs;

    // 注入到 JSON 模式的 hooks 配置（hooks 子 schema 合法：已知事件 + command handler；
    // 与 preseed 无重叠内容，保存后断言旧 hooks 被整体替换）
    const hooksJson = JSON.stringify(
      {
        PreToolUse: [
          {
            matcher: "Bash",
            hooks: [{ type: "command", command: "node e2e-hook-precheck.js", timeout: 5 }],
          },
        ],
        SessionStart: [{ hooks: [{ type: "command", command: "echo e2e-session-start" }] }],
      },
      null,
      2,
    );

    // 页面内工具：按 data-e2e 选择器取按钮（存在且未禁用时）——execute 轮询 + 程序化点击共用
    const btnState = (sel: string) =>
      browser.execute((s: string) => {
        const btn = document.querySelector(s) as HTMLButtonElement | null;
        return btn ? { exists: true, disabled: btn.disabled } : { exists: false, disabled: true };
      }, sel);

    try {
      // 1. 等待 Workspace 就绪
      await waitForWorkspaceReady();

      // 2. 程序化创建项目（根 = tempdir；同时设置后端 project_root，路径沙箱通过）
      await createProject(tempDir);

      // 3. 等待 Dockview API
      await waitForDockviewApi();

      // 3b. 关闭前序用例遗留的设置面板（本用例保存后面板未关；mocha retries:1
      //     重跑时旧面板残留 → addPanel 叠加出多个面板 → hub 选择行 logo 全页计数
      //     断言（logoCount/rowButtonCount）命中间态面板——先关后开保证唯一，
      //     照 :454 it 的既有先例）
      await browser.execute(() => {
        for (const p of window.__dockviewApi!.panels) {
          if (p.component === "settings") p.api.close();
        }
      });

      // 4. 打开设置中心面板（F11 形态，SC-E2E-02：组件 "settings" + 深链
      //    selectedPage="hooks"——hooks 页迁入设置中心后的打开方式；面板 id 用
      //    settings-e2e- 前缀——壳按 settings- 解析 pageId 查不到项目 → 不触发
      //    切项目自动关闭，靠 finally 手动回收；唯一 id 不与同页单例约定冲突）
      const panelId = "settings-e2e-hookscfg-" + Date.now();
      await browser.execute((pid: string) => {
        window.__dockviewApi!.addPanel({
          id: pid,
          component: "settings",
          title: "设置",
          params: { panelId: pid, selectedPage: "hooks" },
        });
      }, panelId);
      // 面板容器仅在非 loading/error 态渲染——存在即表示首次加载（user 层）完成
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-config-panel"]'))) === true,
        { timeout: 15000, timeoutMsg: "hooks 配置页未就绪" },
      );

      // 4b. hub 选择行断言（D-14 Stage 06 段）：选择行位于编辑器上方（编辑器下移一层后的
      //     hub 结构）；单 CLI 也渲染选择行（边界 1，防布局跳动）——claude logo
      //     （iconSrc 16×16）+ displayName 文本。选择行无 data-e2e 契约，按 claude
      //     品牌 logo img 定位；E2E 构建仅 claude 注册（mockcli 属 Stage 07 经 E2E
      //     helper 注册）。img 查询限定面板容器作用域（3b 开前关闭已保证面板唯一）：
      //     终端页签 logo 残留（history spec 恢复编排——双击历史行 → 创建终端 +
      //     pty.write 注入 claude --resume 命令 → useCommandDetection 命中 → 终端
      //     页签渲染 claude 16×16 logo；页面多 Dockview 实例 + CSS 显隐，隐藏页面板
      //     不卸载仍留 DOM）不在面板容器内，全页计数会被污染（Stage 03 fix-loop 仅
      //     覆盖设置面板残留来源，未覆盖终端页签 logo 来源）。
      const hubRow = await browser.execute(() => {
        const panel = document.querySelector('[data-e2e="hooks-config-panel"]');
        const imgs = panel
          ? Array.from(panel.querySelectorAll<HTMLImageElement>('img[src="/cli-icons/claude.png"]'))
          : [];
        const buttons = imgs
          .map((img) => img.closest("button"))
          .filter((b): b is HTMLButtonElement => b !== null);
        // "编辑器"参照取选择行所属容器内的编辑器槽（容器末子元素）——data-e2e 挂在
        // 容器上，容器 = 选择行 + 编辑器槽两层；直接用容器比位会命中祖先分支
        // （compareDocumentPosition 对祖先不置 FOLLOWING 位，实测恒 false）
        const row = imgs[0]?.closest('[data-e2e="hooks-config-panel"]') ?? null;
        const editor = row?.lastElementChild ?? null;
        return {
          logoCount: imgs.length,
          buttonTexts: buttons.map((b) => b.textContent ?? ""),
          rowButtonCount: buttons.length,
          rowAboveEditor:
            imgs.length > 0 && editor !== null
              ? (imgs[0].compareDocumentPosition(editor) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
              : false,
        };
      });
      expect(hubRow.logoCount).toBeGreaterThan(0); // 选择行渲染 claude logo
      expect(hubRow.buttonTexts[0]).toContain("claude"); // 按钮 displayName
      expect(hubRow.rowButtonCount).toBe(1); // 单 CLI 也渲染选择行（恰一枚按钮）
      expect(hubRow.rowAboveEditor).toBe(true); // 选择行在编辑器上方（hub 结构）

      // 5. 切到 project 层：rootPath 就绪后按钮才可点（disabled=!rootPath）——execute 轮询
      //    等待启用，再程序化 .click()（真实 onClick → setLayer → 重读 project 层；
      //    程序化点击不触发 focusin，规避上面注释 1) 的 click 丢失竞态）
      await browser.waitUntil(
        async () => (await btnState('[data-e2e="hooks-layer-project"]')).disabled === false,
        { timeout: 10000, timeoutMsg: "project 层按钮未启用（rootPath 未就绪）" },
      );
      const layerClicked = await browser.execute(() => {
        const btn = document.querySelector('[data-e2e="hooks-layer-project"]') as HTMLButtonElement | null;
        btn?.click();
        return btn !== null;
      });
      expect(layerClicked).toBe(true);

      // 6. 等待 project 层配置加载进 JSON 模式（文档含 preseed marker——只有 project 层
      //    读取应用后才可能出现，排除 user 层初始内容干扰）
      await browser.waitUntil(
        async () => {
          const doc = await browser.execute(() =>
            (window as any).__slterm_e2e_getHooksConfigJson?.() ?? null,
          );
          return doc !== null && doc.includes("preseed-marker");
        },
        { timeout: 10000, timeoutMsg: "project 层配置未加载进 JSON 模式（文档未出现 preseed marker）" },
      );

      // 7. JSON 模式注入合法 hooks 配置（CM6 view.dispatch 全文档替换 → 真实
      //    updateListener → onChange → dirty + schema 校验通过）
      const injected = await browser.execute((text: string) => {
        return (window as any).__slterm_e2e_setHooksConfigJson?.(text) === true;
      }, hooksJson);
      expect(injected).toBe(true);

      // 8. 等待保存按钮可用（dirty && JSON 合法）
      await browser.waitUntil(
        async () => (await btnState('[data-e2e="hooks-save"]')).disabled === false,
        { timeout: 10000, timeoutMsg: "保存按钮未启用（dirty 或 JSON 非法）" },
      );

      // 9. 程序化点击保存（真实 onClick → handleSave → schema 校验 → writeHooksConfig 写盘；
      //    程序化点击不触发 focusin → 不弹 dirty 确认框、无重读覆盖注入内容的风险）
      const saveClicked = await browser.execute(() => {
        const btn = document.querySelector('[data-e2e="hooks-save"]') as HTMLButtonElement | null;
        btn?.click();
        return btn !== null;
      });
      expect(saveClicked).toBe(true);

      // 10. 轮询等待文件 mtime 更新（Node 侧 statSync，不依赖页面交互）
      await browser.waitUntil(
        () => {
          try {
            return statSync(settingsPath).mtimeMs > mtimeBefore;
          } catch {
            return false;
          }
        },
        { timeout: 10000, timeoutMsg: "保存后 <tempdir>/.claude/settings.json mtime 未更新" },
      );
      // 11. 应用内确认：保存成功提示条（saved=true 后渲染）
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-restart-hint"]'))) === true,
        { timeout: 8000, timeoutMsg: "保存成功提示条未出现" },
      );

      // 12. 断言①：mtime 更新
      expect(statSync(settingsPath).mtimeMs).toBeGreaterThan(mtimeBefore);

      // 13. 断言②：hooks 内容正确——写入的事件/handler 存在，预置的旧 hooks 被整体替换
      const saved = JSON.parse(readFileSync(settingsPath, "utf8"));
      expect(saved.hooks.PreToolUse).toHaveLength(1);
      expect(saved.hooks.PreToolUse[0].matcher).toBe("Bash");
      expect(saved.hooks.PreToolUse[0].hooks[0]).toMatchObject({
        type: "command",
        command: "node e2e-hook-precheck.js",
        timeout: 5,
      });
      expect(saved.hooks.SessionStart[0].hooks[0].command).toBe("echo e2e-session-start");
      // 替换语义：预置的 PostToolUse 组不应残留
      expect(saved.hooks.PostToolUse).toBeUndefined();
      expect(JSON.stringify(saved)).not.toContain("preseed-marker");

      // 14. 断言③：merge 保留——permissions/env/$schema 原样保留（后端 read-modify-write）
      expect(saved.permissions).toEqual(preseed.permissions);
      expect(saved.env).toEqual(preseed.env);
      expect(saved.$schema).toBe(preseed.$schema);
    } finally {
      // 回收本用例打开的设置面板（保存用例面板未关；mocha retries:1 重跑时
      // 旧面板残留在上一页 dockview——页面用多 Dockview 实例 + CSS 显隐，隐藏页面板
      // 不卸载仍留 DOM，全页 logo 计数断言（logoCount/rowButtonCount）会命中旧面板。
      // 此时当前页仍活跃，__dockviewApi 正指向本页 dockview，关闭可达；照 :454 it
      // 「先关后开保证唯一」先例，用例结束时回收，重跑/后续用例从零开始）
      await browser.execute(() => {
        for (const p of window.__dockviewApi!.panels) {
          if (p.component === "settings") p.api.close();
        }
      });
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  /**
   * 用例：hub 注入按钮三态（D-14 Stage 06 段）。
   * 注入状态条（data-e2e="hooks-injection-status"）三态文案（已注入/未注入/版本过旧）
   * 经 hub 面板真实按钮链路流转：「注入 Hooks」/「卸载 Hooks」onClick →
   * agent_hooks_inject/uninstall（cliId 实参 = hub 选中态，E2E 构建仅 claude 一个
   * hasConfigEditor CLI，选中态即 claude）。链路末尾恢复「已注入」，不污染后续用例；
   * run-wdio.cjs 对 ~/.claude/settings.json 亦有备份还原兜底（E2E-05）。
   */
  it("hub 注入按钮三态：状态条随注入/卸载链路流转并恢复", async () => {
    // 0. 等待 Workspace 就绪 + Dockview API
    await waitForWorkspaceReady();
    await waitForDockviewApi();

    // 1. 关闭前序用例遗留的设置面板（保存用例面板未关；document.querySelector
    //    取首匹配元素，多面板并存会让状态条/按钮断言命中间态面板——先关后开保证唯一）
    await browser.execute(() => {
      for (const p of window.__dockviewApi!.panels) {
        if (p.component === "settings") p.api.close();
      }
    });

    // 2. 程序化打开设置中心面板（设置中心形态，SC-E2E-02：settings 组件 + 深链
    //    selectedPage="hooks"；hub 容器 = 选择行 + claude 编辑器槽）
    const panelId = "settings-e2e-inject-" + Date.now();
    await browser.execute((pid: string) => {
      window.__dockviewApi!.addPanel({
        id: pid,
        component: "settings",
        title: "设置",
        params: { panelId: pid, selectedPage: "hooks" },
      });
    }, panelId);
    // 注入状态条在编辑器工具栏——仅非 loading/error 态渲染（编辑器下移一层后同语义）
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => !!document.querySelector('[data-e2e="hooks-injection-status"]'))) === true,
      { timeout: 15000, timeoutMsg: "hooks 配置页未就绪（注入状态条未出现）" },
    );

    // 3. 状态条文案读取 + 三态其一等待（初始异步查询未完成时为 "注入状态：--"，非三态）
    const readBarText = () =>
      browser.execute(
        () => document.querySelector('[data-e2e="hooks-injection-status"]')?.textContent ?? "",
      );
    const waitBarText = (expected: string) =>
      browser.waitUntil(
        async () => (await readBarText()) === expected,
        { timeout: 15000, timeoutMsg: `注入状态条未变为「${expected}」` },
      );
    await browser.waitUntil(
      async () => {
        const t = await readBarText();
        return t === "注入状态：已注入" || t === "注入状态：未注入" || t === "注入状态：版本过旧";
      },
      { timeout: 10000, timeoutMsg: "注入状态条未进入三态之一" },
    );

    // 4. 按钮就绪后程序化点击（同保存用例注释：程序化 .click() 不触发 focusin，规避
    //    面板根容器 focus 重读竞态；busy 期间按钮禁用，未就绪/禁用则重试等待）
    const clickWhenReady = (sel: string) =>
      browser.waitUntil(
        async () => {
          const ok = await browser.execute((s: string) => {
            const btn = document.querySelector(s) as HTMLButtonElement | null;
            if (btn && !btn.disabled) {
              btn.click();
              return true;
            }
            return false;
          }, sel);
          return ok;
        },
        { timeout: 10000, timeoutMsg: `按钮 ${sel} 未就绪` },
      );

    // 5. 三态链路：非已注入则经面板注入 → 已注入；卸载 → 未注入；再注入 → 已注入（恢复）
    if ((await readBarText()) !== "注入状态：已注入") {
      await clickWhenReady('[data-e2e="hooks-inject"]');
    }
    await waitBarText("注入状态：已注入");

    await clickWhenReady('[data-e2e="hooks-uninstall"]');
    await waitBarText("注入状态：未注入");

    await clickWhenReady('[data-e2e="hooks-inject"]');
    await waitBarText("注入状态：已注入");
  });
});
