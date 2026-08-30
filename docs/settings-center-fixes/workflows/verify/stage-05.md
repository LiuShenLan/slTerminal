# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 文档类断言须对照当前真实代码核实，防文档撒谎。

## 断言清单

- **DOC-01**：`grep -n "schema 单点保留" docs/settings-center-requirements.md` 零命中；新表述指向 `src/features/cliProfiles/profiles/claude/configEditor/schema/`（grep 命中，且该目录 Glob 真实存在）。
- **DOC-02**：`grep -n "panels/index" docs/settings-center-requirements.md` 零命中。
- **DOC-03**：`src/__tests__/CLAUDE.md` 新增清单逐一命中四文件名：`open-settings.test.ts` / `open-settings-panel.test.ts` / `settings-hooks-page.test.tsx` / `settings-pages-registration.test.ts`。
- **DOC-04**：`grep -n "getSettingsPanelCount" docs/settings-center/report.md` 命中（第 4 项偏离登记）；`grep -n "SLTERM_DATA_DIR" e2e-tests/CLAUDE.md` 命中；e2e-tests/CLAUDE.md 含「target/debug 的 exe 可能是 E2E 构建」类警示与 `__slterm_e2e_setSettingsDirty` helper 说明（Read 确认）。
- **DOC-05**：`grep -n "mount" src/features/settingsCenter/types.ts` 命中（onPageParamsChange mount 期禁止调用约定）。
- **DOC-06**：`grep -n "loadSucceeded" src/stores/CLAUDE.md` 命中；`grep -n "~/.slterminal/settings" src-tauri/src/settings.rs` 零命中；`grep -n "SLTERM_DATA_DIR" src-tauri/src/CLAUDE.md` 命中。
- **DOC-07**：`.claude/test-inventory.md` 表头总数 = L1+L2+L3+L4 四项之和，且与本 Stage 实跑输出一致（实跑取数：cargo test / npm test / npm run test:l3 / npm run e2e 的用例计数行）。
- **转义自查**：本任务全部 md 产物（checklist.md / stages.md / execution-plan.md / verify/*.md，**本文件本条断言的文字表述除外**——其内含该序列作为描述对象）无「反斜杠紧邻反引号」转义残留——用 Grep 工具搜正则「反斜杠后跟反引号」（两字符序列），限定 `docs/settings-center-fixes/**/*.md`，除本条所在行外零命中（.js 脚本内的转义属模板字符串正常纪律，不在此列）。

## 全量测试（全部通过为门禁；本 Stage 含全量四级回归）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
7. `npm run test:l3`
8. `npm run e2e`（全 spec）
