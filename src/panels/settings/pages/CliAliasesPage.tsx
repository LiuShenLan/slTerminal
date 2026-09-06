// CliAliasesPage — 设置中心「CLI 别名」配置页（global 组，注册表驱动）
//
// 按 cliProfileRegistry.getAll() 枚举分区（D6：新增 CLI profile 自动出现分区，页零改动）；
// 每分区：
//   - 内置命令只读 chips（profile.commands）——D3 命名空间占用可视化，不可删除；
//   - 别名 chips（× 删除，无确认——删除可重建，从简）；
//   - 添加行（input + 按钮，Enter/点击提交）。
// 提交（立即提交型，无 dirty 暂存，照 BackgroundTasksPage 先例注释）：
//   校验（aliasValidation.validateCliAlias：语法 + D3 全命名空间唯一）失败 → 行内红字
//   保留 input 文本可改；成功 → store.addAlias 即时生效（App 快照同步 effect 注入注册表，
//   D4）→ handleSubmit 清空 input 与错误。blur 保留草稿与错误（不清空、不提交）——别名是
//   离散添加操作，误 blur 提交比误丢失成本高，刻意不走 BackgroundTasksPage 的 blur 提交语义；
//   且点「添加别名」按钮时 mousedown 先使 input 失焦，blur 若清空则 click 提交必读空串失败
//   （FC-01 曾声称修复此竞态，实际从未落地，故 blur 一律保留草稿，配回归测试防复发）。
// 消费方无需本页感知后端——持久化由 store debounce 落盘。

import React, { useCallback, useState } from "react";
import { useCliAliases } from "../../../stores/cliAliases";
import { cliProfileRegistry } from "../../../features/cliProfiles/cliProfileRegistry";
import { validateCliAlias } from "../../../features/cliProfiles/aliasValidation";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";
import {
  PANEL_BG,
  SIDEBAR_FG,
  DIM_FG,
  PLACEHOLDER_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ERROR_FG,
  SEPARATOR_BG,
} from "../../../theme";

/** chip 底样式（内置只读与别名可删共用，颜色经 token） */
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

const CliAliasesPage: React.FC<SettingsPageProps> = () => {
  const aliases = useCliAliases((s) => s.aliases);
  const addAlias = useCliAliases((s) => s.addAlias);
  const removeAlias = useCliAliases((s) => s.removeAlias);

  /** 每分区独立：输入文本 + 行内红字错误（null = 无错误） */
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string | null>>({});

  const profiles = cliProfileRegistry.getAll();

  /** 提交（Enter/按钮）：校验失败行内红字保留输入；成功 addAlias + 清空输入/错误 */
  const handleSubmit = useCallback(
    (cliId: string) => {
      const raw = inputs[cliId] ?? "";
      const result = validateCliAlias(raw, {
        cliId,
        aliasesByCli: useCliAliases.getState().aliases,
        profiles: cliProfileRegistry.getAll(),
      });
      if (!result.ok) {
        setErrors((prev) => ({ ...prev, [cliId]: result.message }));
        return;
      }
      addAlias(cliId, result.alias);
      setInputs((prev) => ({ ...prev, [cliId]: "" }));
      setErrors((prev) => ({ ...prev, [cliId]: null }));
    },
    [inputs, addAlias],
  );

  return (
    <div
      style={{ width: "100%", height: "100%", background: PANEL_BG, overflowY: "auto" }}
      data-e2e="settings-cli-aliases-page"
    >
      <div style={{ padding: "16px 20px" }}>
        {profiles.length === 0 && (
          <div style={{ fontSize: 12, color: PLACEHOLDER_FG }}>无可用 CLI</div>
        )}
        {profiles.map((profile) => {
          const cliId = profile.id;
          const aliasList = aliases[cliId] ?? [];
          const error = errors[cliId] ?? null;
          return (
            <div
              key={cliId}
              data-e2e={`cli-aliases-group-${cliId}`}
              style={{ marginBottom: 20 }}
            >
              {/* 分区头：logo + displayName */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  fontSize: 13,
                  color: SIDEBAR_FG,
                  marginBottom: 8,
                }}
              >
                <img src={profile.iconSrc} width={16} height={16} alt="" />
                <span>{profile.displayName}</span>
              </div>

              {/* 内置命令只读 chips（D3：命名空间占用可视化，不可删除） */}
              <div style={{ marginBottom: 4 }}>
                {profile.commands.map((command) => (
                  <span
                    key={command}
                    data-e2e={`cli-aliases-builtin-${cliId}-${command}`}
                    style={{ ...CHIP_BASE, color: DIM_FG }}
                  >
                    {command}
                  </span>
                ))}
              </div>
              <div style={{ fontSize: 11, color: DIM_FG, marginBottom: 8 }}>
                内置命令已占用命名空间，不可配置为别名
              </div>

              {/* 别名 chips（× 删除） */}
              <div style={{ marginBottom: 8 }}>
                {aliasList.length === 0 && (
                  <span style={{ fontSize: 12, color: PLACEHOLDER_FG }}>暂无别名</span>
                )}
                {aliasList.map((alias) => (
                  <span
                    key={alias}
                    data-e2e={`cli-aliases-alias-${cliId}-${alias}`}
                    style={{ ...CHIP_BASE, color: SIDEBAR_FG }}
                  >
                    {alias}
                    <button
                      data-e2e={`cli-aliases-remove-${cliId}-${alias}`}
                      onClick={() => removeAlias(cliId, alias)}
                      title={`删除别名 ${alias}`}
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

              {/* 添加行 */}
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <input
                  type="text"
                  value={inputs[cliId] ?? ""}
                  data-e2e={`cli-aliases-input-${cliId}`}
                  onChange={(e) => {
                    setInputs((prev) => ({ ...prev, [cliId]: e.target.value }));
                    // 编辑即清错——错误随下次提交重新判定
                    setErrors((prev) => ({ ...prev, [cliId]: null }));
                  }}
                  onFocus={(e) => {
                    e.currentTarget.style.borderColor = FOCUS_BORDER;
                  }}
                  onBlur={(e) => {
                    e.currentTarget.style.borderColor = INPUT_BORDER;
                    // blur 不清空、不提交（别名是离散添加操作，误 blur 提交比误丢失成本高——
                    // 但仍不清空草稿：真实鼠标点「添加别名」时 mousedown 先使 input 失焦，
                    // 清空会让随后的 click 提交读到空串而失败；误点失焦也不丢输入）
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSubmit(cliId);
                  }}
                  placeholder="输入启动命令别名"
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
                  data-e2e={`cli-aliases-add-${cliId}`}
                  onClick={() => handleSubmit(cliId)}
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
                  添加别名
                </button>
              </div>

              {/* 行内红字（校验失败保留 input 文本可改） */}
              {error !== null && (
                <div
                  data-e2e={`cli-aliases-error-${cliId}`}
                  style={{ marginTop: 6, fontSize: 12, color: ERROR_FG }}
                >
                  {error}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default CliAliasesPage;
