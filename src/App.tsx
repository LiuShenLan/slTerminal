import { useEffect, useState } from "react";
import { registerCloseHandler, onFocusChanged, requestUserAttention } from "./ipc/window";
import { Workspace } from "./workspace";
import {
  loadAllProjects, markLoadSucceeded, markPersistenceReady, saveAllProjects, cancelPendingSave,
  useProjects,
} from "./stores/projects";
import { useLayout } from "./stores/layout";
import { useFontSize, cancelPendingSave as cancelFontSizeSave } from "./stores/fontSize";
import { useKeybindings, cancelPendingSave as cancelKeybindingsSave } from "./stores/keybindings";
import { useSideBar, cancelPendingSave as cancelSideBarSave } from "./stores/sideBar";
import { saveLayout } from "./workspace/layoutSerde";
import { pty } from "./ipc";
import * as agentHooks from "./ipc/agentHooks";
import { CLAUDE_CLI_ID } from "./features/cliProfiles/profiles/claude";
import { setProjectRoot } from "./ipc/fs";
import { TerminalRegistry } from "./panels/terminal/TerminalRegistry";
import { ErrorBoundary, ConfirmDialogHost, ToastHost, toast } from "./lib";
import { getShortcutRegistry, createGlobalShortcuts, wireKeybindings } from "./features/shortcuts";
import { createTerminalShortcuts } from "./panels/terminal/keyboard";
import { createEditorShortcuts } from "./panels/editor/keyboard";
import { createExplorerShortcuts } from "./features/explorer/keyboard";
import { NotificationListener } from "./features/notifications";
import { TitleBar } from "./features/titleBar/TitleBar";
import { PANEL_BG, SECONDARY_BG, SEPARATOR_BG, DIM_FG, APP_BG } from "./theme";
import "dockview-react/dist/styles/dockview.css";
// App.css 从 main.tsx 移此（BOOT-02）：dockview.css 先、App.css 后，CSS 变量覆盖语义正确
import "./App.css";

/** 关闭等待超时（ms）——kill PTY / flush 持久化的最大等待时间 */
const SHUTDOWN_TIMEOUT_MS = 3000;
/** localStorage key：上次活跃页面 ID */
const LS_LAST_ACTIVE_PAGE_KEY = "slterm-last-active-page";

function App() {
  /** S4-D1: 数据就绪后才渲染 Workspace，消除启动竞态 */
  const [ready, setReady] = useState(false);
  /** FE-02: 项目数据加载失败信息（非 null = 加载失败，渲染错误页阻断启动） */
  const [projectsLoadError, setProjectsLoadError] = useState<string | null>(null);

  // FE-02: 项目数据加载 + 启动恢复（从 init 抽取）——
  // 成功路径 markPersistenceReady() + lastPage 恢复原样保留（DBG-6 顺序不动：
  // 先 await setProjectRoot 再 setActivePage）；失败阻断 ready 与写门控（不
  // markPersistenceReady、不 setReady），经 projectsLoadError 渲染错误页
  const loadProjectsAndRestore = async () => {
    try {
      // P2-07: loadAllProjects 内部 JSON.parse 当前数据量小无影响，
      // 若未来项目数据文件膨胀到 MB 级，可改为流式解析或 IndexedDB 存储。
      // E2E 构建（VITE_E2E=1）跳过项目数据恢复：前序 spec 的项目经
      // slterminal-projects.json 恢复进 store（一轮可累积 20+ 页），FE-36
      // 全局页数上限会拒绝后续 addPage（H6/E2E-04 回归根因）；且恢复与
      // __slterm_e2e_createProject 的清空存在竞态（IPC 慢时恢复覆盖）。
      // 内联表达式门控（引用 E2E_ENABLED 常量会使 helpers chunk 残留
      // 生产 dist，CI 守卫 fail——见 main.tsx:77-81）。
      if (import.meta.env.VITE_E2E !== "1") {
        await loadAllProjects();
      } else {
        // FE-02：E2E 构建跳过磁盘恢复——显式放行写盘（防空写守卫误拒 E2E 保存）
        markLoadSucceeded();
      }
    } catch (err) {
      // FE-02：加载失败阻断启动——不 markPersistenceReady、不 setReady（防空写覆盖磁盘数据）
      console.error("[App] 加载项目数据失败:", err);
      setProjectsLoadError(err instanceof Error ? err.message : String(err));
      return;
    }
    markPersistenceReady();

    // 数据就绪后恢复上次 activePageId（确保 pageId 对应的项目数据已加载）
    // E2E 模式跳过：__slterm_e2e_createProject 已设 window.__slterm_e2e_projectPending=true
    try {
      if (!window.__slterm_e2e_projectPending) {
        const lastPage = localStorage.getItem(LS_LAST_ACTIVE_PAGE_KEY);
        if (lastPage) {
          // DBG-6: setActivePage 前先同步项目根路径到后端（路径沙箱前置条件）
          const { projects: currentProjects } = useProjects.getState();
          for (const [, proj] of Object.entries(currentProjects)) {
            if (proj.pages.some((p) => p.pageId === lastPage)) {
              if (proj.rootPath) {
                try {
                  await setProjectRoot(proj.rootPath);
                } catch (err) {
                  console.error("[slTerminal] 启动恢复—设置项目根路径失败:", err);
                  // FE-04（D7）：失败仍继续恢复流程，toast 告警可感知
                  toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
                }
              }
              break;
            }
          }
          useLayout.getState().setActivePage(lastPage);
        }
      }
    } catch {
      // localStorage 不可用时静默失败
    }

    setReady(true);
  };

  // FE-02: 重试项目数据加载——清错误态后重跑启动链，成功经 loadProjectsAndRestore 置 ready
  const retryProjectsLoad = () => {
    setProjectsLoadError(null);
    void loadProjectsAndRestore();
  };

  // FE-02: 以空项目状态继续——用户显式选择空状态，放行写盘（防空写守卫不拦截后续保存）
  const continueWithEmptyProjects = () => {
    markLoadSucceeded();
    markPersistenceReady();
    setReady(true);
  };

  // S4-D1: 启动加载（单一时序：loadProjectsAndRestore → setReady → 渲染 Workspace；
  // 项目数据加载失败时经 projectsLoadError 渲染错误页阻断启动，见 FE-02）
  useEffect(() => {
    const init = async () => {
      // FE-20：字体/快捷键/侧栏三 store loadFromDisk 改 Promise.all 并行加载——
      // 各自独立 try/catch 保留（单个失败不阻塞其余）；loadAllProjects 保持在其后
      // （markPersistenceReady 时序不动，见 S12 契约）
      await Promise.all([
        (async () => {
          try {
            // 加载字体大小偏好（先于项目数据，确保面板渲染时已有正确值）
            await useFontSize.getState().loadFromDisk();
          } catch (err) {
            // FE-03：启动链失败不再静默——降级兜底不变（保持默认值），仅告警记录
            console.warn("[App] 加载字体大小设置失败，保持默认值:", err);
          }
        })(),
        (async () => {
          try {
            // 加载快捷键自定义绑定（覆盖层）——先于面板注册，确保注册表构建时已有覆盖
            await useKeybindings.getState().loadFromDisk();
          } catch (err) {
            // FE-03：启动链失败不再静默——降级兜底不变（保持默认空覆盖），仅告警记录
            console.warn("[App] 加载快捷键设置失败，保持默认绑定:", err);
          }
        })(),
        (async () => {
          try {
            // 加载侧栏视图状态——区划/开关/宽度/比例，先于项目数据确保 Workspace 渲染时已有正确值
            await useSideBar.getState().loadFromDisk();
          } catch (err) {
            // FE-03：启动链失败不再静默——降级兜底不变（保持默认值），仅告警记录
            console.warn("[App] 加载侧栏设置失败，保持默认值:", err);
          }
        })(),
      ]);

      await loadProjectsAndRestore();
    };
    init();
  }, []);

  // S4-D4: 关闭前冲刷持久化（registerCloseHandler 封装 preventDefault + destroy）
  useEffect(() => {
    const unlisten = registerCloseHandler(async () => {
      try {
        // 0. 杀掉所有活跃 PTY session（P1-19：窗口关闭前清理子进程）
        //    通过 TerminalRegistry 遍历所有已注册终端，逐个 kill
        const allSessions = TerminalRegistry.getAll();
        if (allSessions.size > 0) {
          const killPromises: Promise<void>[] = [];
          // FE-05：失败先收集（含 session/panel 归属），全部结束后统一汇总一条日志
          const killFailures: Array<{ sessionId: string; panelId: string; error: unknown }> = [];
          allSessions.forEach((entry, panelId) => {
            if (entry.sessionId) {
              killPromises.push(
                pty.kill(entry.sessionId, panelId).catch((err) => {
                  killFailures.push({ sessionId: entry.sessionId, panelId, error: err });
                }),
              );
            }
          });
          // 并发 kill 所有 session，最长等待 3 秒
          await Promise.race([
            Promise.all(killPromises),
            new Promise<void>((resolve) => setTimeout(resolve, SHUTDOWN_TIMEOUT_MS)),
          ]);
          if (killFailures.length > 0) {
            // FE-05：统一一条汇总日志（含失败数），替代逐条 console.error
            console.error(
              `[slTerminal] 关闭时 ${killFailures.length} 个 PTY session kill 失败:`,
              killFailures,
            );
          }
        }

        // BE-08：Registry kill 后 ptyKillAll 兜底——前后端 session 映射不一致
        // （前端漏记/后端残留）时后端 session 不泄漏；返回 kill 数仅作日志，
        // 失败不阻断关闭（Job Object KILL_ON_JOB_CLOSE 仍有最终兜底）
        try {
          // FE-47：ptyKillAll 包总超时——后端逐 session 3s 串行 kill+join，
          // 极端多 session 场景防拖长关窗（与上方 Registry kill 同形 race）
          const killed = await Promise.race([
            pty.ptyKillAll(),
            new Promise<null>((resolve) =>
              setTimeout(() => resolve(null), SHUTDOWN_TIMEOUT_MS),
            ),
          ]);
          if (killed !== null && killed > 0) {
            console.info(`[slTerminal] 关闭兜底清理 ${killed} 个后端残留 PTY session`);
          }
        } catch (err) {
          console.error("[slTerminal] 关闭兜底 pty_kill_all 失败:", err);
        }

        // 1. flush dirty layout
        const { activePageId } = useLayout.getState();
        if (activePageId && window.__dockviewApi) {
          const layout = saveLayout(window.__dockviewApi);
          const { projects } = useProjects.getState();
          for (const [, proj] of Object.entries(projects)) {
            if (proj.pages.some((p) => p.pageId === activePageId)) {
              useProjects.getState().updatePageLayout(
                proj.projectId,
                activePageId,
                layout as Record<string, unknown>,
              );
              break;
            }
          }
        }
        // 2. 同步保存到磁盘（清除各 store debounce 定时器 + 3s 超时防挂起）
        cancelPendingSave();
        cancelFontSizeSave();
        cancelKeybindingsSave();
        cancelSideBarSave();
        await Promise.race([
          saveAllProjects(),
          new Promise<void>((resolve) => setTimeout(resolve, SHUTDOWN_TIMEOUT_MS)),
        ]);
        // 3. 保存 activePageId
        if (activePageId) {
          try {
            localStorage.setItem(LS_LAST_ACTIVE_PAGE_KEY, activePageId);
          } catch { /* localStorage 不可用时静默失败 */ }
        }
        // 4. 恢复 statusline 桥接（还原备份原配置，备份保留供重开重注入）——
        //    用户在别处用 cli 时终端状态行不受桥接影响；失败静默不阻断关闭
        try {
          await agentHooks.restoreStatusline(CLAUDE_CLI_ID);
        } catch (err) {
          console.error("[slTerminal] 关闭时恢复 statusline 失败:", err);
        }
      } catch (err) {
        console.error("[slTerminal] 关闭保存失败:", err);
      }
    });
    return unlisten;
  }, []);

  // 注册全局 + 面板级快捷键（一次性；面板命令 handler 经 active 指针派发到聚焦实例）
  useEffect(() => {
    const registry = getShortcutRegistry();
    return registry.register([
      ...createGlobalShortcuts(() => window.__dockviewApi),
      ...createTerminalShortcuts(),
      ...createEditorShortcuts(),
      ...createExplorerShortcuts(),
    ]);
  }, []);

  // 将用户自定义绑定（覆盖层）持续同步到注册表
  useEffect(() => {
    return wireKeybindings(getShortcutRegistry(), useKeybindings);
  }, []);

  // P2-FE-04：窗口焦点监听——供通知调度门控使用
  useEffect(() => {
    // 初始状态：窗口刚启动时认为有焦点
    window.__slterm_windowFocused = true;
    return onFocusChanged((focused) => {
      window.__slterm_windowFocused = focused;
      // P2-FE-05：窗口恢复焦点时停止任务栏闪烁
      if (focused) {
        requestUserAttention(null).catch((err) => {
          // FE-06：停止闪烁失败为非关键路径——仅告警记录，不 toast
          console.warn("[App] 停止任务栏闪烁失败:", err);
        });
      }
    });
  }, []);

  if (!ready) {
    return (
      <ErrorBoundary>
        {projectsLoadError === null ? (
          <div
            style={{
              width: "100vw",
              height: "100vh",
              background: PANEL_BG,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: DIM_FG, // FE-10：说明文字 fg-3 档（UI-104，原误用输入框边框色）
              fontSize: 13, // UI-204：正文 13px
              fontFamily: '"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace', // UI-201：全局字体栈
            }}
          >
            slTerminal 启动中…
          </div>
        ) : (
          // FE-02：项目数据加载失败错误页——重试 / 以空状态继续
          // （样式经 theme token：PANEL_BG 底 + SECONDARY_BG 按钮底 + SEPARATOR_BG 边框 + DIM_FG 13px，禁止硬编码颜色）
          <div
            data-e2e="projects-load-error"
            style={{
              width: "100vw",
              height: "100vh",
              background: PANEL_BG,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 12,
              color: DIM_FG, // FE-10：说明文字 fg-3 档（UI-104，原误用输入框边框色）
              fontSize: 13, // UI-204：正文 13px
              fontFamily: '"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace', // UI-201：全局字体栈
            }}
          >
            <div>项目数据加载失败</div>
            <div>{projectsLoadError}</div>
            <div>可选择重试，或以空项目状态继续（磁盘上的项目数据不会被覆盖）</div>
            <div style={{ display: "flex", gap: 8 }}>
              <button
                data-e2e="projects-load-retry"
                onClick={retryProjectsLoad}
                style={{
                  background: SECONDARY_BG, // 按钮底
                  border: `1px solid ${SEPARATOR_BG}`, // 1px 边框
                  color: DIM_FG,
                  fontSize: 13,
                  fontFamily: "inherit",
                  padding: "6px 16px",
                  cursor: "pointer",
                }}
              >
                重试
              </button>
              <button
                data-e2e="projects-load-continue-empty"
                onClick={continueWithEmptyProjects}
                style={{
                  background: SECONDARY_BG, // 按钮底
                  border: `1px solid ${SEPARATOR_BG}`, // 1px 边框
                  color: DIM_FG,
                  fontSize: 13,
                  fontFamily: "inherit",
                  padding: "6px 16px",
                  cursor: "pointer",
                }}
              >
                以空状态继续
              </button>
            </div>
          </div>
        )}
      </ErrorBoundary>
    );
  }

  return (
    <ErrorBoundary>
      {/* TB-05: ready 后骨架改列向 flex——首行自绘标题栏，其余为 Workspace（原三栏结构包一层） */}
      <div
        style={{
          width: "100vw",
          height: "100vh",
          background: APP_BG,
          display: "flex",
          flexDirection: "column",
        }}
      >
        {/* FE-28：五个顶层组件分别包 inline ErrorBoundary——单个渲染错误降级占位，不拖垮全局 */}
        <ErrorBoundary variant="inline">
          <TitleBar />
        </ErrorBoundary>
        <div style={{ flex: 1, minHeight: 0 }}>
          <ErrorBoundary variant="inline">
            <Workspace />
          </ErrorBoundary>
        </div>
        <ErrorBoundary variant="inline">
          <NotificationListener />
        </ErrorBoundary>
      </div>
      {/* OV-01: 全局浮层挂载点（fixed 定位，不参与布局）——ConfirmDialog/toast */}
      <ErrorBoundary variant="inline">
        <ConfirmDialogHost />
      </ErrorBoundary>
      <ErrorBoundary variant="inline">
        <ToastHost />
      </ErrorBoundary>
    </ErrorBoundary>
  );
}

export default App;
