// EditorPanel — 编辑器面板 React 组件
//
// 挂载即创建 CodeMirror 实例 → readFile 填充内容。
// Ctrl+S 保存到文件。面板由 Dockview 管理生命周期。

import React, { useRef, useState, useEffect } from "react";
import { useCodeMirror } from "./useCodeMirror";
import { EDITOR_BG } from "../../theme";

interface EditorPanelProps {
  /** Dockview 传入的面板参数 */
  params: {
    panelId: string;
    filePath?: string;
  };
}

const EditorPanel: React.FC<EditorPanelProps> = ({ params }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);

  useEffect(() => {
    setContainer(containerRef.current);
  }, []);

  useCodeMirror({
    container,
    filePath: params.filePath,
    panelId: params.panelId,
  });

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        background: EDITOR_BG,
        overflow: "auto",
      }}
    />
  );
};

export default EditorPanel;
