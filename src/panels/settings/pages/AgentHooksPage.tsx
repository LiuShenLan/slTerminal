// AgentHooksPage — 设置中心 Agent 组「Hooks 配置」页（ADR-0023，F4）
//
// 自 HooksSettingsPage 迁入：删 CLI 选择行（导航已按 agent 分节，选择行冗余），
// 直渲染 profile.capabilities.hooks.configEditor（KZ-1 分派语义不变）；
// askGuardRef 本地持有（切页守卫由壳承担，编辑器回归触发重读的防循环守卫随页）；
// dirty 经 onDirtyChange 直传壳（导航圆点 + 切页守卫数据源，SC-FE-07）。
// configEditor 缺失（声明不一致）→ 空态占位防御。

import React, { useRef } from "react";
import { cliProfileRegistry } from "../../../features/cliProfiles/cliProfileRegistry";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";
import { PANEL_BG, HTML_PANEL_LOADING_FG } from "../../../theme";

const AgentHooksPage: React.FC<SettingsPageProps & { cliId: string }> = ({
  cliId,
  onDirtyChange,
}) => {
  // 编辑器回归触发重读的防循环守卫（照 hub 先例，本地持有——无 CLI 切换后
  // 仅服务编辑器自身 visibilitychange 回归场景）
  const askGuardRef = useRef(false);

  const profile = cliProfileRegistry.get(cliId);
  const Editor = profile?.capabilities.hooks?.configEditor ?? null;

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: PANEL_BG,
        display: "flex",
        flexDirection: "column",
      }}
      data-e2e={`agent-hooks-page-${cliId}`}
    >
      {profile && Editor ? (
        <Editor
          profile={profile}
          onDirtyChange={onDirtyChange}
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
      )}
    </div>
  );
};

export default AgentHooksPage;
