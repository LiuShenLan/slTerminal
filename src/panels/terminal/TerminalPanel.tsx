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
import type { TabState } from "./TabTitleRegistry";
import type { DockviewPanelApi } from "dockview-react";

interface TerminalPanelProps {
  /** Dockview 传入的面板 API */
  api: DockviewPanelApi;
  /** Dockview 传入的面板参数 */
  params: {
    panelId: string;
    /** 终端工作目录（可选） */
    cwd?: string;
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

  // F3: IPC 获取真实 Windows build 号（动态设置 ConPTY reflow 阈值）
  useEffect(() => {
    pty.getWindowsBuildNumber().then((bn) => {
      setBuildNumber(bn);
    });
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
  const originalTitleRef = useRef(api.title ?? "terminal");
  const handleTabStateChange = useCallback((state: TabState) => {
    if (state.active) {
      originalTitleRef.current = api.title ?? "terminal";
      api.setTitle(state.title!);
      api.updateParameters({ ...params, tabIcon: state.icon! });
    } else {
      api.setTitle(originalTitleRef.current);
      api.updateParameters({ ...params, tabIcon: null });
    }
  }, [api, params]);

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
    const timer = setTimeout(() => setLoading(false), 1500);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div
      ref={containerRef}
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
