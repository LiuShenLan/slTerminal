// CliAliasSection — 「CLI 别名」配置节（单 cliId 形态，ADR-0023 自 CliAliasesPage 抽取）
//
// 单 CLI 分区内容：内置命令只读 chips（profile.commands，D3 命名空间占用可视化）+
// 别名 chips（× 删除，无确认——删除可重建）+ 添加行（input + 按钮，Enter/点击提交）。
// 提交（立即提交型，无 dirty 暂存）：
//   校验（aliasValidation.validateCliAlias：语法 + D3 全命名空间唯一）失败 → 行内红字
//   保留 input 文本可改；成功 → store.addAlias 即时生效（App 快照同步 effect 注入注册表，
//   D4）→ 清空 input 与错误。blur 保留草稿与错误（不清空、不提交）——别名是离散添加
//   操作，误 blur 提交比误丢失成本高；且点「添加别名」按钮时 mousedown 先使 input 失焦，
//   blur 若清空则 click 提交必读空串失败（FC-01 教训，配回归测试防复发）。
// data-e2e 保持 `cli-aliases-*` 前缀（选择器语义继承）。

import React, { useCallback, useState } from "react";
import { useCliAliases } from "../../../stores/cliAliases";
import { cliProfileRegistry } from "../../../features/cliProfiles/cliProfileRegistry";
import { validateCliAlias } from "../../../features/cliProfiles/aliasValidation";
import {
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

const CliAliasSection: React.FC<{ cliId: string }> = ({ cliId }) => {
  const aliases = useCliAliases((s) => s.aliases[cliId]);
  const addAlias = useCliAliases((s) => s.addAlias);
  const removeAlias = useCliAliases((s) => s.removeAlias);

  /** 输入文本 + 行内红字错误（null = 无错误） */
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);

  const profile = cliProfileRegistry.get(cliId);
  const aliasList = aliases ?? [];

  /** 提交（Enter/按钮）：校验失败行内红字保留输入；成功 addAlias + 清空输入/错误 */
  const handleSubmit = useCallback(() => {
    const result = validateCliAlias(input, {
      cliId,
      aliasesByCli: useCliAliases.getState().aliases,
      profiles: cliProfileRegistry.getAll(),
    });
    if (!result.ok) {
      setError(result.message);
      return;
    }
    addAlias(cliId, result.alias);
    setInput("");
    setError(null);
  }, [input, cliId, addAlias]);

  // profile 未注册（防御——pages.ts 枚举注册表生成页面，正常不会缺）
  if (!profile) {
    return <span style={{ fontSize: 12, color: PLACEHOLDER_FG }}>未知 CLI</span>;
  }

  return (
    <div data-e2e={`cli-aliases-group-${cliId}`}>
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
          value={input}
          data-e2e={`cli-aliases-input-${cliId}`}
          onChange={(e) => {
            setInput(e.target.value);
            // 编辑即清错——错误随下次提交重新判定
            setError(null);
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
            if (e.key === "Enter") handleSubmit();
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
          onClick={handleSubmit}
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
};

export default CliAliasSection;
