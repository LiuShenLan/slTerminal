// HooksSettingsPage — hooks 配置页（设置中心 hub 容器，F11，SC-FE-05）
//
// HooksConfigPanel（hooks 双模式配置面板 hub）迁移改造为设置中心配置页：
// - CLI 选择行 + 编辑器槽分派逻辑照搬原 hub（MC-502~507）：遍历 cliProfileRegistry
//   getAll() 过滤 capabilities.hooks?.hasConfigEditor===true 渲染按钮（iconSrc 16×16
//   logo + displayName）；选中态背景高亮走 theme token（硬约束 #6）；点击切换下方编辑器；
//   单 CLI 也渲染选择行（边界 1，防布局跳动）。
// - 编辑器槽（KZ-1）：经选中 CLI 的 capabilities.hooks.configEditor 分派渲染，hub 不
//   直接引用任何具体 CLI 编辑器；key={cliId} 强制卸载重挂载（ADR-0001 先例——dirty/
//   选中态丢弃）；dirty 守卫：dirty 时切换需 confirmDialog 确认丢弃（askGuard 防循环
//   复用，照原 hub :139-152/:174-209 先例）；configEditor 缺失（声明不一致）→ 空态占位。
// - props 改 SettingsPageProps（壳单点持久化）：selectedCli 读 pageParams?.selectedCli、
//   写经 onPageParamsChange({ selectedCli })——不再自持 updateParameters/saveLayout/
//   handleLayoutPersist（壳是 params 持久化单点，SC-FE-03）；编辑器 dirty 经 onDirtyChange
//   直传壳（导航圆点 + 切页守卫数据源），hub 自身 dirtyRef 保留（切 CLI 守卫）。
// - 空态（MC-507）：无任何 hasConfigEditor profile → 「无可配置 CLI」占位。
// - 根容器保留 data-e2e="hooks-config-panel"（选择器语义继承，最小化 E2E 适配面）。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useCallback, useMemo, useRef, useState } from "react";
import { cliProfileRegistry } from "../../../features/cliProfiles";
import { confirmDialog } from "../../../lib";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";
import {
  PANEL_BG,
  EXPLORER_SELECTION_BG,
  HTML_PANEL_LOADING_FG,
  INPUT_BORDER,
  SIDEBAR_FG,
} from "../../../theme";

/** confirmDialog 弹窗关闭后守卫窗口（ms）——期间内的回归触发的重读被抑制（防循环，照 useHooksConfig 同常量） */
const ASK_GUARD_MS = 500;

/** 页根容器样式 */
const containerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  background: PANEL_BG,
  display: "flex",
  flexDirection: "column",
};

/** CLI 选择行样式 */
const selectorRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 10px",
  borderBottom: `1px solid ${INPUT_BORDER}`,
};

/** CLI 选择按钮样式（active = 选中态背景高亮，硬约束 #6 token） */
function cliButtonStyle(active: boolean): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "3px 10px",
    fontSize: 12,
    cursor: "pointer",
    background: active ? EXPLORER_SELECTION_BG : "transparent",
    color: SIDEBAR_FG,
    border: `1px solid ${INPUT_BORDER}`,
    borderRadius: 6, // GL-03：CLI 选择行按钮 3→6（按钮档）
  };
}

/** 编辑器槽样式（flex 撑满，minHeight 0 防内容撑开） */
const editorSlotStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  minHeight: 0,
};

const HooksSettingsPage: React.FC<SettingsPageProps> = ({
  onDirtyChange,
  pageParams,
  onPageParamsChange,
}) => {
  /** 有能力 CLI profile 列表（选择行数据源，MC-502）——注册表模块级单例，挂载期稳定 */
  const eligibleProfiles = useMemo(
    () =>
      cliProfileRegistry
        .getAll()
        .filter((p) => p.capabilities?.hooks?.hasConfigEditor === true),
    [],
  );

  // 选中态（MC-503 语义迁入壳单点）：读 pageParams.selectedCli 恢复；缺省/失效回退
  // 首个有能力 CLI；写经 onPageParamsChange({ selectedCli })（壳持久化，不再自持）
  const [selectedCliId, setSelectedCliId] = useState<string | null>(() => {
    const saved = pageParams?.selectedCli;
    if (saved && eligibleProfiles.some((p) => p.id === saved)) return saved as string;
    return eligibleProfiles[0]?.id ?? null;
  });

  /** 当前选中 profile（编辑器渲染与 restartHint 数据源）；无 → null（空态） */
  const selectedProfile = useMemo(
    () => eligibleProfiles.find((p) => p.id === selectedCliId) ?? null,
    [eligibleProfiles, selectedCliId],
  );

  /** 编辑器槽组件（KZ-1）：选中 CLI 的 capabilities.hooks.configEditor（分派数据源）；
      hasConfigEditor=true 但 configEditor 缺失 → null → 编辑器槽空态占位防御 */
  const Editor = selectedProfile?.capabilities?.hooks?.configEditor ?? null;

  // ref 镜像：异步回调闭包内读取最新值
  const selectedCliRef = useRef(selectedCliId);
  selectedCliRef.current = selectedCliId;
  // 当前编辑器 dirty（编辑器组件 onDirtyChange 上报，切 CLI 守卫数据源）
  const dirtyRef = useRef(false);
  // 切换确认弹窗守卫：弹窗打开期间 + 关闭后短暂窗口内抑制编辑器 visibilitychange 回归
  // 触发重读——弹窗开/关伴随回归触发，无守卫将再弹编辑器自己的确认弹窗（循环复用，
  // 照 useHooksConfig askGuard 先例，MC-505）
  const askGuardRef = useRef(false);

  /** 编辑器 dirty 上报：hub 自身守卫（dirtyRef）+ 直传壳（导航圆点/切页守卫，SC-FE-07） */
  const handleDirtyChange = useCallback(
    (dirty: boolean) => {
      dirtyRef.current = dirty;
      onDirtyChange?.(dirty);
    },
    [onDirtyChange],
  );

  /** 切换 CLI（MC-505 语义保留）：dirty 时 confirmDialog 确认丢弃；确认后切换 = 卸载
      当前编辑器并重挂载目标编辑器（key={cliId} 强制重建，ADR-0001——dirty/选中态丢弃）
      + 选中态经 onPageParamsChange 交壳持久化 */
  const handleCliSelect = useCallback(
    (cliId: string) => {
      if (cliId === selectedCliRef.current) return;
      void (async () => {
        // dirty 守卫：有未保存修改时 confirmDialog 确认；弹窗打开前置 askGuardRef
        // （弹窗开/关伴随 visibilitychange 回归触发，无守卫将再弹窗——防循环复用）
        if (dirtyRef.current) {
          askGuardRef.current = true;
          let ok: boolean;
          try {
            ok = await confirmDialog({
              title: "未保存的修改",
              message: "当前 CLI 有未保存的修改，切换将丢弃这些修改。",
              kind: "warning",
            });
          } finally {
            setTimeout(() => {
              askGuardRef.current = false;
            }, ASK_GUARD_MS);
          }
          if (!ok) return;
          // 确认丢弃：新编辑器挂载前清除旧 dirty（防切换后脏 ref 导致再次误弹）；
          // 同步清壳 dirty（新编辑器初始不 dirty 可能不上报，防圆点/切页守卫残留）
          dirtyRef.current = false;
          onDirtyChange?.(false);
        }
        setSelectedCliId(cliId);
        onPageParamsChange?.({ selectedCli: cliId });
      })();
    },
    [onDirtyChange, onPageParamsChange],
  );

  // 空态（MC-507）：无任何 hasConfigEditor profile → 占位，不渲染编辑器（防御分支）
  if (eligibleProfiles.length === 0) {
    return (
      <div style={containerStyle} data-e2e="hooks-config-panel">
        <div
          style={{
            width: "100%",
            height: "100%",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }} data-e2e="hooks-cli-empty">
            无可配置 CLI
          </span>
        </div>
      </div>
    );
  }

  return (
    <div style={containerStyle} data-e2e="hooks-config-panel">
      {/* CLI 选择行（MC-502）：按钮 = iconSrc 16×16 logo + displayName；选中态背景高亮走
          theme token（硬约束 #6）；单 CLI 也渲染（边界 1，防布局跳动） */}
      <div style={selectorRowStyle}>
        {eligibleProfiles.map((p) => (
          <button
            key={p.id}
            type="button"
            title={p.displayName}
            data-e2e={`hooks-cli-${p.id}`}
            onClick={() => handleCliSelect(p.id)}
            style={cliButtonStyle(selectedCliId === p.id)}
          >
            <img
              src={p.iconSrc}
              width={16}
              height={16}
              style={{ flexShrink: 0, display: "block" }}
              alt=""
            />
            <span>{p.displayName}</span>
          </button>
        ))}
      </div>
      {/* 编辑器槽（MC-504/505，KZ-1）：经 selectedProfile.capabilities.hooks.configEditor 分派
          渲染（hub 不直接引用任何具体 CLI 编辑器）；key={cliId} 强制卸载重挂载（ADR-0001）；
          dirty 守卫在切换入口；configEditor 缺失（声明不一致）→ 空态占位防御 */}
      <div style={editorSlotStyle}>
        {selectedProfile &&
          (Editor ? (
            <Editor
              key={selectedProfile.id}
              profile={selectedProfile}
              onDirtyChange={handleDirtyChange}
              askGuardRef={askGuardRef}
            />
          ) : (
            <div
              style={{
                width: "100%",
                height: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
              data-e2e="hooks-editor-empty"
            >
              <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>
                该 CLI 未提供配置编辑器
              </span>
            </div>
          ))}
      </div>
    </div>
  );
};

export default HooksSettingsPage;
