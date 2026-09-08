// EditorPanel — 编辑器面板 React 组件
//
// 挂载即创建 CodeMirror 实例 → readFile 填充内容。
// Ctrl+S 保存到文件。面板由 Dockview 管理生命周期。
// CP-022: useCodeMirror 返回 largeFile 信号（>10MB 文件）时,同面板内形态切换
// 为 LargeFileViewer 只读分片浏览（不经 panelRegistry 新类型;CM 编辑区不挂载）。

import React, { useRef, useState, useEffect } from "react";
import { useCodeMirror } from "./useCodeMirror";
import { useFontSize } from "../../stores";
import { EDITOR_BG } from "../../theme";
import { LargeFileViewer } from "./largeFileViewer/LargeFileViewer";

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

  const editorFontSize = useFontSize((s) => s.editorFontSize);
  const setEditorFontSize = useFontSize((s) => s.setEditorFontSize);

  const { largeFile } = useCodeMirror({
    container,
    filePath: params.filePath,
    panelId: params.panelId,
    fontSize: editorFontSize,
    onFontSizeChange: setEditorFontSize,
  });

  // 大文件形态（只读浏览）: CM 容器 div 不渲染 → 容器节点重建;容器状态须随
  // 形态切换重新捕获（视图中仅 isViewer 变化驱动——大文件信号置位/文件切换清空
  // 都会翻转形态,旧容器引用已脱离文档,不复捕获会导致 view 挂到游离节点）
  const isViewer = largeFile !== null;
  useEffect(() => {
    setContainer(containerRef.current);
  }, [isViewer]);

  if (isViewer) {
    return (
      <LargeFileViewer
        filePath={largeFile.filePath}
        fileSizeBytes={largeFile.sizeBytes}
        sourceLabel=""
      />
    );
  }

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        background: EDITOR_BG,
        overflow: "clip",
      }}
    />
  );
};

export default EditorPanel;
