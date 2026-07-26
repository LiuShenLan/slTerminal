# ADR-0002：单条 hook 禁用状态存 slTerminal 侧

**状态**：accepted（2026-07-26，grilling 问答 Q13 确认）

## 决策

hooks 配置面板 GUI 表单的单条启用/禁用开关，其禁用状态**存储在 slTerminal 自己的 settings**（`~/.slterminal/settings.json`）中，保存时从用户的 `.claude/settings.json` 里**剔除**被禁用条目——用户配置文件永远是合法的、只含生效 hook 的 JSON。

## 理由

Claude Code 官方只有 `disableAllHooks` 全局开关，**没有**单条 hook 的禁用字段；settings.json 是纯 JSON 不支持注释；写入非标准字段（如 `"disabled": true`）可能被 claude 拒绝或触发 Schema 校验报错。把禁用状态放在 slTerminal 侧，用户文件零污染。

## 否决的备选

| 备选 | 否决理由 |
|------|---------|
| 非标准字段写入 settings.json | 可能被 claude 拒绝或 Schema 校验报错，污染用户文件 |
| 移入自定义键（如 `_disabledHooks`）暂存 | 同样是非标准内容，Schema 校验可能报警 |
| 只做全局 `disableAllHooks` 开关 | 粒度太粗，用户无法单独停用某一条 |

## 后果

- **「我的 hook 去哪了」困惑风险**：用户在 slTerminal 之外用文本编辑器打开 settings.json 时看不到被禁用的条目——GUI 必须在显眼位置提示「禁用条目由 slTerminal 托管，不出现在配置文件中」
- **换工具则禁用状态丢失**：用户卸载 slTerminal 或改用其他编辑器管理配置时，禁用记录不再生效（被禁用条目已在文件中剔除，无法自动恢复）——卸载流程应提示用户
- 禁用列表需按（层级， 事件， matcher, command）四元组标识条目，配置内容被外部修改后标识可能失配——失配条目在 UI 中标记为「失效的禁用记录」而非静默丢弃
