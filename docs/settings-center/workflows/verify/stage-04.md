# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SC-FE-09a**：`src/features/shortcuts/ShortcutRegistry.ts` 存在 `setCaptureSuspended` 与 `getEffectiveKeystroke` 两公共 API（grep 双命中）
- **SC-FE-09b**：捕获态短路（语义式：Read ShortcutRegistry.ts 确认 handleKeyDown 与 resolve 起始均有 `if (this.captureSuspended) return false`；既有 resolve/effectiveKeystroke 语义零变更——diff 仅新增）
- **SC-FE-09c**：`getEffectiveKeystroke` 实现正确（语义式：Read 确认 commands.get 缺 id 返回 null、effectiveKeystroke 结果经 formatKeystroke、null 透传——显示与运行期同源，不自行另算）
- **SC-FE-09d**：`src/panels/settings/pages/KeybindingsPage.tsx` 存在；`findConflict` 为页内导出纯函数（grep 命中 export）；语义式 Read 确认录制态分支齐全：isComposing 跳过 / Escape 取消 / Backspace|Delete 解绑 / 纯修饰键忽略 / isReserved 行内红字拒绝 / findConflict 警告放行 / 合法 setBinding 写入 / 录制开始 setCaptureSuspended(true) 且结束/取消/卸载三路复位 false（卸载兜底缺失判 partial）
- **SC-FE-09e**：`src/features/settingsCenter/pages.ts` 含 keybindings 注册 `{ id: "keybindings", title: "快捷键", group: "global", order: 10 }`（Read 确认）
- **SC-FE-09f**：测试绿（测试 agent 产出判定）：settings-keybindings.test.tsx 全例；shortcuts.test.ts 新增 2 例（suspended 不消费 / resolve 返回 false）；command-catalog.test.ts 无增删（语义式：grep 该文件命令目录计数断言字面量仍为 9 条——npm test 绿只能证明断言自洽，计数断言数值须 Read 核对未随本 Stage 漂移）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
