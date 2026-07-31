// HooksConfigPanel — hooks 配置面板（F6）Stage 03 骨架（P3-FE-02）
//
// 顶部工具栏：层级切换器（user/project/local，标注优先级 local>project>user）
// + 模式切换（GUI | JSON）占位 + 注入状态条占位 + 保存按钮。
// 中部为模式渲染容器，Stage 03 显示占位文案（JsonMode/GuiMode 后续 Stage 实现）。
// 三态：loading → content / error（损坏错误态——read 返回 Err，与无配置 null 区分）。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useCallback } from "react";
import { useHooksConfig } from "./useHooksConfig";
import type { HooksLayer } from "../../types/hooksConfig";
import { PANEL_BG, ERROR_FG, HTML_PANEL_LOADING_FG, INPUT_BORDER, SIDEBAR_FG } from "../../theme";

/** HooksConfigPanel 面板参数——单例面板无需 panelId（Stage 08 同页单例），保留 props 兼容 Dockview */
interface HooksConfigPanelProps {
  params?: { panelId?: string };
}

/** 层级定义（显示优先级标注 local>project>user） */
const LAYERS: { id: HooksLayer; label: string; hint: string }[] = [
  { id: "user", label: "User", hint: "用户级（全局生效，优先级最低）" },
  { id: "project", label: "Project", hint: "项目级（当前项目生效）" },
  { id: "local", label: "Local", hint: "本地级（当前项目生效，优先级最高）" },
];

/** 优先级标注文案 */
const PRIORITY_HINT = "优先级：Local > Project > User";
/** Stage 03 模式容器占位文案（JsonMode/GuiMode 后续 Stage 实现） */
const MODE_PLACEHOLDER_TEXT = "配置编辑区将在后续阶段实现（GUI / JSON 模式）";

/** 居中容器样式（loading / error 共用） */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

/** 面板根容器样式 */
const containerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  background: PANEL_BG,
  display: "flex",
  flexDirection: "column",
};

/** 顶部工具栏样式 */
const toolbarStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 10px",
  borderBottom: `1px solid ${INPUT_BORDER}`,
};

/** 层级按钮样式（active 高亮） */
function layerButtonStyle(active: boolean): React.CSSProperties {
  return {
    padding: "3px 10px",
    fontSize: 12,
    cursor: "pointer",
    background: active ? INPUT_BORDER : "transparent",
    color: SIDEBAR_FG,
    border: `1px solid ${INPUT_BORDER}`,
    borderRadius: 3,
  };
}

/** 次要提示文案样式（优先级标注 / 占位文案） */
const hintStyle: React.CSSProperties = {
  fontSize: 11,
  color: HTML_PANEL_LOADING_FG,
};

/** 模式渲染容器样式 */
const modeContainerStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: HTML_PANEL_LOADING_FG,
  fontSize: 13,
};

const HooksConfigPanel: React.FC<HooksConfigPanelProps> = () => {
  const { layer, setLayer, rootPath, dirty, error, loading, save, reload } = useHooksConfig();

  /** 保存按钮：失败仅 console.error（保留 dirty，不丢用户修改） */
  const handleSave = useCallback(() => {
    void save().catch((err) => {
      console.error("[slTerminal] hooks 配置保存失败:", err);
    });
  }, [save]);

  /** 面板聚焦（focusin）轻量重读——外部修改检测，dirty 时 ask 确认（useHooksConfig 内部处理） */
  const handleFocus = useCallback(() => {
    void reload();
  }, [reload]);

  if (loading) {
    return (
      <div style={centerStyle}>
        <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>加载中...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div style={centerStyle}>
        <span style={{ color: ERROR_FG, fontSize: 13 }}>配置文件损坏，请先修复</span>
      </div>
    );
  }

  return (
    <div style={containerStyle} onFocus={handleFocus} data-e2e="hooks-config-panel">
      {/* 顶部工具栏 */}
      <div style={toolbarStyle}>
        {/* 层级切换器（rootPath 为空时 project/local 禁用，仅 user 层可用） */}
        {LAYERS.map((l) => (
          <button
            key={l.id}
            type="button"
            title={l.hint}
            data-e2e={`hooks-layer-${l.id}`}
            disabled={l.id !== "user" && !rootPath}
            onClick={() => setLayer(l.id)}
            style={layerButtonStyle(layer === l.id)}
          >
            {l.label}
          </button>
        ))}
        <span style={hintStyle}>{PRIORITY_HINT}</span>
        <span style={{ flex: 1 }} />
        {/* 模式切换（GUI | JSON）占位——JsonMode/GuiMode 后续 Stage 实现 */}
        <span style={hintStyle}>GUI | JSON</span>
        {/* 注入状态条占位——Stage 07 并入注入状态 */}
        <span style={hintStyle}>注入状态：--</span>
        {/* 保存按钮（dirty 才可点） */}
        <button
          type="button"
          data-e2e="hooks-save"
          disabled={!dirty}
          onClick={handleSave}
          style={{ padding: "3px 12px", fontSize: 12, cursor: dirty ? "pointer" : "default" }}
        >
          保存
        </button>
      </div>
      {/* 模式渲染容器（Stage 03 占位） */}
      <div style={modeContainerStyle} data-e2e="hooks-mode-container">
        {MODE_PLACEHOLDER_TEXT}
      </div>
    </div>
  );
};

export default HooksConfigPanel;
