// useXterm — PTY 通信 + 终端渲染协调 hook（编排层）
//
// 职责：
// - 编排各子 hook：Terminal 实例、PTY 输出、Resize、剪贴板、命令检测、字体
// - PTY spawn / write / kill（终端进程管理）
// - 快捷键委托 + OSC 8 超链接
// - visible 管理（非焦点终端降频）
// - E2E 测试钩子
//
// 子 hook 分工：
// - useTerminalInstance.ts — Terminal 实例 + addon 生命周期
// - usePtyOutput.ts       — 输出合帧（Idle+Max 双定时器 + DEC 2026）
// - usePtyResize.ts       — ResizeObserver X/Y 分离 debounce
// - useClipboardHandler.ts — OSC 52 剪贴板拦截
// - useCommandDetection.ts — OSC 133 命令边界检测 + 页签标题
// - useFontSizeWheel.ts (src/lib/) — Ctrl+Wheel 共享 hook
// - webgl.ts              — WebGL 检测 + WebglAddon 加载

import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { terminalOptions } from "./theme";
import { useTerminalInstance } from "./useTerminalInstance";
import { usePtyOutput } from "./usePtyOutput";
import { usePtyResize } from "./usePtyResize";
import { useClipboardHandler } from "./useClipboardHandler";
import { useCommandDetection } from "./useCommandDetection";
export { detectWebgl, resetWebglCache } from "./webgl";
import { pty } from "../../ipc";
import { openUrl } from "../../ipc/shell";
import { setActiveTerminal, clearActiveTerminal, type TerminalActions } from "./activeTerminal";
import { usePanelFocus, getShortcutRegistry } from "../../features/shortcuts";
import { TerminalRegistry } from "./TerminalRegistry";
// ZQ-2: 来源 CLI 标识三级解析单点（契约 4）——空串/空白 cliId 同等回退
import { resolvePayloadCliId } from "./resolvePayloadCliId";
import { useFontSizeWheel } from "../../lib/useFontSizeWheel";
import { E2E_ENABLED } from "../../lib/e2eEnabled";
// FE-08: 错误消息统一经 getErrorMessage + toast（契约：src/ipc/appError.ts，src/lib re-export）
import { toast, getErrorMessage } from "../../lib";
import { FONT_SIZE_MIN, FONT_SIZE_MAX } from "../../stores/fontSize";
// Agent 事件订阅（MC-202：onAgentEvent，照 onFsEvent 模式直接引 ipc 文件）
import { onAgentEvent } from "../../ipc/agentHooks";
// 运行中会话标题通道（人工验证问题 3）：与历史 session 同源回退链
import { readHistoryTitle } from "../../ipc/agentHistory";
// MC-205: agent 事件按 cliId 解析 profile——eventToStatus 等 hooks 能力实现迁入 profile
// （lib 层不再含 claude 事件名映射）；缺省回退常量经 profiles/claude 导出（AC-5 兼容）
import { cliProfileRegistry } from "../../features/cliProfiles";
// AC-5: 事件名字面量只允许出现在 profiles/claude/（claude 合法领地）——
// SessionEnd/Exit 判定一律引用本常量，不写字面量
import { SESSION_END_EVENT, EXIT_EVENT, SESSION_START_EVENT } from "../../features/cliProfiles/profiles/claude";
import type { TabState } from "./useCommandDetection";
import {
  installTerminalWriteToPty,
  setTerminalSessionReady,
  setTerminalSessionError,
} from "../../../e2e-tests/helpers";

// xterm.js ConPTY 兼容阈值（@xterm/xterm src/common/CoreTerminal.ts:283）：
// buildNumber < 21376 时启用 wrapping 启发式（每次 LF + CSI H 强制重算 isWrapped），
// 在 claude 高频全屏重绘下误判行 wrap 状态致 buffer 错乱（Win10 四症状，ADR-0004）。
// 钳制真实 build 号至下界，使 Win10 走「新 ConPTY」分支，行为与 Win11 对齐。
export const XTERM_CONPTY_MIN_BUILD = 21376;

/** 钳制真实 Windows build 号至 xterm「新 ConPTY」分支下界（ADR-0004） */
export function clampWindowsBuildForXterm(build: number): number {
  return Math.max(build, XTERM_CONPTY_MIN_BUILD);
}

export interface UseXtermOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 终端列数（初始值，运行时由 fit 重算） */
  cols: number;
  /** 终端行数（初始值，运行时由 fit 重算） */
  rows: number;
  /** 面板 ID，用于关联 PTY 会话 */
  panelId: string;
  /** Windows 真实 build 号（动态检测），用于 ConPTY reflow 阈值 */
  windowsBuildNumber?: number;
  /** 终端工作目录（来自操作页面 cwd） */
  cwd?: string;
  /** 面板是否可见（页面切换时 CSS display:none → 非焦点降频门控；FE-34 不再按可见性释放 WebGL） */
  visible?: boolean;
  /** 终端字体大小（运行时动态调节，默认 14） */
  fontSize?: number;
  /** 字体大小变更回调（Ctrl+Wheel 触发）——已由 useFontSizeWheel 内部处理，保留仅接口兼容 */
  onFontSizeChange?: (size: number) => void;
  /** 命令运行状态变更回调（OSC 133 检测到注册命令启动/退出时触发） */
  onTabStateChange?: (state: TabState) => void;
}

/** useXterm hook 返回类型 */
export interface UseXtermReturn {
  /** 聚焦终端输入 */
  focus: () => void;
  /** 测试专用接口（生产代码忽略） */
  _test: {
    cancelPendingFlush: () => void;
    flushBuffer: () => void;
    getPendingBuffer: () => Uint8Array[];
  };
}

/** fallback 终端尺寸 */
const DEFAULT_COLS = 80;
const DEFAULT_ROWS = 24;

/** write 连续失败次数阈值（FE-08）：≥3 次触发 toast 提醒（单次失败多为瞬态，静默记日志） */
const WRITE_FAIL_TOAST_THRESHOLD = 3;

/** 运行中会话标题重查节流（人工验证问题 3）：SessionStart 立即查一次，
 *  其后 agent-event 距上次查询 ≥5s 才重查（/rename custom-title、ai-title 运行中变化） */
const TITLE_FETCH_THROTTLE_MS = 5000;

/** 检查终端是否可以安全执行 fit 操作（五条件守卫） */
export function canFit(
    terminal: Terminal | null | undefined,
    fitAddon: FitAddon | null | undefined,
    containerEl: HTMLElement | null | undefined,
    isDisposedRef: { current: boolean } | null | undefined
): boolean {
    if (!isDisposedRef) return false;
    if (!terminal || !fitAddon) return false;
    if (!containerEl) return false;
    if (containerEl.offsetWidth === 0 ||
        containerEl.offsetHeight === 0) return false;
    if (!terminal.element) return false;
    if (isDisposedRef.current) return false;
    return true;
}

export function useXterm({
  container, panelId,
  windowsBuildNumber, cwd, visible, fontSize,
  onFontSizeChange, onTabStateChange,
}: UseXtermOptions): UseXtermReturn {

  // ═══════════════════════════════════════════════════════════════
  // 1. 字体大小（Ctrl+Wheel 共享 hook + store 订阅）
  //    fontSize prop 来自 TerminalPanel（store.terminalFontSize），fontSizeRef 跟踪当前值
  //    wheel handler 直接调 onFontSizeChange（即 store.setTerminalFontSize）
  // ═══════════════════════════════════════════════════════════════
  const fontSizeRef = useRef(fontSize ?? 14);
  fontSizeRef.current = fontSize ?? 14;
  useFontSizeWheel(container, FONT_SIZE_MIN, FONT_SIZE_MAX, fontSizeRef, (size) => {
    onFontSizeChange?.(size);
  });

  // ═══════════════════════════════════════════════════════════════
  // 2. Terminal 实例 + addon 生命周期（useTerminalInstance）
  // ═══════════════════════════════════════════════════════════════
  const {
    terminal: terminalRef,
    fitAddon: fitAddonRef,
    webglAddon: webglAddonRef,
    isDisposed: isDisposedRef,
    e2eTextBuffer: e2eTextBufferRef,
  } = useTerminalInstance(container, terminalOptions, fontSize);

  // ═══════════════════════════════════════════════════════════════
  // 3. 共享状态
  // ═══════════════════════════════════════════════════════════════

  /** Terminal 状态变量——当 Terminal 实例就绪时设置，触发下游 hook 的 useEffect 重新注册 */
  const [termState, setTermState] = useState<Terminal | null>(null);

  /** 最近一次 spawn 使用的尺寸（供 exit/retry 使用） */
  const lastColsRef = useRef(DEFAULT_COLS);
  const lastRowsRef = useRef(DEFAULT_ROWS);

  /** 共享的命令运行状态——useCommandDetection 写入，usePtyOutput exit 时读取 */
  const isCommandRunningRef = useRef(false);

  /** doSpawn 回调 ref（供 usePtyOutput 的 setupRetry / Enter 重连触发） */
  const doSpawnRef = useRef<((cols: number, rows: number) => void) | null>(null);

  // ── 运行中会话标题通道（人工验证问题 3）──
  // 节流时间戳与已应用标题——声明于 hook 顶层（不进 effect），
  // 容器 bridge 重渲染/effect 重建不重置节流与去重状态。
  /** 上次读标题时间戳（agent-event 5s 节流；SessionStart 归零强制立即查询） */
  const lastTitleFetchAtRef = useRef(0);
  /** 上次已应用标题（标题未变不重复回调——防 setTitle/onDidTitleChange 抖动） */
  const lastAppliedTitleRef = useRef<string | null>(null);

  // ═══════════════════════════════════════════════════════════════
  // 4. 终端快捷键上下文（active terminal + focus context）
  // ═══════════════════════════════════════════════════════════════

  // FE-08: write 连续失败计数（成功清零）——PTY 输入是终端关键路径，
  // 连续失败 ≥3 次说明管道已断，toast 提醒用户；单次失败多为瞬态，仅记日志
  const writeFailCountRef = useRef(0);

  /** write 失败统一处理：连续失败 ≥3 次 toast，其余 console.error（FE-08） */
  const handleWriteError = useCallback((err: unknown) => {
    writeFailCountRef.current += 1;
    if (writeFailCountRef.current >= WRITE_FAIL_TOAST_THRESHOLD) {
      writeFailCountRef.current = 0; // 每轮 3 次连续失败提醒一次，防 toast 风暴
      toast.show("error", `终端输入发送失败: ${getErrorMessage(err)}`);
    } else {
      console.error("PTY write 失败:", getErrorMessage(err));
    }
  }, []);

  const writeToPty = useCallback((data: Uint8Array) => {
    const sid = TerminalRegistry.get(panelId)?.sessionId;
    if (sid) {
      pty.write(sid, panelId, data)
        .then(() => {
          writeFailCountRef.current = 0;
        })
        .catch(handleWriteError);
    }
  }, [panelId, handleWriteError]);

  const terminalActions = useMemo<TerminalActions>(
    () => ({
      getSelection: () => terminalRef.current?.getSelection(),
      paste: (text: string) => terminalRef.current?.paste(text),
      writeToPty,
    }),
    [writeToPty],
  );
  const activateTerminal = useCallback(() => setActiveTerminal(terminalActions), [terminalActions]);
  const deactivateTerminal = useCallback(() => clearActiveTerminal(terminalActions), [terminalActions]);
  usePanelFocus("terminal", container, activateTerminal, deactivateTerminal);

  // ═══════════════════════════════════════════════════════════════
  // 5. 子 hook 调用
  // ═══════════════════════════════════════════════════════════════

  // PTY 输出处理（合帧缓冲 + Idle+Max 定时器 + 进程退出/重连）
  const { flushBuffer, cancelPendingFlush, dispose, handlePtyOutput, getPendingBuffer, setupRetry, retryDisposableRef } = usePtyOutput(
    terminalRef,
    panelId,
    visible ?? true,
    onTabStateChange,
    doSpawnRef,
    e2eTextBufferRef,
    isCommandRunningRef,
  );

  // 运行时 resize（ResizeObserver + X/Y 分离 debounce）
  const isReady = termState !== null && fitAddonRef.current !== null && !isDisposedRef.current;
  usePtyResize(
    container,
    termState,
    fitAddonRef.current,
    panelId,
    cancelPendingFlush,
    isReady,
    isDisposedRef,
  );

  // OSC 52 剪贴板拦截
  useClipboardHandler(termState, visible ?? true);

  // OSC 133 命令边界检测 → 页签标题/图标切换
  const { resetCommandState } = useCommandDetection(termState, panelId, onTabStateChange, isCommandRunningRef);

  // ═══════════════════════════════════════════════════════════════
  // 6. 主 useEffect：PTY spawn + onData + OSC 8 + 键盘委托 + E2E
  // ═══════════════════════════════════════════════════════════════
  useEffect(() => {
    if (!container) return;

    const term = terminalRef.current;
    const fitAddon = fitAddonRef.current;
    if (!term || !fitAddon) return;

    // 同步 termState 供下游 hook 使用
    setTermState(term);

    // F3: 动态设置 ConPTY buildNumber（ADR-0004 钳制——见 clampWindowsBuildForXterm）
    if (windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: clampWindowsBuildForXterm(windowsBuildNumber),
      };
    }

    // ── 键盘事件委托（xterm.js 内部 keydown 之前拦截）──
    term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (event.type !== "keydown") return true;
      const consumed = getShortcutRegistry().resolve(event, "terminal");
      if (consumed) {
        event.preventDefault();
        return false;
      }
      return true;
    });

    // OSC 8 超链接：点击后通过系统默认浏览器打开
    term.options.linkHandler = {
      activate: (_event, url) => {
        // FE-08: 非关键路径（openUrl）——打开失败不影响终端，仅 console.error
        openUrl(url).catch((err) =>
          console.error("打开链接失败:", getErrorMessage(err)),
        );
      },
    };

    // E2E 测试辅助钩子——委托给 helpers.ts（E2E_ENABLED 时编译期保留，生产 tree-shake）
    // __e2e_writeToTerminal / __e2e_getTerminalText 由 useTerminalInstance 在 E2E_ENABLED 时安装
    if (E2E_ENABLED) {
      setTerminalSessionReady(container, false);
      installTerminalWriteToPty(container, (data: string) => {
        const sid = TerminalRegistry.get(panelId)?.sessionId;
        if (sid) {
          pty.write(sid, panelId, new TextEncoder().encode(data))
            .catch((err) => console.error("E2E PTY write 失败:", getErrorMessage(err)));
        }
      });
    }

    // ── PTY spawn ──
    // rAF 轮询容器就绪 → fit → proposeDimensions → pty.spawn(真实尺寸)
    // 30 帧 / 500ms 上限，超时回退 80×24
    let fitRafId: number | null = null;
    let fitFrames = 0;
    const MAX_FRAMES = 30;
    const FIT_TIMEOUT = 500;
    const fitStartTime = performance.now();

    const doSpawn = (realCols: number, realRows: number) => {
      const cols = Number.isFinite(realCols) ? realCols : DEFAULT_COLS;
      const rows = Number.isFinite(realRows) ? realRows : DEFAULT_ROWS;
      if (!Number.isFinite(realCols) || !Number.isFinite(realRows)) {
        console.warn(
          `[H6] [warn] proposeDimensions 返回 NaN，回退 ${DEFAULT_COLS}x${DEFAULT_ROWS} (raw: cols=${realCols} rows=${realRows})`
        );
      }
      lastColsRef.current = cols;
      lastRowsRef.current = rows;

      pty
        .spawn({ panelId, cols, rows, cwd }, handlePtyOutput)
        .then((sessionId) => {
          // 注册到 TerminalRegistry（跨页面切换时可供 reattach 查询，约束 #8 单点元数据）
          TerminalRegistry.register(panelId, {
            term,
            sessionId,
            webglAddon: webglAddonRef.current,
            fitAddon,
          });
          if (E2E_ENABLED) setTerminalSessionReady(container, true);
          // PTY spawn 初始化：重置命令运行状态（覆盖持久化残留）
          resetCommandState();
        })
        .catch((err) => {
          term.writeln(`\r\n[重新连接] 按 Enter 重试...\r\n`);
          if (E2E_ENABLED) setTerminalSessionError(container, String(err));
          console.error(`[H6] spawn FAIL panelId="${panelId}"`, err);
          // FE-08: 关键路径（spawn 失败）——toast 提醒用户，终端内仍写重连提示
          toast.show("error", `终端启动失败: ${getErrorMessage(err)}`);
          // 设置 Enter 重连监听（不立即重新 spawn，由用户按 Enter 触发）
          setupRetry(cols, rows);
        });
    };

    // 暴露 doSpawn 供 usePtyOutput 的 setupRetry 通过 doSpawnRef 触发
    doSpawnRef.current = doSpawn;

    const pollFitAndSpawn = () => {
      fitFrames++;
      const elapsed = performance.now() - fitStartTime;

      if (container.offsetWidth > 0 && container.offsetHeight > 0) {
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          try {
            fitAddon.fit();
            const dims = fitAddon.proposeDimensions();
            if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
              doSpawn(dims.cols, dims.rows);
              return;
            }
          } catch {
            // fit 失败 → 回退
          }
        }
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
        return;
      }

      if (fitFrames >= MAX_FRAMES || elapsed >= FIT_TIMEOUT) {
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
        return;
      }

      fitRafId = requestAnimationFrame(pollFitAndSpawn);
    };

    fitRafId = requestAnimationFrame(pollFitAndSpawn);

    // 终端输入 → 后端
    term.onData((data) => {
      const sid = TerminalRegistry.get(panelId)?.sessionId;
      if (sid) {
        pty.write(
          sid,
          panelId,
          new TextEncoder().encode(data),
        ).then(() => {
          writeFailCountRef.current = 0;
        }).catch(handleWriteError);
      }
    });

    // ── Agent 事件订阅（F3 页签四态指示 + 会话状态写入）──
    const unsubscribeAgentEvent = onAgentEvent((payload) => {
      if (payload.panelId !== panelId) return;

      // MC-205 三级解析单点（ZQ-2，契约 4）：payload.cliId（trim 后非空）
      // → 反查注册表 agentSession.cliId → 缺省 CLAUDE_CLI_ID。
      // 旧信号无 cliId 字段（serde default）——缺省分支兼容；
      // 空串/仅空白与 null/undefined 同等回退（原 ?? 链遇空串短路失效）
      const cliId = resolvePayloadCliId(payload);
      const profile = cliProfileRegistry.get(cliId);
      const hooksCapability = profile?.capabilities?.hooks;

      // MC-206/403: 未知 cliId（未注册）或无 hooks 能力 → console.warn + 跳过
      // （不建行/不置图标/不通知），不抛异常
      if (!hooksCapability) {
        console.warn(
          `[hooks] 未知或无 hooks 能力的 cliId "${cliId}"——跳过 hook 事件（panelId=${panelId}）`,
        );
        return;
      }

      // 写入会话状态（与页签 emoji 正交）——四态映射委托 profile.hooks.eventToStatus
      const status = hooksCapability.eventToStatus(payload.event, payload.notificationType);
      if (payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT) {
        TerminalRegistry.setAgentSession(panelId, null);
      } else {
        TerminalRegistry.setAgentSession(panelId, {
          // sessionId/status 供历史区四态派生（问题 2 修复：两区同源 TerminalRegistry）
          // || undefined 空串防御：claude hook 输入缺字段时兜底成空串，
          // 空串会使下游（derive/标题覆盖/usage 拉取）全部失效——统一归一为 undefined
          sessionId: payload.sessionId || undefined,
          usageSourcePath: payload.usageSourcePath || undefined,
          // null 状态不传（undefined 保留旧值）——与活跃区行状态保留语义一致
          status: status ?? undefined,
        });
      }

      // ── 运行中会话标题（人工验证问题 3）：与历史 session 同源回退链 ──
      // custom-title > ai-title > summary > firstPrompt（后端 agent_history_read_title
      // 复用历史扫描 resolve_title）。SessionStart 立即查一次；其后 agent-event
      // 5s 节流重查（/rename custom-title、ai-title 在运行中变化）。
      // 无 sessionId（matchedCommand-only）/读取失败（未知 cliId 无 provider）/
      // 面板已卸载/会话已切换 → 静默保持现标题（兜底 = profile.tabTitle）。
      const sessionId = payload.sessionId || undefined;
      const refreshSessionTitle = (force: boolean) => {
        if (!sessionId || isDisposedRef.current) return;
        if (
          !force &&
          Date.now() - lastTitleFetchAtRef.current < TITLE_FETCH_THROTTLE_MS
        ) {
          return;
        }
        lastTitleFetchAtRef.current = Date.now();
        readHistoryTitle(cliId, sessionId)
          .then((resolved) => {
            // 陈旧守卫：应用前确认面板仍注册且会话未变（防迟到结果覆盖新会话标题）
            if (
              TerminalRegistry.get(panelId)?.agentSession?.sessionId !== sessionId
            ) {
              return;
            }
            const title = resolved?.title ?? profile?.tabTitle;
            if (!title || title === lastAppliedTitleRef.current) return;
            lastAppliedTitleRef.current = title;
            // 仅标题（不带 status）——不动状态圆点
            onTabStateChange?.({ active: true, title });
          })
          .catch((err) => {
            // 读取失败不影响现标题（无 provider 的 CLI 不炸，兜底 CLI 名）——
            // FE-08: 非关键路径仅 console.error，不打扰用户
            console.error("[slTerminal] 读取会话标题失败:", getErrorMessage(err));
          });
      };

      if (status === null) {
        // ZQ-6: 清图标条件对齐删 agentSession 的双事件判定（SessionEnd ∨ Exit，
        // 见上方 setAgentSession(null) 分支）——原仅 SessionEnd，Exit 事件
        // 会先删 agentSession 后漏清页签图标
        if (payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT) {
          // B13: 仅清图标不恢复标题——/resume 触发 SessionEnd→SessionStart
          // 时 claude 进程未退出，恢复会把标题误回退为 terminal-N；
          // 标题恢复只由真退出信号（OSC 133 D / PTY EXIT）承担
          onTabStateChange?.({ active: false, restoreTitle: false });
        }
        return;
      }
      // B13: SessionStart 补 title 重设——/resume 恢复历史会话时无 OSC 133 C
      // （TUI 内部斜杠命令不经 shell），标题经 profile.tabTitle 保持 claude；
      // profile 未注册（前置 hooksCapability 校验已拦截）→ 无 title 零副作用。
      // 人工验证问题 3：同步兜底 tabTitle 后，异步读历史同源标题覆盖
      //（节流时间戳归零——新会话强制立即查询）
      if (payload.event === SESSION_START_EVENT) {
        onTabStateChange?.({
          active: true,
          status,
          title: profile?.tabTitle,
        });
        // 记录同步已应用标题——异步回退同值时去重（不重复 setTitle）
        lastAppliedTitleRef.current = profile?.tabTitle ?? null;
        lastTitleFetchAtRef.current = 0;
        refreshSessionTitle(true);
        return;
      }
      // 非 SessionStart：5s 节流重查（/rename custom-title、ai-title 运行中变化）
      refreshSessionTitle(false);
      onTabStateChange?.({ active: true, status });
    });

    // ── 清理 ──
    return () => {
      unsubscribeAgentEvent();
      isDisposedRef.current = true;
      if (fitRafId !== null) {
        cancelAnimationFrame(fitRafId);
      }

      // FE-18: 先清理输出合帧——清除 idle/max 定时器 + 丢弃待 flush 缓冲（防卸载后定时器回调泄漏）
      dispose();

      // FE-08: 再清理 retry disposable（Terminal dispose 前的显式路径）
      retryDisposableRef.current?.dispose();
      retryDisposableRef.current = null;

      const entry = TerminalRegistry.get(panelId);
      if (entry) {
        // FE-08: 非关键路径（kill）——卸载清理中失败仅 console.error
        pty.kill(entry.sessionId, panelId).catch((err) =>
          console.error("终止 PTY 失败:", getErrorMessage(err)),
        );
      }
      TerminalRegistry.remove(panelId);
      doSpawnRef.current = null;
      // Terminal/addon 清理由 useTerminalInstance 的 useEffect cleanup 处理
    };
  }, [container, panelId, flushBuffer, cancelPendingFlush, dispose, handlePtyOutput, resetCommandState]);

  // F3 Bug 1 修复: 独立 useEffect 监听 windowsBuildNumber 异步更新（ADR-0004 钳制）
  useEffect(() => {
    const term = terminalRef.current;
    if (term && windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: clampWindowsBuildForXterm(windowsBuildNumber),
      };
    }
  }, [windowsBuildNumber]);

  // FE-34: WebGL 上下文不再随焦点切换创建/释放——挂载即加载（useTerminalInstance），
  // 切换仅 flush 非焦点期间累积的 PTY 输出。避免多终端快速切换时反复创建/销毁
  // GPU context 的重建延迟与首帧回退 DOM 闪烁。资源释放由两处兜底：
  // ① 面板卸载清理（useTerminalInstance performDispose → webglCancelRef → addon dispose）
  // ② context loss / 加载失败指数退避重试 → 耗尽回退 DOM renderer（webgl.ts）
  // 压力评估（静态）：多面板（≤MAX_PAGES=20）各持 1 个 context，超 Chromium 上限
  // （约 16）时后续 WebglAddon 构造失败走退避 → DOM 兜底，不崩溃——潜在压力无实测
  // 证据，是否超限由人工验证点（chrome://gpu GPU 内存观察）兜底，若实测有压力再恢复释放
  useEffect(() => {
    const term = terminalRef.current;
    if (!term) return;

    if (visible === true) {
      // 切回可见时 flush 非焦点期间累积的 PTY 输出
      flushBuffer();
    }
  }, [visible, flushBuffer]);

  // 字体大小变化 → fit + PTY resize
  useEffect(() => {
    const term = terminalRef.current;
    if (!term || fontSize === undefined) return;

    if (!canFit(term, fitAddonRef.current, container, isDisposedRef)) return;

    try {
      fitAddonRef.current!.fit();
      const dims = fitAddonRef.current!.proposeDimensions();
      const sid = TerminalRegistry.get(panelId)?.sessionId;
      if (dims && sid) {
        pty.resize(sid, panelId, dims.cols, dims.rows)
          .catch((err) => console.error("PTY resize 失败:", getErrorMessage(err)));
      }
    } catch {
      // fit 失败不影响渲染
    }
  }, [fontSize, container]);

  // ═══════════════════════════════════════════════════════════════
  // 7. 返回对外接口
  // ═══════════════════════════════════════════════════════════════
  return {
    focus: useCallback(() => {
      terminalRef.current?.focus();
    }, []),
    _test: {
      cancelPendingFlush,
      flushBuffer,
      getPendingBuffer,
    },
  };
}
