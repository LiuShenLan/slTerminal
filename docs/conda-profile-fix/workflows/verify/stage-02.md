# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DOC-B17a-1**：`src-tauri/src/pty/CLAUDE.md`「Shell 白名单（SEC-01 / SEC-15）」段尾行已更新——含「启动参数固定 `-NoLogo -NoExit -EncodedCommand`」与「禁止 `-NoProfile`」及守卫用例名 `test_pwsh_args_no_noprofile_b17`（Read 确认；且与 shell.rs 当前真实参数一致，不撒谎）
- **DOC-B17a-2**：`src-tauri/src/pty/CLAUDE.md`「外部坑/红线」列表含 B17 红线行——语义为「PowerShell 交互 shell 禁止 -NoProfile，用户 profile 必须原生加载，缺钩子则 conda activate 失效」（不限措辞，Read 确认语义）
- **DOC-B17b-1**：`.claude/CLAUDE.md`「需求编号索引」表含且仅含一行 B17 条目（`grep -n '| B17 |' .claude/CLAUDE.md` 命中 1 行），类型为「缺陷」，含义指向 -NoProfile 致 profile 不加载的 conda 故障
- **DOC-B17c-1**：`.claude/test-inventory.md` shell.rs 行计数 = 该文件 `#[test]` 属性实跑统计数（预期 33，以实跑为准），描述含「B17 profile 加载守卫」
- **DOC-B17c-2**：`.claude/test-inventory.md` 表头 / L1 段头 / L1 段小计三处计数一致，且与测试 agent 报告的 cargo test passed 总数口径一致（TQ-CI-01；差值须在行内注释或既有口径说明中可解释）
- **DOC-B17c-3**：`.claude/test-inventory.md` 豁免表含 B17 行——项目为 win11/win10 真实终端 conda 激活实测，兜底层级引用 L1 守卫用例（grep `B17` 命中豁免表区域）
- **DOC-INTEG-1**（语义式）：本次 Stage 02 三个文档之外无其他文件被改动（`git status --porcelain` 确认仅 `.claude/CLAUDE.md`、`.claude/test-inventory.md`、`src-tauri/src/pty/CLAUDE.md` 三个 .md 文件为 modified——docs/ 下本任务计划产物除外）

## 全量测试（全部通过为门禁）

1. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（为 DOC-B17c 计数核对供数；纯文档 Stage 无静态门禁）
