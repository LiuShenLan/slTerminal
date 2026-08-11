# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 是首个 Stage，断言针对 Stage 01 完成后的状态（CS-2 八路径扫描自此守护后续 Stage）。

## 断言清单

- **AQ-1**：`src/features/cliProfiles/profiles/claude/strategies.ts` 的 buildResumeCommand 中，cwd 的单引号被按 PowerShell 规则转义为两个单引号（语义式——Read 确认存在对 session.cwd 的单引号替换逻辑，不限实现写法）；「原样保留/未转义」类自述注释零残留（grep `原样保留` 于 strategies.ts 无命中）；`src/__tests__/cli-profile-claude.test.ts` 存在 cwd 含单引号的回归用例（如 `C:\Bob's Project` → `C:\Bob''s Project` 形态断言），且 L2 全绿
- **AQ-2**：`src-tauri/src/hooks/signal.rs` 存在信号文件大小上限常量（1MB 量级）与超限拒绝路径（语义式——Read 确认：读取前有大小判定；超限 → warn 日志 + 删除文件 + 不调用 emit；不限常量名）；同文件存在 >1MB 超限 L1 用例（断言 emit 零调用 + 文件已删），且 L1 全绿
- **AQ-3**：`src-tauri/src/agent_history/claude/ops.rs` 定位与删除链三处不跟随符号链接（语义式——Read 确认：一级子目录 is_dir 判定、命中文件 is_file 判定、delete_session 同名目录判定三处均附符号链接排除，不限具体函数调用形态）；同文件存在符号链接 L1 用例（含权限不足时跳过的注释说明）；SEC-05 的 sessionId 校验与定位流程未被削弱（Read 对照确认仅新增拒绝分支），且 L1 全绿
- **AQ-4**：`e2e-tests/run-wdio.cjs` fixture 缺失分支为非零退出（语义式——Read 确认 wdio 启动前存在 `process.exit(1)` 或等价 throw，且文案含防止回落真实 ~/.claude/ 的语义；不存在自动创建/临时目录兜底等新降级路径）；`node --check e2e-tests/run-wdio.cjs` 通过；L4 全绿（fixture 存在正常路径回归）；fixture 缺失终止的负向路径列入人工验证点（rename 实测），不强制自动化
- **ZQ-5**：`src-tauri/src/hooks/claude/config.rs` 写路径 null 入参视作空对象（语义式——Read 确认 config_write_sync 入口：null → 按 {} 参与 merge，不再 Validation；非 null 且非 object 仍 Validation；write_hooks_subtree 的 is_object 闸门不变）；同文件存在 hooks=null 写入 → 文件 hooks 键为 {} 且其他字段保留的 L1 用例，且 L1 全绿；`src-tauri/src/hooks/CLAUDE.md` write 语义段与上述行为一致（文档对照代码核实）
- **CS-1**：`src/__tests__/no-claude-literals.test.ts` 词法器对含表达式的模板字符串不再整体跳过（语义式——Read 确认：提取表达式外的字面量片段拼接后参与禁令判定）；存在自检用例（`` cl${''}aude `` 形态样例源码 → 判定违规），且 L2 全绿
- **CS-2**：SCAN_DIRS 含 `src/features/cliProfiles`（共八路径）且存在 `profiles/claude/` 目录级豁免（语义式——Read 确认扫描按路径前缀排除该目录，normalize 后比较）；存在豁免目录存在性断言（防拼写错静默空扫）；完整性断言为八路径；守卫全绿（八路径扫描零违规——含 cliProfiles 根目录现状零命中间接证实）
- **文档同步**：`src/features/cliProfiles/CLAUDE.md`（AQ-1 转义表述）、`e2e-tests/CLAUDE.md`（fixture 缺失终止）、`src/__tests__/CLAUDE.md`（守卫八路径 + 模板拼接检测）、`src-tauri/src/agent_history/CLAUDE.md`（符号链接拒跟随）、`.claude/test-inventory.md`（新用例登记 + 守卫条目）五处与代码终态一致（Read 对照核实）；`src-tauri/src/hooks/CLAUDE.md` 含 AQ-2 大小限制一句（backend-config 代写裁决）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `node --check e2e-tests/run-wdio.cjs`（launcher 语法级——tsc/eslint 覆盖外文件）
9. `npm run e2e`（L4——其余命令全部完成后单独串行执行）
