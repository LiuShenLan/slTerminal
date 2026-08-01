# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-01**：`e2e-tests/fixtures/claude-projects/` 存在；内含 ≥2 个编码目录；fixture 覆盖全部 7 形态（Read 文件清单逐一核对）：custom-title 会话、ai-title 会话、无标题回退首条 prompt 会话、无 cwd 会话、孤儿 cwd 会话、`agent-*.jsonl` 平铺文件（应排除）、`<id>/subagents/agent-*.jsonl`（应排除）；指向 E2E 临时项目目录的 cwd 用占位符（Read 确认占位符标记存在）。
- **TE-02**：`e2e-tests/run-wdio.cjs` grep 命中 `SLTERM_CLAUDE_PROJECTS_DIR` 与 `.tmp-claude-projects`；副本逻辑 = 启动前从 fixtures 复制重建 + 占位符替换真实路径（Read 确认）；`.gitignore` grep `.tmp-claude-projects` 命中。
- **SEC-02 红线（语义式）**：`run-wdio.cjs`、`test.e2e.ts` 新 describe、fixtures 构建脚本中**不存在**任何指向用户真实 `~/.claude/projects`（或 `%USERPROFILE%\.claude\projects`、`homedir()` 拼接）的写/删操作（Read 全部新增代码确认；读操作亦只允许经 env 覆盖后的副本根）。
- **TE-03**：`e2e-tests/test.e2e.ts` 含新 describe（历史会话主题）；用例覆盖：展开全部项目区列表展示、标题回退展示、搜索过滤、复制恢复命令剪贴板断言、重命名（弹窗→列表更新→副本文件尾部 custom-title 行 Node 侧断言）、删除（确认→列表移除→副本文件消失 Node 侧断言）、孤儿 ✗ + 双击无反应、恢复编排（项目入列 + 切页 + 终端页签 + 终端缓冲含 `claude --resume`）；恢复用例**不含**「claude 成功进入会话」类断言（Read 确认——fixture id 非真实会话）。
- **选择器一致**：新 describe 中所有 `data-e2e` 选择器均在 FE-12 清单内（grep 提取新 describe 的选择器逐一比对 Stage 05 落盘属性）。
- **TE-04**：`.claude/test-inventory.md` 中 E2E 用例数已更新且与本 Stage 后 test.e2e.ts 实际 `it(` 计数一致（静态 grep 计数比对）；新 describe 在清单中有条目。
- **旧用例不回归**：wdio 实跑输出中既有用例（含 Agent Status 相关用例）全部通过——以测试 agent 报告为准（E2E 红线实证兜底）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run build:e2e`
7. `npm run wdio`
