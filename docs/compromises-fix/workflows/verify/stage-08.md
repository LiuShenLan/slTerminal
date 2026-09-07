# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：compute_conpty_flags 禁区仅对 CP-009 解除；其余项不得触碰 ConPTY flags。

## 断言清单

- **CP-020**：`grep -n "terminal.interrupt" src/features/shortcuts/commandCatalog.ts src/panels/terminal/keyboard.ts` 各 ≥1；`grep -n 'commandFromMeta("terminal.interrupt"' src/panels/terminal/keyboard.ts` 命中且紧邻 `return false;`（Read 确认透传语义）；`src/lib/agentStatus.ts` 已知行为注释为 CP-020 口径（Read 确认）；`npx vitest run command-catalog terminal-shortcuts terminal-interrupt` 绿（随全量覆盖）。**人工/L4 可观测**：claude 运行中 Ctrl+C → 页签绿转黄且 PTY 收到 \x03。
- **CP-018**：`grep -n "isSwiftShaderRenderer" src/panels/terminal/webgl.ts` ≥2；`grep -n "failIfMajorPerformanceCaveat" src/panels/terminal/webgl.ts` 仅注释命中（Read 确认未改检测契约）；toast 一次性（swiftShaderNotified 旗标，Read 确认）；`npx vitest run detect-webgl webgl-setup` 绿（随全量覆盖）。
- **CP-039**：`grep -rn "import.*editorTheme[^S]" src/ --include=*.ts --include=*.tsx | grep -v getEditorTheme | grep -v editorThemeSlot` 零命中；`grep -n "onDidChange" src/theme/schemeRegistry.ts` ≥1；`src/theme/editorThemeSlot.ts` 存在且 DiffPanel 双栏双槽（Read 确认 left/right 各一 Compartment）；`npx vitest run theme-overrides theme-scheme-registry use-code-mirror diff-panel gitshow-panel` 绿（随全量覆盖）。
- **CP-002**：`grep -rn "codemirror-json-schema" src/ package.json vitest.config.ts vitest.l3.config.ts` 零命中（docs/.claude 历史登记不计）；`grep -rn 'from "json-schema"' src/` 零命中；`npm ls json-schema-library` 仅 11.x 单实例；vitest 双配置 `server.deps.inline` 块已删（Read 确认）；`src/features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm.ts` 存在（pointerToRange/pathAt/resolveSchemaPath/hooksSchemaLinter/hooksSchemaHover 五导出，Read 确认）；`npx vitest run hooks-json-schema-cm hooks-config-jsonmode` 绿（随全量覆盖）；`npx knip --production` 退出码 0。
- **CP-009**：`cargo test conpty -- --test-threads=1` 绿（含 `conpty_flags_default_matrix_matches_legacy_tristate` 三态等价用例，随全量覆盖）；`SETTINGS_ALLOWED_KEYS` 为七键含 `conptyInputModes`（Read 确认）；settingsCenter 新增「终端输入模式」页（四开关 + passthrough 警示文案，Read 确认）；`rg "永不启用 0x8" src-tauri/src/pty/CLAUDE.md` 零命中（红线已改写为默认矩阵口径）；adr.md ADR-0007 后果节已追加 CP-009 口径；test-exemptions.md 新增人工门禁登记行。**人工验证点**：默认矩阵零漂移（本项无 0x8 默认值变更时人工实测豁免执行，登记于 commit body）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
7. `npx knip --production`
