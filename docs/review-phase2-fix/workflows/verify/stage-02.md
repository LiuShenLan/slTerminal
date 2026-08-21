# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read/命令实跑逐条核实，给出证据（命令输出）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-06**：`npm ls @tauri-apps/plugin-dialog` 输出 2.7.2 且单实例（verify agent 实跑取数）
- **TE-07**：`npm ls typescript` 单版本 7.x，输出中无 `@typescript/native`、无 `typescript6` 残留（verify agent 实跑取数）
- **TE-07**：grep `package.json` 无 `@typescript/native`；`"typescript"` 字段值为 `^7.0.2`（Read 确认）
- **TE-07**：若执行期发生 eslint 兼容妥协（升级 typescript-eslint 或 overrides），`package.json` 中可见对应改动且 S02 执行报告/进度表留有结论记录（无妥协则本条自动通过）
- **TE-14**：`npm ls @wdio/globals expect-webdriverio webdriverio` 三包各单版本（verify agent 实跑取数）
- **TE-14**：`npm run build:e2e` 退出码 0（verify agent 实跑取数；全量测试的 e2eBuild 为 `npx tauri build --debug --no-bundle`，两者其一通过即可，以 build:e2e 为准）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx tauri build --debug --no-bundle`
