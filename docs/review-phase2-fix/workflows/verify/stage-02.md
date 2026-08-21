# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read/命令实跑逐条核实，给出证据（命令输出）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-06**：`npm ls @tauri-apps/plugin-dialog` 输出 2.7.2 且单实例（verify agent 实跑取数）
- **TE-07**：`npx tsc --version` 输出 7.x——编译器实际为 TS7，tsc bin 由 `@typescript/native`（`npm:typescript@7.0.2`）提供（verify agent 实跑取数）
- **TE-07**：`npm ls typescript` 输出无 `invalid` 标记（允许 `npm:@typescript/typescript6@6.0.2` 单实例存在）；`npm ls @typescript/native` 输出 7.0.2 且单实例（verify agent 实跑取数）
- **TE-07**：妥协结论记录存在——execution-plan.md 进度表 S02 行含 commit hash + 妥协摘要，或 `s02-execution-report.md` 存在且含升级触发条件（Read 确认）
- **TE-07 分工说明（非断言）**：ADR 登记由 S10-C 负责（stages.md S10 已含 TE-07 结果登记义务），本 Stage 只留结论不写 ADR——verify agent 不得以「ADR 未登记」判 not_fixed
- **TE-14**：`npm ls @wdio/globals expect-webdriverio webdriverio` 三包各单版本（verify agent 实跑取数）
- **TE-14**：`npm run build:e2e` 退出码 0（verify agent 实跑取数；全量测试的 e2eBuild 为 `npx tauri build --debug --no-bundle`，两者其一通过即可，以 build:e2e 为准）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx tauri build --debug --no-bundle`
