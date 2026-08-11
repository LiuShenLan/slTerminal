// HooksConfigPanel — hooks 配置面板 hub 容器（Stage 06，MC-501~507）
//
// hub 容器职责：顶部 CLI 选择行 + 编辑器槽。
// - 选择行（MC-502）：遍历 cliProfileRegistry.getAll() 过滤 capabilities.hooks?.hasConfigEditor
//   === true，渲染按钮（iconSrc 16×16 logo + displayName）；选中态背景高亮走 theme token
//   （硬约束 #6）；点击切换下方编辑器；单 CLI 也渲染选择行（边界 1，防布局跳动）。
// - 选中态持久化（MC-503）：params.selectedCli 随布局 JSON 持久化——persistSelectedCli
//   （照 F8 customTitle 先例：api.updateParameters({...params, selectedCli}) + 显式
//   onLayoutChange(saveLayout(containerApi))——updateParameters 不触发 onDidLayoutChange
//   （dockviewPanel.js:84-95），必须显式保存）；挂载时读 params 恢复；
//   缺省/失效回退首个有能力 CLI。
// - 编辑器槽（MC-504/505，KZ-1）：经选中 CLI 的 capabilities.hooks.configEditor 分派渲染
//   （claude = ClaudeHooksConfigEditor，由 claude profile 挂载）——hub 不再直接引用任何
//   具体 CLI 编辑器（新增 CLI 自带编辑器组件即可接入）；key={cliId} 卸载当前编辑器并
//   重挂载目标编辑器（ADR-0001 先例——dirty/选中态丢弃）；dirty 守卫：dirty 时切换需
//   dialog.ask 确认丢弃（照切层/visibilitychange ask 守卫先例，askGuard 防循环复用）；
//   hasConfigEditor=true 但 configEditor 缺失（声明不一致）→ 编辑器槽空态占位防御。
// - 空态（MC-507）：无任何 hasConfigEditor profile → 渲染「无可配置 CLI」占位，不渲染编辑器。
// - 入口零改动（MC-501）：面板 id hooksConfig-{pageId}、侧栏右键菜单流程、pageApis 不动。
// 配色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useCallback, useMemo, useRef, useState } from "react";
import { cliProfileRegistry } from "../../features/cliProfiles";
import { ask } from "../../ipc/dialog";
import { saveLayout } from "../../workspace/layoutSerde";
import { useProjects } from "../../stores/projects";
import {
  PANEL_BG,
  EXPLORER_SELECTION_BG,
  HTML_PANEL_LOADING_FG,
  INPUT_BORDER,
  SIDEBAR_FG,
} from "../../theme";
import type { DockviewPanelApi, DockviewApi } from "dockview-react";

/** HooksConfigPanel 面板参数——单例面板无需 panelId（Stage 08 同页单例），保留 props 兼容 Dockview；
    selectedCli = 选中 CLI id（随布局 JSON 持久化，MC-503） */
interface HooksConfigPanelProps {
  /** Dockview 传入的面板 API */
  api: DockviewPanelApi;
  /** Dockview 传入的容器 API（saveLayout 序列化布局用） */
  containerApi: DockviewApi;
  /** Dockview 传入的面板参数 */
  params?: { panelId?: string; selectedCli?: string };
}

/** ask 弹窗关闭后守卫窗口（ms）——期间内的回归触发的重读被抑制（防循环，照 useHooksConfig 同常量） */
const ASK_GUARD_MS = 500;

/** 面板根容器样式 */
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
    borderRadius: 3,
  };
}

/** 编辑器槽样式（flex 撑满，minHeight 0 防内容撑开） */
const editorSlotStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  minHeight: 0,
};

/**
 * 持久化选中 CLI 到布局 JSON（照 F8 applyRename 先例，导出纯函数供 L2 直测，MC-503）：
 * 1. updateParameters 写入 params.selectedCli（随布局 JSON 持久化的单一真值源）
 * 2. 显式 onLayoutChange(saveLayout(containerApi)) 触发持久化——updateParameters
 *    不触发 onDidLayoutChange（dockviewPanel.js:84-95），必须显式保存
 */
export function persistSelectedCli(
  api: DockviewPanelApi,
  containerApi: DockviewApi,
  params: Record<string, unknown>,
  selectedCli: string,
  onLayoutChange: (layout: Record<string, unknown>) => void,
): void {
  api.updateParameters({ ...params, selectedCli });
  onLayoutChange(saveLayout(containerApi) as Record<string, unknown>);
}

const HooksConfigPanel: React.FC<HooksConfigPanelProps> = ({
  api,
  containerApi,
  params,
}) => {
  /** 有能力 CLI profile 列表（选择行数据源，MC-502）——注册表模块级单例，挂载期稳定 */
  const eligibleProfiles = useMemo(
    () =>
      cliProfileRegistry
        .getAll()
        .filter((p) => p.capabilities?.hooks?.hasConfigEditor === true),
    [],
  );

  // 选中态（MC-503）：挂载时读 params.selectedCli 恢复；缺省/失效回退首个有能力 CLI
  const [selectedCliId, setSelectedCliId] = useState<string | null>(() => {
    const saved = params?.selectedCli;
    if (saved && eligibleProfiles.some((p) => p.id === saved)) return saved;
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
  // 当前编辑器 dirty（编辑器组件 onDirtyChange 上报，MC-505 守卫数据源）
  const dirtyRef = useRef(false);
  // 切换确认弹窗守卫：弹窗打开期间 + 关闭后短暂窗口内抑制编辑器 visibilitychange 回归
  // 触发重读——弹窗开/关伴随回归触发，无守卫将再弹编辑器自己的 ask（循环复用，
  // 照 useHooksConfig askGuard 先例，MC-505）
  const askGuardRef = useRef(false);

  /** 编辑器 dirty 上报（切换守卫读取） */
  const handleDirtyChange = useCallback((dirty: boolean) => {
    dirtyRef.current = dirty;
  }, []);

  /** 显式布局保存终点（照 Workspace.handlePageLayoutChange 模式：经 pageId 查 projId 写 store，
      等价于页面级 onLayoutChange 链）——persistSelectedCli 的 onLayoutChange 实参 */
  const handleLayoutPersist = useCallback(
    (layout: Record<string, unknown>) => {
      const panelId = params?.panelId;
      if (!panelId?.startsWith("hooksConfig-")) return;
      const pageId = panelId.slice("hooksConfig-".length);
      const { projects } = useProjects.getState();
      for (const [projId, proj] of Object.entries(projects)) {
        if (proj.pages.some((p) => p.pageId === pageId)) {
          useProjects.getState().updatePageLayout(projId, pageId, layout);
          break;
        }
      }
    },
    [params?.panelId],
  );

  /** 切换 CLI（MC-505）：dirty 时 ask 确认丢弃；确认后切换 = 卸载当前编辑器并重挂载目标
      编辑器（key={cliId} 强制重建，ADR-0001——dirty/选中态丢弃）+ 持久化选中态 */
  const handleCliSelect = useCallback(
    (cliId: string) => {
      if (cliId === selectedCliRef.current) return;
      void (async () => {
        // dirty 守卫：有未保存修改时 ask 确认；ask 打开前置 askGuardRef
        // （弹窗开/关伴随 visibilitychange 回归触发，无守卫将再弹窗——防循环复用）
        if (dirtyRef.current) {
          askGuardRef.current = true;
          let ok: boolean;
          try {
            ok = await ask("当前 CLI 有未保存的修改，切换将丢弃这些修改。", {
              title: "未保存的修改",
              kind: "warning",
            });
          } finally {
            setTimeout(() => {
              askGuardRef.current = false;
            }, ASK_GUARD_MS);
          }
          if (!ok) return;
          // 确认丢弃：新编辑器挂载前清除旧 dirty（防切换后脏 ref 导致再次误弹）
          dirtyRef.current = false;
        }
        setSelectedCliId(cliId);
        persistSelectedCli(
          api,
          containerApi,
          (params ?? {}) as Record<string, unknown>,
          cliId,
          handleLayoutPersist,
        );
      })();
    },
    [api, containerApi, params, handleLayoutPersist],
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

export default HooksConfigPanel;
