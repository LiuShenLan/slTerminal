# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-02**：`grep -n "SLTERM_DATA_DIR" e2e-tests/run-wdio.cjs` 命中（env 注入在 spawn wdio 之前，Read 确认顺序）；`e2eDataDir`（os.tmpdir() 下 slterm-e2e-data）有 rmSync 清理 + mkdirSync 创建。
- **TE-02**：`grep -n "slterminal-projects" e2e-tests/run-wdio.cjs` 零命中（旧 projects.json/.bak 备份三件套已删）；`grep -n ".slterminal" e2e-tests/run-wdio.cjs` 若仍有命中须 Read 确认与 settings.json 备份无关（失实备份段已删）。
- **TE-03**：`e2e-tests/settings.e2e.ts` 的 settingsJsonPath 经 `process.env.SLTERM_DATA_DIR ??` 推导（grep 命中），硬编码路径仅在 `??` 兜底侧。
- **TE-03**：用例⑧存在切页前后 `panelId` 恒为 `settings-${pageIdA}` 的归属断言（Read 确认，非仅 DOM 计数）。
- **TE-03**：存在用例⑪ × 关闭 dirty 守卫（Read 确认链路：后门置 dirty → 点 `tab-close-settings-` × → confirm-dialog 出现 → confirm-cancel 面板保留 → 再点 → confirm-ok 面板关闭）。
- **TE-03**：`e2e-tests/helpers.ts` 的 `getSettingsPanelState` 返回含 `panelId` 字段；存在 `__slterm_e2e_setSettingsDirty` helper（grep 双命中）。
- **FE-04**：`grep -n "tab-close-" src/workspace/PageDockviewHost.tsx` 命中；`grep -n "hooksConfig" src/workspace/PageDockviewHost.tsx` 零命中。
- **FE-04**：× 按钮 data-e2e 取值来自 `tabParams.panelId`（语义式：不接受与面板无关的字面量，须 Read 确认取值来源）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run build:e2e`
5. settings spec 实跑全绿（`npx wdio run wdio.conf.ts --spec e2e-tests/settings.e2e.ts`，11 用例）
