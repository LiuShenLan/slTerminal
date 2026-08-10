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
    /** CLI 品牌 logo 根绝对路径（随布局持久化，spawn 成功重置清除残留） */
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
  // 当前 CLI logo 根绝对路径：仅 OSC 133 C（有 command）更新；hook 事件路径
  // （无 command）不传 logo 保持前值；null 清除（该 CLI 未注册 logo）
  const logoRef = useRef<string | null>(params.tabLogo ?? null);
  const handleTabStateChange = useCallback((state: TabState) => {
    if (state.active) {
      // 仅当 title/icon 存在时才更新，不覆盖 originalTitleRef
      if (state.title) api.setTitle(state.title);
      if (state.logo !== undefined) logoRef.current = state.logo;
      if (state.icon !== undefined) {
        api.updateParameters({ ...params, tabIcon: state.icon, tabLogo: logoRef.current });
      }
    } else {
      api.setTitle(originalTitleRef.current);
      // 双清 icon + logo（覆盖布局 JSON 持久化残留，spawn 成功后 resetCommandState 触发）
      api.updateParameters({ ...params, tabIcon: null, tabLogo: null });
    }
  }, [api, params]);

  // 重命名同步：右键菜单重命名会 updateParameters({ customTitle })，订阅参数变化
  // 更新原标题基准，保证 OSC 133 D 恢复时用自定义名而非挂载时旧名
  useEffect(() => {
    const d = api.onDidParametersChange((p) => {
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
    <div
      ref={containerRef}
      data-e2e="terminal-container"
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
