// TerminalPanel — 终端面板 React 组件
//
// 挂载即创建 xterm.js 实例 → 获取 Windows build 号 → spawn PTY → 订阅输出 → 发送输入。
// 面板由 Dockview 管理生命周期。
// 从 Dockview params 读取 cwd 作为终端工作目录。

import React, { useRef, useState, useEffect, useMemo, useCallback } from "react";
import { useXterm } from "./useXterm";
import { pty } from "../../ipc";
import { useLayout, useFontSize } from "../../stores";
import { PANEL_BG, INPUT_BORDER } from "../../theme";
import type { TabState } from "./useCommandDetection";
import { TerminalRegistry } from "./TerminalRegistry";
import { cliProfileRegistry } from "../../features/cliProfiles";
// AC-5: 事件名字面量只允许出现在 profiles/claude/（claude 合法领地）——
// 缺省 cliId 兜底常量经 profiles/claude 导出（与 useAgentStatus 行建行口径一致）
import { CLAUDE_CLI_ID } from "../../features/cliProfiles/profiles/claude";
import type { DockviewPanelApi } from "dockview-react";

/** 加载遮罩兜底超时（ms）——首帧数据未到达时自动隐藏 */
const LOADING_MASK_TIMEOUT_MS = 1500;

interface TerminalPanelProps {
  /** Dockview 传入的面板 API */
  api: DockviewPanelApi;
  /** Dockview 传入的面板参数 */
  params: {
    panelId: string;
    /** 终端工作目录（可选） */
    cwd?: string;
    /** 用户自定义页签标题（右键菜单重命名，随布局持久化） */
    customTitle?: string;
    /** CLI 品牌 logo 根绝对路径（会话绑定写入，F9 修订；布局残留由挂载同步覆盖） */
    tabLogo?: string | null;
  };
}

const TerminalPanel: React.FC<TerminalPanelProps> = ({ api, params }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [loading, setLoading] = useState(true);
  const [buildNumber, setBuildNumber] = useState<number | undefined>(undefined);

  // 获取容器 DOM 引用
  useEffect(() => {
    setContainer(containerRef.current);
  }, []);

  // 获取真实 Windows build 号（动态设置 ConPTY reflow 阈值）
  useEffect(() => {
    let cancelled = false;
    pty.getWindowsBuildNumber().then((bn) => {
      if (!cancelled) {
        setBuildNumber(bn);
      }
    });
    return () => { cancelled = true; };
  }, []);

  const cwd = params.cwd;

  // P1-13: 从 panelId 解析所属 pageId，对比 activePageId 判断可见性
  // panelId 格式: terminal-{pageId}-{seq}
  const pageId = useMemo(() => {
    const match = params.panelId.match(/^terminal-(.+)-(\d+)$/);
    return match ? match[1] : "";
  }, [params.panelId]);
  const activePageId = useLayout((s) => s.activePageId);
  const visible = activePageId === pageId;

  // 字体大小：从 store 订阅 + 通过 setter 回调变更
  const terminalFontSize = useFontSize((s) => s.terminalFontSize);
  const setTerminalFontSize = useFontSize((s) => s.setTerminalFontSize);

  // 命令运行状态变化 → 更新 Dockview 页签标题和图标
  // originalTitleRef 仅在组件挂载时初始化，active=true 时不覆盖
  // 挂载时优先取 params.customTitle（重命名后的自定义标题）——重启恢复时布局 JSON
  // 的 title 字段可能是瞬态值（如 claude 运行中退出），customTitle 才是真名
  const originalTitleRef = useRef(params.customTitle ?? api.title ?? "terminal");
  // 最新参数快照：props 同步 + onDidParametersChange 同步（下方订阅）——tabIcon/tabLogo
  // 由两处 updateParameters 分头写入，互不可见；合并必须基于最新参数而非 props 快照
  // （快照覆盖会抹掉另一路径刚写入的键——mockcli E2E 冒烟 tabIcon 丢失根因）
  const latestParamsRef = useRef<Record<string, unknown>>(params);
  latestParamsRef.current = params;
  const handleTabStateChange = useCallback((state: TabState) => {
    if (state.active) {
      // 仅当 title/icon 存在时才更新，不覆盖 originalTitleRef
      if (state.title) api.setTitle(state.title);
      if (state.icon !== undefined) {
        api.updateParameters({ ...latestParamsRef.current, tabIcon: state.icon });
      }
    } else {
      api.setTitle(originalTitleRef.current);
      // F9 行为修订：只清 icon——logo 跟随 agentSession 生命周期（会话绑定），
      // 由下方 TerminalRegistry 订阅驱动清除，此处不再双清
      api.updateParameters({ ...latestParamsRef.current, tabIcon: null });
    }
  }, [api]);

  // F9 行为修订：页签 logo 会话绑定——agentSession 存在即显示 logo（按 cliId 查
  // profile.iconSrc，与 agent 侧栏行同源），会话结束（删行）即消失。
  // register 事件同样触发同步：重启恢复时 agentSession 未设置 → 清布局 JSON
  // 持久化残留；页面切回重挂载时（H6）register 幂等保留旧 session → logo 立即恢复。
  // deps 仅 [api, panelId]（panelId 经 ref 读）——tabIcon 高频更新（hook 事件）不重建订阅。
  const panelIdRef = useRef(params.panelId);
  panelIdRef.current = params.panelId;
  useEffect(() => {
    const syncTabLogo = () => {
      const session = TerminalRegistry.get(panelIdRef.current)?.agentSession;
      // cliId 缺省兜底口径与 useAgentStatus 行建行一致（entry.agentSession.cliId ?? CLAUDE_CLI_ID）
      const tabLogo = session
        ? cliProfileRegistry.get(session.cliId ?? CLAUDE_CLI_ID)?.iconSrc ?? null
        : null;
      // 基于最新参数合并（见 latestParamsRef 注释）——快照覆盖会抹掉 tabIcon
      api.updateParameters({ ...latestParamsRef.current, tabLogo });
    };
    // 挂载立即同步一次：覆盖布局恢复的 tabLogo 残留（未注册会话 → null）
    syncTabLogo();
    const unsubscribe = TerminalRegistry.subscribe((e) => {
      if (e.panelId !== panelIdRef.current) return;
      if (e.type !== "register" && e.type !== "sessionChange") return;
      syncTabLogo();
    });
    return unsubscribe;
  }, [api, params.panelId]);

  // 参数变化同步：① 最新参数快照合并（tabIcon/tabLogo/customTitle 多路径分头写入，
  // 互不可见——合并基准必须随每次参数变化更新）；② 重命名同步 originalTitleRef，
  // 保证 OSC 133 D 恢复时用自定义名而非挂载时旧名
  useEffect(() => {
    const d = api.onDidParametersChange((p) => {
      latestParamsRef.current = { ...latestParamsRef.current, ...p };
      const tp = p as { customTitle?: string };
      if (tp?.customTitle !== undefined) {
        originalTitleRef.current = tp.customTitle;
      }
    });
    return () => d.dispose();
  }, [api]);

  const { focus } = useXterm({
    container,
    cols: 80,
    rows: 24,
    panelId: params.panelId,
    windowsBuildNumber: buildNumber,
    cwd,
    visible,
    fontSize: terminalFontSize,
    onFontSizeChange: setTerminalFontSize,
    onTabStateChange: handleTabStateChange,
  });

  // 首帧数据到达时隐藏加载遮罩
  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), LOADING_MASK_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, []);

  return (
    // E2E 定位锚点：app 可能恢复用户布局的多终端面板，__e2e_* helper 经
    // data-panel-id 按 panelId 精确定位（见 e2e-tests/mockcli.e2e.ts 首匹配歧义说明）
    <div
      ref={containerRef}
      data-e2e="terminal-container"
      data-panel-id={params.panelId}
      style={{
        width: "100%",
        height: "100%",
        background: PANEL_BG,
        position: "relative",
      }}
      onClick={focus}
    >
      {loading && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: INPUT_BORDER,
            fontSize: 14,
            background: PANEL_BG,
            transition: "opacity 0.3s",
            pointerEvents: "none",
          }}
        >
          正在连接...
        </div>
      )}
    </div>
  );
};

export default TerminalPanel;
