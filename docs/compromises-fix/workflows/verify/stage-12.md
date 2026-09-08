# Stage 12 逐项验证断言（唯一真值源）

> stage-12 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CP-023-a（口径改造）**：**执行期翻案（2026-09-08）**——`#[coverage(off)]` 在 stable rustc（1.94/1.95/1.96/1.98.1 实测）仍为 experimental（E0658，tracking #84605），checklist「rustc 1.96 已稳定支持」前提失实（同 CP-040 先例，checklist.md 条目已补翻案注记）。走 A 分支：不引入属性（`rg "#[coverage(off)]" src-tauri/src` 零命中为预期）；pty 5 文件缺口按逐行映射逐条登记 pty/CLAUDE.md 豁免表（Read 确认逐条三列、非笼统「收尾」）；test-exemptions.md 含 main.rs 3 行豁免登记 + 现行口径新数字。
- **CP-023-b（基线重测）**：`cargo llvm-cov -- --test-threads=1` 退出码 0（全绿才计数字）；摘要百分比 = test-exemptions.md 登记新值（±0.1pp）。
- **CP-023-c（目标重定）**：P_new ≥ 90% → test-exemptions.md 原 :25 行已删且有销记；P_new < 90% → 该行改写为生产口径 + 残余缺口逐项登记（pty/CLAUDE.md 豁免表逐条三列，禁止笼统「收尾」登记，Read 确认）。
- **CP-023-d（main.rs）**：`grep -c "main.rs" .claude/test-exemptions.md` ≥ 1（3 行胶水豁免行新增）；`src-tauri/src/main.rs` 未加 `#[coverage(off)]`（Read 确认——登记豁免更诚实）。
- **销项总扫**：`docs/compromises.md` 44 项逐条已勾选销项或转休眠/翻案（Read 全表确认，格式照 CP-015 先例）；转休眠/翻案项（CP-001 机检未触发、CP-003 分支 B、CP-007 指纹分支、CP-029 环境出口、CP-031 同态维持、CP-033 分支 B2 等）登记口径与代码现状一致；抽查 ≥5 个已销项登记点原文对照代码现状（证据列入 details）。
- **全量回归**：四级测试 + 四门静态门禁 + knip + debug 构建全绿（见下）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
7. `npm run test:l3`
8. `npm run e2e`
9. `npx knip --production`
10. `npx tauri build --debug --no-bundle`

## 人工验证点总收（逐项打勾才闭环）

- CP-009 claude 实机滚轮（默认矩阵零漂移确认）
- CP-010 Win10 回退 toast
- CP-003 Node 26 e2e 全量
- CP-029 定责结论
- CP-007 扫描成本门槛数字
- S10 spike go/no-go + 预览行为回归
- S11 布局/面板全场景
