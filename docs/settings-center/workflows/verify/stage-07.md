# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 取数口径：L1/L2/L3 计数取本 Stage 门禁实跑产出；L4 计数沿用 Stage 06 的 npm run e2e 结果（本 Stage 不动代码）。

## 断言清单

- **SC-DOC-01a**：`grep -n "请先创建项目" docs/settings-center-requirements.md` 命中（§4.1/§7 验收分支）；`grep -n "模块级 static" docs/settings-center-requirements.md` 命中（§5.2）
- **SC-DOC-01b**：旧措辞零残留——`grep -n "无项目也可打开" docs/settings-center-requirements.md` 零命中；§4.2 项目组禁用条目已删（Read 确认）；决策记录含 R1–R3 三行（Read 确认）
- **SC-DOC-02a**：`.claude/CLAUDE.md` 需求索引含 F11 行（grep 命中）；模块索引含 `src/features/settingsCenter` 行且不含 `src/features/hooksConfig` 行（Read 模块索引表确认）
- **SC-DOC-02b**：CONTEXT.md 含术语（设置中心/配置页/全局组/项目组/前端消费型/后端消费型——grep 逐词命中）；面板类型列举 hooksConfig→settings（Read :26-27 区域确认）；活动栏配置钮描述改设置中心入口（Read :102 区域确认）
- **SC-DOC-03**：`grep -n "## 0012" .claude/adr.md` 命中；ADR 含上下文/决策/被否决/后果四段（Read 确认；被否决须含模态/独立窗口/侧栏视图/完整后端注册表/inventory 自注册/Ctrl+, 六条）
- **SC-DOC-04a**：`src/features/settingsCenter/CLAUDE.md` 存在且经模板四段（存在理由→约束决策→红线→测试模式；Read 确认含家族契约 #13 与 side-effect import 触发点登记）
- **SC-DOC-04b**：`src/features/hooksConfig/CLAUDE.md` 不存在（Glob 零命中）；schema 单点 MC-223/P3-FE-07/TE-09/TE-15 已并入 `src/features/cliProfiles/CLAUDE.md`（grep 四编号命中）；KZ-1 重写（Read 确认 configEditor/ 归域口径，无「跨 panels 引用合法化」旧口径）
- **SC-DOC-04c**：`src/panels/CLAUDE.md` 含 settings 节（grep 命中）；「添加新面板类型的步骤」节无 `src/panels/index.ts` 引用（grep 零命中——失实修正）；`grep -n "hooksConfig" src/panels/CLAUDE.md src/features/shortcuts/CLAUDE.md src/ipc/CLAUDE.md` 仅历史语境或零命中（逐条人工判断语境）
- **SC-DOC-04d**：shortcuts/CLAUDE.md 含 setCaptureSuspended/getEffectiveKeystroke 登记；ipc/CLAUDE.md planBalance 节四命令；src-tauri/src/CLAUDE.md 白名单聚合口径（前端消费型集中/后端消费型归域先例）；plan_balance/CLAUDE.md 动态间隔+新命令；workspace/CLAUDE.md openSettingsPanel+× 拦截；__tests__/CLAUDE.md 迁移清单（逐文件 grep/Read 确认）
- **SC-DOC-05a**：`.claude/test-inventory.md` 三处计数一致（语义式：Read 表头总数 == L1/L2/L3/L4 各段小计之和；计数来源 = 本 Stage 门禁实跑（L1/L2/L3）+ Stage 06 e2e 结果（L4=50））
- **SC-DOC-05b**：F10 豁免行口径扩注动态间隔内存读取（Read 确认）；新增「settings.json corrupted 警示条 L4」豁免行（grep 命中）；L4 段含 settings.e2e 行（grep 命中）；L2 段无 open-hooks-config 两行、含 settings-hooks-page（grep 双向确认）
- **DOC-转义**：本任务全部 md 产物零转义残留——对 docs/settings-center/、CONTEXT.md、.claude/CLAUDE.md、.claude/adr.md、.claude/test-inventory.md 执行 grep，模式为「反斜杠紧跟反引号」两字符序列（md 产物禁转义；命中即 not_fixed——本条断言自身的文字描述除外）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
7. `npm run test:l3`
