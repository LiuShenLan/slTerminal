// ClaudeHooksConfigEditor — claude hooks 配置编辑器（Stage 06 hub 化后整体下移一层，MC-504）
//
// 内容 = 原 HooksConfigPanel 全部：顶部工具栏（层级切换器——数据源 = profile 声明的
// capabilities.hooks.configLayers，KZ-4（claude 三层值 user/project/local 迁入 claude profile，
// 优先级标注 local>project>user 保留编辑器内部硬编码）+ 模式切换 GUI | JSON 默认 JSON
// + F2 注入状态条与注入/卸载按钮（P3-FE-21/22）+ 重启提示条 + 保存按钮）
// + 中部模式渲染容器（JsonMode / GuiMode，Master-Detail 事件树 + 详情区）。
// 双模式同步（P3-FE-16）：JsonMode.onChange → updateConfigJson（JSON.parse 门控），
// GuiMode.onChange → updateGui（guiToJson），configJson/guiModel/dirty 共享于 useHooksConfig。
// JSON 非法（onValidationChange 上报 false）→ GUI 按钮禁用 + 工具栏错误提示。
// 保存（P3-FE-17）：按钮经 useHooksConfig.save() 走语法 + schema 双校验
// → writeHooksConfig；成功后状态条显示重启提示（文案 = profile.capabilities.hooks.restartHint 驱动，MC-506）。
// 三态：loading → content / error（损坏错误态——read 返回 Err，与无配置 null 区分）。
// 保存按钮：dirty 且 JSON 合法（onValidationChange 上报）才可点。
// F2 注入（P3-FE-21/22）：工具栏「注入 Hooks」/「卸载 Hooks」按钮调用 src/ipc/agentHooks 的
// inject()/uninstall()（cliId 实参 = hub 选中态 profile.id——Stage 03 中间态已回收）；
// 注入状态条显示 getInjectionStatus() 三态（已注入/未注入/版本过旧，数据源 = 选中态 cliId，MC-506）；
// 注入/卸载完成后刷新状态 + 自动重读 user 层配置
// （操作改写 ~/.claude/settings.json，C13-8——当前层为 user 直接 reload，非 user 切到 user 层）。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。
//
// hub 契约（跨边界，写死）：
// - profile：hub 选中 CLI 的 profile——cliId = profile.id（ipc 实参唯一来源），
//   重启提示文案 = profile.capabilities.hooks.restartHint
// - onDirtyChange：dirty 变化上报（hub 切换 CLI 的 dirty 守卫数据源）
// - askGuardRef：hub 切换确认弹窗守卫 ref——弹窗打开/关闭伴随 visibilitychange 回归触发，
//   守卫期间不重读（照 useHooksConfig askGuard 先例，防循环复用，MC-505）

import React, { useCallback, useEffect, useRef, useState } from "react";
import { useHooksConfig } from "./useHooksConfig";
import JsonMode from "./JsonMode";
import GuiMode from "./GuiMode";
import {
  inject,
  uninstall,
  getInjectionStatus,
} from "../../ipc/agentHooks";
import type { AgentHookInjectionStatus } from "../../types/agent";
import type { HooksConfigGui as ConfigGui } from "./configModel";
import type { HooksConfigJson, HooksLayer } from "../../types/hooksConfig";
import type { CodingCliProfile } from "../../features/cliProfiles";
import {
  PANEL_BG,
  ERROR_FG,
  HTML_PANEL_LOADING_FG,
  INPUT_BORDER,
  SIDEBAR_FG,
} from "../../theme";

/** claude hooks 配置编辑器 props */
export interface ClaudeHooksConfigEditorProps {
  /** 目标 CLI profile（hub 选中态）——cliId = profile.id，ipc 实参唯一来源 */
  profile: CodingCliProfile;
  /** dirty 变化上报（hub 切换 CLI 的 dirty 守卫用；undefined = 不上报） */
  onDirtyChange?: (dirty: boolean) => void;
  /** hub 切换确认弹窗守卫 ref——守卫期间 visibilitychange 回归触发不重读（防循环） */
  askGuardRef?: React.MutableRefObject<boolean>;
}

/** 优先级标注文案（claude 领地内硬编码——层集合已迁 profile.configLayers（KZ-4），
    local>project>user 优先级语义是 claude 知识，不随层声明泛化） */
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

/** 编辑器根容器样式 */
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
    borderRadius: 6, // GL-03：层切换按钮 3→6（按钮档）
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
function injectionStatusText(status: AgentHookInjectionStatus | null): string {
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
function injectionStatusColor(status: AgentHookInjectionStatus | null): string {
  if (status === null || status.status === "notInjected") return HTML_PANEL_LOADING_FG;
  if (status.status === "outdated") return ERROR_FG;
  return SIDEBAR_FG;
}

const ClaudeHooksConfigEditor: React.FC<ClaudeHooksConfigEditorProps> = ({
  profile,
  onDirtyChange,
  askGuardRef,
}) => {
  // hub 选中态：cliId = profile.id（ipc 实参唯一来源，Stage 03 临时代理常量已回收）
  const cliId = profile.id;
  // 重启提示文案 = profile 能力域驱动（claude 值同现状文案，MC-506/222）
  const restartHint = profile.capabilities?.hooks?.restartHint ?? "";
  // 层集合数据源 = profile 声明（KZ-4：LAYERS 常量已退役，值迁入 claude profile
  // configLayers；缺失 → 空渲染防御，不崩溃）
  const layers = profile.capabilities?.hooks?.configLayers ?? [];
  // FE-14 收窄守卫：HooksLayer 联合仅 user/project/local（后端 parse_layer 只认这三值）——
  // profile 声明的层 id 落在联合内才放行进 API；未来 CLI 加层时再泛化 HooksLayer
  const isHooksLayer = (id: string | undefined): id is HooksLayer =>
    id === "user" || id === "project" || id === "local";
  // 初始层 = configLayers[0].id（KZ-4）；不在联合内 → 缺省回退 useHooksConfig 的 "user"
  const initialLayer = layers[0] && isHooksLayer(layers[0].id) ? layers[0].id : undefined;

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
  } = useHooksConfig(cliId, initialLayer);
  // JsonMode 校验上报：非法 JSON / schema 违规 → 禁用保存 + 禁用切 GUI + 工具栏错误提示（P3-FE-16）
  const [jsonValid, setJsonValid] = useState(true);
  const [jsonError, setJsonError] = useState<string | null>(null);
  // 模式切换（GUI | JSON），默认 JSON；JSON 非法时 GUI 按钮禁用（P3-FE-16）
  const [mode, setMode] = useState<"gui" | "json">("json");

  /** dirty 上报（hub 切换 CLI 的 dirty 守卫数据源，MC-505） */
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

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

  /** 面板根容器 ref——visibilitychange 重读的可见性判断（面板不可见时不重读） */
  const containerRef = useRef<HTMLDivElement>(null);

  /** visibilitychange 轻量重读——外部修改检测，dirty 时 confirmDialog 确认（useHooksConfig 内部处理）。
      仅页面重新可见时（document.visibilityState === "visible"，如最小化恢复 / Alt+Tab
      切回）重读；窗口移动/缩放边框全程可见，不触发 visibilitychange——不误弹
      （window focus 方案下拖动窗口标题框致 WebView2 焦点暂失再回归会误触发）。
      页面内焦点转移（select 下拉/元素点击）也不触发 visibilitychange。
      确认弹窗打开/关闭的回归触发由 useHooksConfig 的 askGuard 抑制（防循环）。
      面板不可见（Dockview 页面 display:none 显隐，或 loading/error 态无容器）时跳过。
      hub 切换确认弹窗守卫（MC-505）：hub 的 askGuardRef 置位期间（弹窗打开 + 关闭后
      短暂窗口）回归触发的重读跳过——防 hub 弹窗关闭后编辑器再次弹确认框（循环复用） */
  const handleVisibilityChange = useCallback(() => {
    if (askGuardRef?.current) return;
    if (document.visibilityState !== "visible") return;
    if (!containerRef.current) return;
    let el: HTMLElement | null = containerRef.current;
    while (el) {
      if (getComputedStyle(el).display === "none") return;
      el = el.parentElement;
    }
    void reload();
  }, [reload, askGuardRef]);

  // visibilitychange 监听（cleanup 移除）
  useEffect(() => {
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, [handleVisibilityChange]);

  // F2 注入状态（P3-FE-22）：挂载查询一次 + 注入/卸载后刷新；null = 查询中/未查询
  const [injectionStatus, setInjectionStatus] = useState<AgentHookInjectionStatus | null>(null);
  // 注入/卸载操作进行中（防重复点击）
  const [injectionBusy, setInjectionBusy] = useState(false);
  // 注入/卸载失败提示（如 ~/.claude/settings.json 为非法 JSON 被后端拒绝）
  const [injectionError, setInjectionError] = useState<string | null>(null);

  /** 刷新注入状态（挂载 / 注入 / 卸载后调用，数据源 = 选中态 cliId，MC-506）；
      查询失败 console.warn 降级，状态条保持上次值 */
  const refreshInjectionStatus = useCallback(async () => {
    try {
      setInjectionStatus(await getInjectionStatus(cliId));
    } catch (err) {
      console.warn("[slTerminal] 查询 hooks 注入状态失败:", err);
    }
  }, [cliId]);

  // 挂载时查询一次注入状态
  useEffect(() => {
    void refreshInjectionStatus();
  }, [refreshInjectionStatus]);

  /** 注入/卸载完成后自动重读 user 层配置（操作改写 ~/.claude/settings.json，C13-8）：
      当前层为 user → reload；非 user → 切到 user 层（dirty 守卫由 useHooksConfig 内部
      confirmDialog 处理，用户拒绝丢弃则不覆盖——与既有切层/重载语义一致） */
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
      setInjectionStatus(await inject(cliId));
      reloadUserConfig();
    } catch (err) {
      console.error("[slTerminal] hooks 注入失败:", err);
      setInjectionError("注入失败，请检查 ~/.claude/settings.json");
    } finally {
      setInjectionBusy(false);
    }
  }, [cliId, reloadUserConfig]);

  /** 卸载：成功后重新查询状态（uninstall 返回 void）+ 重读 user 层配置；失败显示错误提示 */
  const handleUninstall = useCallback(async () => {
    setInjectionBusy(true);
    setInjectionError(null);
    try {
      await uninstall(cliId);
      await refreshInjectionStatus();
      reloadUserConfig();
    } catch (err) {
      console.error("[slTerminal] hooks 卸载失败:", err);
      setInjectionError("卸载失败，请检查 ~/.claude/settings.json");
    } finally {
      setInjectionBusy(false);
    }
  }, [cliId, refreshInjectionStatus, reloadUserConfig]);

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
    <div ref={containerRef} style={containerStyle}>
      {/* 顶部工具栏 */}
      <div style={toolbarStyle}>
        {/* 层级切换器（数据源 = profile.configLayers——KZ-4；rootPath 为空时
            project/local 禁用判定是 claude 语义，保留编辑器内部） */}
        {layers.map((l) => (
          <button
            key={l.id}
            type="button"
            title={l.hint}
            data-e2e={`hooks-layer-${l.id}`}
            disabled={l.id !== "user" && !rootPath}
            onClick={() => {
              // FE-14 收窄：联合外的层 id 不进入 API（后端会拒绝，静默忽略）
              if (isHooksLayer(l.id)) setLayer(l.id);
            }}
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
        {/* JSON 非法错误提示（P3-FE-16）——显示首条诊断；恢复合法后隐藏。
            单行截断（nowrap + ellipsis + maxWidth）：长诊断消息不换行撑高工具栏
            挤压编辑器（验收 1.2）；完整消息挂 title 悬浮 */}
        {!jsonValid && (
          <span
            style={{
              ...hintStyle,
              color: ERROR_FG,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
              minWidth: 0,
              maxWidth: 240,
            }}
            title={jsonError ?? "配置不符合 schema"}
            data-e2e="hooks-json-error"
          >
            JSON 存在错误，无法切换 GUI：{jsonError ?? "配置不符合 schema"}
          </span>
        )}
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
        {/* 保存成功提示条（P3-FE-17）——文案 = profile.restartHint 驱动（MC-506/222）；
            下次编辑/重载后隐藏 */}
        {saved && (
          <span style={hintStyle} data-e2e="hooks-restart-hint">
            {restartHint}
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

export default ClaudeHooksConfigEditor;
