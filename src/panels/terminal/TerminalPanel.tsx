// TerminalPanel — 终端面板 React 组件
//
// 挂载即创建 xterm.js 实例 → spawn PTY → 订阅输出 → 发送输入。
// 面板由 Dockview 管理生命周期。

import React, { useRef, useState, useEffect } from "react";
import { useXterm } from "./useXterm";

interface TerminalPanelProps {
  /** Dockview 传入的面板参数 */
  params: {
    panelId: string;
  };
}

const TerminalPanel: React.FC<TerminalPanelProps> = ({ params }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [loading, setLoading] = useState(true);

  // 获取容器 DOM 引用
  useEffect(() => {
    setContainer(containerRef.current);
  }, []);

  const { focus } = useXterm({
    container,
    cols: 80,
    rows: 24,
    panelId: params.panelId,
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
