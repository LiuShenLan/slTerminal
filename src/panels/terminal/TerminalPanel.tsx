// TerminalPanel — 终端面板 React 组件
//
// 挂载即创建 xterm.js 实例 → 获取 Windows build 号 → spawn PTY → 订阅输出 → 发送输入。
// 面板由 Dockview 管理生命周期。
// 从 Dockview params 读取 binding?.worktreePath 作为 cwd。

import React, { useRef, useState, useEffect } from "react";
import { useXterm } from "./useXterm";
import { pty } from "../../ipc";
import type { WorktreeBinding } from "../../types/git";

interface TerminalPanelProps {
  /** Dockview 传入的面板参数 */
  params: {
    panelId: string;
    /** worktree 绑定信息（可选），用于设置终端 cwd */
    binding?: WorktreeBinding;
  };
}

const TerminalPanel: React.FC<TerminalPanelProps> = ({ params }) => {
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
      console.log("[slTerminal] Windows build:", bn);
      setBuildNumber(bn);
    });
  }, []);

  // 从 binding 计算 cwd
  const cwd = params.binding?.worktreePath;

  const { focus } = useXterm({
    container,
    cols: 80,
    rows: 24,
    panelId: params.panelId,
    windowsBuildNumber: buildNumber,
    cwd,
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
        background: "#1E1E1E",
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
            color: "#6C6C6C",
            fontSize: 14,
            background: "#1E1E1E",
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
