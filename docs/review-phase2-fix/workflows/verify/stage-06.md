# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-37**：grep `src/stores/` 全目录无 `setProjectRoot` 命中（零命中截图/输出为证）
- **FE-37**：`src/stores/projects.ts` 的 `switchToPage` 为纯状态转换（Read 确认：函数体内无 IPC 调用、无 toast，仅 set 状态）
- **FE-36**：`src/stores/projects.ts` 的 `addPage` 上限校验为跨项目全局计数（语义断言：计数覆盖全部项目的 pages 求和，不限变量名——Read 确认非单项目 `project.pages.length`）
- **BE-23**：`src/workspace/pageApis.ts` 的 `switchToPageShared` catch 块含 `toast.show`（Read 确认，warning 级别，文案含「项目根路径设置失败」）
- **FE-36**：`src/__tests__/projects.test.ts` 含跨项目全局计数用例（grep 「全局」或「跨项目」命中；A 15 页 + B 5 页构造，Read 确认）
- **FE-37+FE-36**：`src/stores/CLAUDE.md` 含 FE-37 纯状态转换与 FE-36 全局计数两句登记（grep 命中）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx tauri build --debug --no-bundle`
