// SettingsSection.tsx — 设置中心「配置节」共享组件（ADR-0023）
//
// 节 = 标题 + 内容块的统一视觉档（样式照 KeybindingsPage 分组档抽取：
// 标题 12px DIM_FG + marginBottom 6，节容器 marginBottom 16）。
// 消费方：KeybindingsPage 分组、AgentBasicPage 两配置节等页内分节场景。

import React from "react";
import { DIM_FG } from "../../../theme";

const sectionStyle: React.CSSProperties = {
  marginBottom: 16,
};

const titleStyle: React.CSSProperties = {
  fontSize: 12,
  color: DIM_FG,
  marginBottom: 6,
  userSelect: "none",
};

const SettingsSection: React.FC<{
  title: string;
  /** 节标题 data-e2e 完整值（调用方自带语义前缀——如 `kb-group-<cat>`、`agent-display-<cliId>`）；
   *  不传则无 e2e 标记 */
  testId?: string;
  children: React.ReactNode;
}> = ({ title, testId, children }) => {
  return (
    <div style={sectionStyle}>
      <div
        style={titleStyle}
        {...(testId ? { "data-e2e": testId } : {})}
      >
        {title}
      </div>
      {children}
    </div>
  );
};

export default SettingsSection;
