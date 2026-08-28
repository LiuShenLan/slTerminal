# Stage 划分（conda-profile-fix）

> 偏离豁免：skill 建议每 Stage 3-15 项；本任务共 5 项（2 代码 + 3 文档），属单点缺陷修复。Stage 01 仅 2 项的豁免理由 = 两项同文件强耦合（修复 + 其防复发测试不可拆分），拆开只会制造串行空转。

## Stage 01 — 删 -NoProfile + 防复发测试

- 改动项：B17-FIX、TE-B17
- agent 分工表：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| shell-fix | B17-FIX + TE-B17 | `src-tauri/src/pty/shell.rs` |

单 agent 单文件，无文件重叠问题。

- 实现要点：照抄 checklist B17-FIX / TE-B17 步骤与代码块；不碰 spawn.rs / reader.rs / 任何 ConPTY flags；注释中文；`e2e-tests/terminal.e2e.ts:412` 的 `-NoProfile` 不动（一次性辅助命令，非本故障链路）。
- 人工验证点：本 Stage 无（人工验证统一在收尾 MANUAL-B17）。
- 门禁命令：
  1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
  2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- verify：`docs/conda-profile-fix/workflows/verify/stage-01.md`
- commit message：`fix(pty): 移除 spawn PowerShell 的 -NoProfile——恢复用户 profile 加载，修复 conda activate 失效（B17）`
- git add：`src-tauri/src/pty/shell.rs`
- fix-loop constraints：无特殊（传空串）

## Stage 02 — 文档同步（固定最后）

- 改动项：DOC-B17a、DOC-B17b、DOC-B17c
- agent 分工表：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| doc-pty | DOC-B17a | `src-tauri/src/pty/CLAUDE.md` |
| doc-root | DOC-B17b | `.claude/CLAUDE.md` |
| doc-inventory | DOC-B17c | `.claude/test-inventory.md` |

三文件零重叠，可并行。

- 实现要点：
  - doc-inventory 必须先实跑取数（cargo test passed 总数 + shell.rs `#[test]` 属性数）再写计数（步骤见 checklist DOC-B17c），登记纪律 TQ-CI-01 禁凭计算；
  - doc-pty 红线行引用守卫用例名 `test_pwsh_args_no_noprofile_b17`；
  - 所有文档描述须对照 Stage 01 完成后的 shell.rs 真实代码，不撒谎。
- Stage 特殊纪律（fix-loop constraints 传同一句话）：**本 Stage 只改 markdown 文档，禁止改任何代码/测试文件**。
- 人工验证点：无。
- 门禁命令：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（为 DOC-B17c 计数核对供数；纯文档无静态门禁）
- verify：`docs/conda-profile-fix/workflows/verify/stage-02.md`
- commit message：`docs(pty): B17 文档同步——profile 加载红线 + 编号登记 + 用例清单/豁免登记`
- git add：`src-tauri/src/pty/CLAUDE.md`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`

## 收尾人工验证（MANUAL-B17）

全部 Stage 完成后执行（checklist 同名节三步）：win11 本机 + win10 部署机（miniforge）双系统实测 conda 激活 + python 启动 + OSC cwd/标题切换冒烟。
