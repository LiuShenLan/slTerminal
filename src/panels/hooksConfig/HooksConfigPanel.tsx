// HooksConfigPanel — hooks 配置面板（F6）Stage 06（P3-FE-02 + P3-FE-11/12 + P3-FE-16/17 接入）
//
// 顶部工具栏：层级切换器（user/project/local，标注优先级 local>project>user）
// + 模式切换（GUI | JSON，默认 JSON）+ 注入状态条占位 + 重启提示条 + 保存按钮。
// 中部为模式渲染容器：JSON 模式渲染 JsonMode；GUI 模式渲染 GuiMode
// （Master-Detail 事件树 + 详情区）。
// 双模式同步（P3-FE-16）：JsonMode.onChange → updateConfigJson（JSON.parse 门控），
// GuiMode.onChange → updateGui（guiToJson），configJson/guiModel/dirty 共享于 useHooksConfig。
// JSON 非法（onValidationChange 上报 false）→ GUI 按钮禁用 + 工具栏错误提示。
// 保存（P3-FE-17）：按钮经 useHooksConfig.save() 走语法 + schema 双校验 → filterDisabled
// → writeHooksConfig；成功后状态条显示「hooks 改动需重启 claude 会话生效」。
// 三态：loading → content / error（损坏错误态——read 返回 Err，与无配置 null 区分）。
// 保存按钮：dirty 且 JSON 合法（onValidationChange 上报）才可点。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useCallback, useState } from "react";
import { useHooksConfig } from "./useHooksConfig";
import JsonMode from "./JsonMode";
import GuiMode from "./GuiMode";
import type { HooksConfigGui as ConfigGui } from "./configModel";
import type { HooksConfigJson, HooksLayer } from "../../types/hooksConfig";
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

/** 模式渲染容器样式（JsonMode 撑满，flex 列布局） */
const modeContainerStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  minHeight: 0,
};

const HooksConfigPanel: React.FC<HooksConfigPanelProps> = () => {
  const {
    layer,
    setLayer,
    rootPath,
    configJson,
    guiModel,
    dirty,
    saved,
    error,
    loading,
    save,
    reload,
    updateConfigJson,
    updateGui,
  } = useHooksConfig();
  // JsonMode 校验上报：非法 JSON / schema 违规 → 禁用保存 + 禁用切 GUI + 工具栏错误提示（P3-FE-16）
  const [jsonValid, setJsonValid] = useState(true);
  const [jsonError, setJsonError] = useState<string | null>(null);
  // 模式切换（GUI | JSON），默认 JSON；JSON 非法时 GUI 按钮禁用（P3-FE-16）
  const [mode, setMode] = useState<"gui" | "json">("json");

  /** JsonMode onChange：合法 JSON 才更新 configJson（非法保留最后合法快照，仅校验上报） */
  const handleJsonChange = useCallback(
    (text: string) => {
      try {
        updateConfigJson(JSON.parse(text) as HooksConfigJson);
      } catch {
        // 非法 JSON：不更新 configJson（保留最后合法快照），onValidationChange 已上报
      }
    },
    [updateConfigJson],
  );

  /** 保存按钮：失败仅 console.error（保留 dirty，不丢用户修改）；校验失败弹窗由 useHooksConfig.save 内部处理 */
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
        {/* 模式切换（GUI | JSON）——JSON 非法时 GUI 按钮禁用（P3-FE-16） */}
        <span style={{ display: "flex", gap: 4 }}>
          <button
            type="button"
            data-e2e="hooks-mode-gui"
            disabled={!jsonValid}
            title={!jsonValid ? "JSON 存在错误，无法切换到 GUI 模式" : undefined}
            onClick={() => setMode("gui")}
            style={layerButtonStyle(mode === "gui")}
          >
            GUI
          </button>
          <button
            type="button"
            data-e2e="hooks-mode-json"
            onClick={() => setMode("json")}
            style={layerButtonStyle(mode === "json")}
          >
            JSON
          </button>
        </span>
        {/* JSON 非法错误提示（P3-FE-16）——显示首条诊断；恢复合法后隐藏 */}
        {!jsonValid && (
          <span style={{ ...hintStyle, color: ERROR_FG }} data-e2e="hooks-json-error">
            JSON 存在错误，无法切换 GUI：{jsonError ?? "配置不符合 schema"}
          </span>
        )}
        {/* 注入状态条占位——Stage 07 并入注入状态 */}
        <span style={hintStyle}>注入状态：--</span>
        {/* 保存成功提示条（P3-FE-17）——hooks 改动需重启 claude 会话生效；下次编辑/重载后隐藏 */}
        {saved && (
          <span style={hintStyle} data-e2e="hooks-restart-hint">
            hooks 改动需重启 claude 会话生效
          </span>
        )}
        {/* 保存按钮（dirty 且 JSON 合法才可点） */}
        <button
          type="button"
          data-e2e="hooks-save"
          disabled={!dirty || !jsonValid}
          onClick={handleSave}
          style={{
            padding: "3px 12px",
            fontSize: 12,
            cursor: dirty && jsonValid ? "pointer" : "default",
          }}
        >
          保存
        </button>
      </div>
      {/* 模式渲染容器（JSON = JsonMode；GUI = GuiMode）——onChange 均接入 useHooksConfig setter（P3-FE-16） */}
      <div style={modeContainerStyle} data-e2e="hooks-mode-container">
        {mode === "gui" ? (
          <GuiMode gui={guiModel as unknown as ConfigGui} onChange={updateGui} />
        ) : (
          <JsonMode
            value={JSON.stringify(configJson, null, 2)}
            onChange={handleJsonChange}
            onValidationChange={(isValid, diagnostics) => {
              setJsonValid(isValid);
              setJsonError(isValid ? null : (diagnostics[0]?.message ?? null));
            }}
          />
        )}
      </div>
    </div>
  );
};

export default HooksConfigPanel;
