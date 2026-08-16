# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **DOC-01**：`grep -ri "darcula" src/ --include=CLAUDE.md` 零命中；`src/features/navTree/CLAUDE.md` 与 `src/features/titleBar/CLAUDE.md` 存在（含职责/架构决策/文件表结构）；语义式断言：src/theme/CLAUDE.md 与 src/features/sideViews/CLAUDE.md 的关键描述（默认方案 id、视图清单、活动栏宽度）须 Read 对照当前代码核实一致（防文档撒谎）
- **DOC-02**：grep `已退役\|退役` 命中 `CONTEXT.md`（Agent Status 视图条）；「配置」钮描述存在
- **DOC-03**：Read `.claude/test-inventory.md` 确认新增/修改/删除用例已登记（抽查 emoji-scan/title-bar/nav-tree/confirm-dialog 条目存在）
- **DOC-04**：grep `远期愿景` 命中 `docs/ui-redesign/requirements.md`（UI-405/406/407 三条均有注记——须 Read 确认三条全覆盖）
- **DOC-05**：grep `实现期决策` 命中 `.claude/adr.md`（ADR-0003 追加小节，含剔除/替换/配置钮/挂法/依赖/取舍六点）
- **DOC-06**：Read 根 `.claude/CLAUDE.md` 模块索引确认含 navTree 与 titleBar 行；grep `Agent 状态视图` 等过期描述已更新（语义式：对照 sideViewDefs.ts 当前注册核实索引描述不撒谎）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
