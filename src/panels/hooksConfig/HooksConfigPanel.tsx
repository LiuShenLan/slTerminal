// HooksConfigPanel — hooks 配置面板（F6）Stage 06/07（P3-FE-02 + P3-FE-11/12 + P3-FE-16/17 + P3-FE-21/22 接入）
//
// 顶部工具栏：层级切换器（user/project/local，标注优先级 local>project>user）
// + 模式切换（GUI | JSON，默认 JSON）+ F2 注入状态条与注入/卸载按钮（P3-FE-21/22）+ 重启提示条 + 保存按钮。
// 中部为模式渲染容器：JSON 模式渲染 JsonMode；GUI 模式渲染 GuiMode
// （Master-Detail 事件树 + 详情区）。
// 双模式同步（P3-FE-16）：JsonMode.onChange → updateConfigJson（JSON.parse 门控），
// GuiMode.onChange → updateGui（guiToJson），configJson/guiModel/dirty 共享于 useHooksConfig。
// JSON 非法（onValidationChange 上报 false）→ GUI 按钮禁用 + 工具栏错误提示。
// 保存（P3-FE-17）：按钮经 useHooksConfig.save() 走语法 + schema 双校验 → filterDisabled
// → writeHooksConfig；成功后状态条显示「hooks 改动需重启 claude 会话生效」。
// 三态：loading → content / error（损坏错误态——read 返回 Err，与无配置 null 区分）。
// 保存按钮：dirty 且 JSON 合法（onValidationChange 上报）才可点。
// F2 注入（P3-FE-21/22）：工具栏「注入 Hooks」/「卸载 Hooks」按钮调用 src/ipc/hooks 的
// inject()/uninstall()（不改其实现）；注入状态条显示 getInjectionStatus() 三态
// （已注入/未注入/版本过旧）；注入/卸载完成后刷新状态 + 自动重读 user 层配置
// （操作改写 ~/.claude/settings.json，C13-8——当前层为 user 直接 reload，非 user 切到 user 层）。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useCallback, useEffect, useState } from "react";
import { useHooksConfig } from "./useHooksConfig";
import JsonMode from "./JsonMode";
import GuiMode from "./GuiMode";
import {
  inject,
  uninstall,
  getInjectionStatus,
  type HookInjectionStatus,
} from "../../ipc/hooks";
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

/** 注入状态显示文案（P3-FE-22）：null = 未查询/查询中；三态照契约 C6 status 枚举 */
function injectionStatusText(status: HookInjectionStatus | null): string {
  if (status === null) return "--";
  switch (status.status) {
    case "injected":
      return "已注入";
    case "notInjected":
      return "未注入";
    case "outdated":
      return "版本过旧";
  }
}

/** 注入状态显示颜色（硬约束 #6 token）：已注入正常色 / 未注入次要灰 / 版本过旧警示色 */
function injectionStatusColor(status: HookInjectionStatus | null): string {
  if (status === null || status.status === "notInjected") return HTML_PANEL_LOADING_FG;
  if (status.status === "outdated") return ERROR_FG;
  return SIDEBAR_FG;
}

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
    disabledKeys,
    staleDisabledKeys,
    toggleDisable,
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

  // F2 注入状态（P3-FE-22）：挂载查询一次 + 注入/卸载后刷新；null = 查询中/未查询
  const [injectionStatus, setInjectionStatus] = useState<HookInjectionStatus | null>(null);
  // 注入/卸载操作进行中（防重复点击）
  const [injectionBusy, setInjectionBusy] = useState(false);
  // 注入/卸载失败提示（如 ~/.claude/settings.json 为非法 JSON 被后端拒绝）
  const [injectionError, setInjectionError] = useState<string | null>(null);

  /** 刷新注入状态（挂载 / 注入 / 卸载后调用）；查询失败 console.warn 降级，状态条保持上次值 */
  const refreshInjectionStatus = useCallback(async () => {
    try {
      setInjectionStatus(await getInjectionStatus());
    } catch (err) {
      console.warn("[slTerminal] 查询 hooks 注入状态失败:", err);
    }
  }, []);

  // 挂载时查询一次注入状态
  useEffect(() => {
    void refreshInjectionStatus();
  }, [refreshInjectionStatus]);

  /** 注入/卸载完成后自动重读 user 层配置（操作改写 ~/.claude/settings.json，C13-8）：
      当前层为 user → reload；非 user → 切到 user 层（dirty 守卫由 useHooksConfig 内部 ask 处理，
      用户拒绝丢弃则不覆盖——与既有切层/重载语义一致） */
  const reloadUserConfig = useCallback(() => {
    if (layer === "user") {
      void reload();
    } else {
      setLayer("user");
    }
  }, [layer, reload, setLayer]);

  /** 注入：成功后用返回值刷新状态 + 重读 user 层配置；失败显示错误提示（保留 dirty 不丢用户修改） */
  const handleInject = useCallback(async () => {
    setInjectionBusy(true);
    setInjectionError(null);
    try {
      setInjectionStatus(await inject());
      reloadUserConfig();
    } catch (err) {
      console.error("[slTerminal] hooks 注入失败:", err);
      setInjectionError("注入失败，请检查 ~/.claude/settings.json");
    } finally {
      setInjectionBusy(false);
    }
  }, [reloadUserConfig]);

  /** 卸载：成功后重新查询状态（uninstall 返回 void）+ 重读 user 层配置；失败显示错误提示 */
  const handleUninstall = useCallback(async () => {
    setInjectionBusy(true);
    setInjectionError(null);
    try {
      await uninstall();
      await refreshInjectionStatus();
      reloadUserConfig();
    } catch (err) {
      console.error("[slTerminal] hooks 卸载失败:", err);
      setInjectionError("卸载失败，请检查 ~/.claude/settings.json");
    } finally {
      setInjectionBusy(false);
    }
  }, [refreshInjectionStatus, reloadUserConfig]);

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
        {/* 常驻提示（P3-FE-19/ADR-0002）：禁用条目由 slTerminal 托管，不出现在配置文件中 */}
        <span style={hintStyle} data-e2e="hooks-disabled-hint">
          禁用条目由 slTerminal 托管，不出现在配置文件中
        </span>
        {/* F2 注入状态条（P3-FE-22）：已注入 / 未注入 / 版本过旧；注入/卸载按钮（P3-FE-21）——
            busy 期间禁用防重复点击 */}
        <span
          style={{ ...hintStyle, color: injectionStatusColor(injectionStatus) }}
          data-e2e="hooks-injection-status"
        >
          注入状态：{injectionStatusText(injectionStatus)}
        </span>
        {injectionError && (
          <span style={{ ...hintStyle, color: ERROR_FG }} data-e2e="hooks-injection-error">
            {injectionError}
          </span>
        )}
        <button
          type="button"
          data-e2e="hooks-inject"
          disabled={injectionBusy}
          onClick={() => void handleInject()}
          style={{
            padding: "3px 12px",
            fontSize: 12,
            cursor: injectionBusy ? "default" : "pointer",
          }}
        >
          注入 Hooks
        </button>
        <button
          type="button"
          data-e2e="hooks-uninstall"
          disabled={injectionBusy}
          onClick={() => void handleUninstall()}
          style={{
            padding: "3px 12px",
            fontSize: 12,
            cursor: injectionBusy ? "default" : "pointer",
          }}
        >
          卸载 Hooks
        </button>
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
      {/* 失效禁用记录条（P3-FE-19）：四元组在当前层配置中找不到匹配（外部修改/手动改 JSON 失配）——
          标记而非静默丢弃（ADR-0002）；点击「启用」移除记录（= 删除失效记录，记录从禁用列表删除） */}
      {staleDisabledKeys.length > 0 && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            alignItems: "center",
            gap: 6,
            padding: "4px 10px",
            borderBottom: `1px solid ${INPUT_BORDER}`,
          }}
          data-e2e="hooks-stale-disabled"
        >
          <span style={{ ...hintStyle, color: ERROR_FG }}>失效的禁用记录：</span>
          {staleDisabledKeys.map((k, i) => (
            <span key={i} style={hintStyle} data-e2e={`hooks-stale-key-${i}`}>
              {k.event} / {k.matcher ?? "全匹配"} / {k.command === "" ? "整组" : k.command}
              <button
                type="button"
                data-e2e={`hooks-stale-enable-${i}`}
                onClick={() => toggleDisable({ event: k.event, matcher: k.matcher, command: k.command })}
                style={{
                  marginLeft: 4,
                  padding: "0 6px",
                  fontSize: 11,
                  cursor: "pointer",
                  color: SIDEBAR_FG,
                  background: "transparent",
                  border: `1px solid ${INPUT_BORDER}`,
                  borderRadius: 3,
                }}
              >
                启用
              </button>
            </span>
          ))}
        </div>
      )}
      {/* 模式渲染容器（JSON = JsonMode；GUI = GuiMode）——onChange 均接入 useHooksConfig setter（P3-FE-16） */}
      <div style={modeContainerStyle} data-e2e="hooks-mode-container">
        {mode === "gui" ? (
          <GuiMode
            gui={guiModel as unknown as ConfigGui}
            onChange={updateGui}
            disabledKeys={disabledKeys}
            onToggleDisabled={toggleDisable}
          />
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
