// AgentBasicPage — 设置中心 Agent 组「基础配置」页（ADR-0023，F3）
//
// 两配置节（SettingsSection 共享档）：
// 1. 侧栏展示内容：「Agent 全局文件」视图展示配置——radio 全部/自定义 + 自定义名单
//    chips（× 删除）+ 添加行（validateCustomName 行内红字）+ 「显示运行时文件」开关
//    （custom 模式下禁用置灰、值保留——开关仅 all 模式生效）。
// 2. CLI 别名：CliAliasSection 单 cliId 形态（自原 CliAliasesPage 分区抽取）。
//
// 全部操作即时生效（无 dirty 暂存）——store 直写 + 2s debounce 落盘；
// 侧栏视图经 store 订阅即时联动（rootFilter 引用变化 → refreshExpanded）。

import React, { useCallback, useMemo, useState } from "react";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";
import { useAgentGlobalFiles } from "../../../stores/agentGlobalFiles";
import { cliProfileRegistry } from "../../../features/cliProfiles/cliProfileRegistry";
import {
  defaultConfig,
  validateCustomName,
} from "../../../features/agentFiles/filtering";
import SettingsSection from "./SettingsSection";
import CliAliasSection from "./CliAliasSection";
import {
  PANEL_BG,
  SIDEBAR_FG,
  DIM_FG,
  PLACEHOLDER_FG,
  INPUT_BG,
  INPUT_BORDER,
  ERROR_FG,
  SEPARATOR_BG,
} from "../../../theme";

/** chip 底样式（与 CliAliasSection CHIP_BASE 同档） */
const CHIP_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  padding: "2px 8px",
  margin: "2px 8px 2px 0",
  fontSize: 12,
  background: INPUT_BG,
  border: `1px solid ${SEPARATOR_BG}`,
  borderRadius: 4,
};

const radioLabelStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  fontSize: 13,
  color: SIDEBAR_FG,
  cursor: "pointer",
  marginBottom: 6,
};

const AgentBasicPage: React.FC<SettingsPageProps & { cliId: string }> = ({
  cliId,
}) => {
  const cfgRaw = useAgentGlobalFiles((s) => s.configs[cliId]);
  // useMemo 稳定缺省引用（defaultConfig() 每次新对象——直用会令消费方 effect 抖动）
  const config = useMemo(() => cfgRaw ?? defaultConfig(), [cfgRaw]);
  const setMode = useAgentGlobalFiles((s) => s.setMode);
  const addCustomName = useAgentGlobalFiles((s) => s.addCustomName);
  const removeCustomName = useAgentGlobalFiles((s) => s.removeCustomName);
  const setShowRuntimeFiles = useAgentGlobalFiles((s) => s.setShowRuntimeFiles);

  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);

  const profile = cliProfileRegistry.get(cliId);
  const runtimePaths = profile?.capabilities.globalFiles?.runtimePaths ?? [];

  const isCustom = config.mode === "custom";

  /** 添加名单项：校验失败行内红字保留输入；成功清空 */
  const handleAdd = useCallback(() => {
    const message = validateCustomName(input, config.customNames);
    if (message !== null) {
      setError(message);
      return;
    }
    addCustomName(cliId, input);
    setInput("");
    setError(null);
  }, [input, config.customNames, cliId, addCustomName]);

  return (
    <div
      style={{ width: "100%", height: "100%", background: PANEL_BG, overflowY: "auto" }}
      data-e2e={`agent-basic-page-${cliId}`}
    >
      <div style={{ padding: "16px 20px" }}>
        <SettingsSection title="侧栏展示内容" testId={`agent-display-${cliId}`}>
          {/* 模式 radio：全部 / 自定义 */}
          <label style={radioLabelStyle} data-e2e={`agent-basic-${cliId}-mode-all`}>
            <input
              type="radio"
              checked={!isCustom}
              onChange={() => setMode(cliId, "all")}
            />
            全部
          </label>
          <label style={radioLabelStyle} data-e2e={`agent-basic-${cliId}-mode-custom`}>
            <input
              type="radio"
              checked={isCustom}
              onChange={() => setMode(cliId, "custom")}
            />
            自定义
          </label>

          {/* 自定义名单（custom 模式展开） */}
          {isCustom && (
            <div style={{ margin: "6px 0" }}>
              <div style={{ marginBottom: 6 }}>
                {config.customNames.length === 0 && (
                  <span style={{ fontSize: 12, color: PLACEHOLDER_FG }}>
                    名单为空——视图中不展示任何条目
                  </span>
                )}
                {config.customNames.map((name) => (
                  <span
                    key={name}
                    data-e2e={`agent-basic-${cliId}-chip-${name}`}
                    style={{ ...CHIP_BASE, color: SIDEBAR_FG }}
                  >
                    {name}
                    <button
                      data-e2e={`agent-basic-${cliId}-remove-${name}`}
                      onClick={() => removeCustomName(cliId, name)}
                      title={`移除 ${name}`}
                      style={{
                        background: "none",
                        border: "none",
                        padding: 0,
                        fontSize: 12,
                        lineHeight: 1,
                        color: DIM_FG,
                        cursor: "pointer",
                      }}
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <input
                  type="text"
                  value={input}
                  data-e2e={`agent-basic-${cliId}-input`}
                  onChange={(e) => {
                    setInput(e.target.value);
                    setError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleAdd();
                  }}
                  placeholder="输入文件/目录名"
                  style={{
                    width: 160,
                    padding: "4px 8px",
                    fontSize: 13,
                    color: SIDEBAR_FG,
                    background: INPUT_BG,
                    border: `1px solid ${INPUT_BORDER}`,
                    borderRadius: 4,
                    outline: "none",
                  }}
                />
                <button
                  data-e2e={`agent-basic-${cliId}-add`}
                  onClick={handleAdd}
                  style={{
                    padding: "4px 10px",
                    fontSize: 12,
                    color: SIDEBAR_FG,
                    background: INPUT_BG,
                    border: `1px solid ${SEPARATOR_BG}`,
                    borderRadius: 4,
                    cursor: "pointer",
                  }}
                >
                  添加
                </button>
              </div>
              {error !== null && (
                <div
                  data-e2e={`agent-basic-${cliId}-error`}
                  style={{ marginTop: 6, fontSize: 12, color: ERROR_FG }}
                >
                  {error}
                </div>
              )}
            </div>
          )}

          {/* 「显示运行时文件」开关（custom 模式禁用置灰、值保留——仅 all 模式生效） */}
          <label
            style={{
              ...radioLabelStyle,
              marginTop: 4,
              marginBottom: 0,
              cursor: isCustom ? "not-allowed" : "pointer",
              opacity: isCustom ? 0.5 : 1,
            }}
            data-e2e={`agent-basic-${cliId}-runtime`}
          >
            <input
              type="checkbox"
              checked={config.showRuntimeFiles}
              disabled={isCustom}
              onChange={(e) => setShowRuntimeFiles(cliId, e.target.checked)}
            />
            显示运行时文件
          </label>
          {!isCustom && runtimePaths.length > 0 && (
            <div style={{ fontSize: 11, color: DIM_FG, marginTop: 4 }}>
              运行时文件：{runtimePaths.join("、")}
            </div>
          )}
        </SettingsSection>

        <SettingsSection title="CLI 别名" testId={`agent-alias-${cliId}`}>
          <CliAliasSection cliId={cliId} />
        </SettingsSection>
      </div>
    </div>
  );
};

export default AgentBasicPage;
