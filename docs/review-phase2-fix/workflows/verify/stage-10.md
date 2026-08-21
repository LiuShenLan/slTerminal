# Stage 10 逐项验证断言（唯一真值源）

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read/Glob 逐条核实，给出证据（文件+行号）；文档类断言须对照真实代码/test-inventory 核实，防文档撒谎；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-31**：Glob `src/panels/editor/CLAUDE.md` 存在；Read 确认含「职责 → 架构决策 → 文件表 → 测试模式」结构且含 FE-31 大文件不虚拟化决策全文
- **FE-31**：`src/panels/CLAUDE.md` 无大段编辑器细节残留（编辑器专属节已迁出，留 `@editor/CLAUDE.md` 交叉引用——grep 命中），面板通用决策保留
- **FE-31**：`.claude/adr.md` ADR-0009 FE-31 行登记点指向 `src/panels/editor/CLAUDE.md`（grep 命中）
- **DOC-11**：grep `src/ipc/CLAUDE.md` 无 `setFocus` 残留；含「六个 wrapper」与「9 用例」表述（Read 确认与 `src/ipc/window.ts` 实际导出数一致）
- **DOC-12**：grep `src/ipc/CLAUDE.md` 含 `ipc-agent-history-contract.test.ts（18 用例`；grep `src/__tests__/ipc-agent-history-contract.test.ts` 头注释含 18（两处与 test-inventory:100 一致）
- **DOC-13**：grep `src/panels/CLAUDE.md` 四处用例数与 `.claude/test-inventory.md` 一致（detect-webgl=4、terminal-instance=6、use-xterm-lifecycle=86、terminal=27——verify agent 须先查 test-inventory 真值再比对）
- **DOC-14**：grep `src/features/sideViews/CLAUDE.md` 含 54（sideBarState）与 40（activityBar），与 test-inventory:226-227 一致
- **TE-15**：grep `.claude/adr.md` 含「json-schema-library」债务登记；grep `src/features/hooksConfig/CLAUDE.md` 含同义引用
- **登记收口**：`.claude/adr.md` 含 D12~D20 决策登记（grep 「D15」命中）；`.claude/CLAUDE.md` 需求编号索引含 SEC-15/SEC-16/SEC-17 三行（grep 命中）
- **登记收口**：`.claude/test-inventory.md` 含 S03~S09 新增用例登记（抽查 3 个新用例名——如 `paths_match_single_side_failure_rejected`、`set_project_root_serializes_concurrent_calls`、global 命令集守卫用例——grep 命中）

## 全量测试（全门禁终跑，全部通过为门禁）

1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `npx tsc --noEmit`
4. `npx eslint src/`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm test`
7. `npm run test:l3`
8. `npx knip --production`
9. `npx tauri build --debug --no-bundle`
