# Stage 05 逐项验证断言（唯一真值源）

> stage-05-e2e 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）。本 Stage 改动文件在 tsc/eslint 覆盖外——五条门禁仅作回归防线，wdio 实跑为人工验证点（不内嵌）。

## 断言清单

- **E2E-01**：grep `settings-plan-balance|settings-nav-planBalance` 在 e2e-tests/ 零命中；grep `"planBalance"` 在 settings.e2e.ts 仅出现于 taskId 上下文（`backgroundTasks.planBalance` 落盘断言 / `settings-background-tasks-interval-planBalance` 选择器后缀 / `-error-planBalance`）——逐处 Read 确认无语义残留（页 id 实参全部为 `"backgroundTasks"`）。
- **E2E-01**：settings.e2e.ts 用例④落盘断言为 `backgroundTasks.planBalance.intervalSec === 120` 形态（waitForSettingsFile 判定函数同步）；用例⑤红字文案断言为 `10–3600 秒`（不含「默认」字样）；用例⑧两处 `selectedPage` 断言为 `"backgroundTasks"`；用例数不变（11 例）——grep `it(` 计数核对。
- **E2E-02**：`e2e-tests/background-tasks.e2e.ts` 存在，用例 A-D describe/it 结构完整（A 页渲染两行齐备 / B 改频率端到端落盘 + 规范化回显 / C 勾选禁用 planBalance → footer 隐藏 → 重启用重显 + 磁盘 enabled 翻转 / D 非法频率行内红字不落盘）；grep `data-e2e` 选择器全部为新系列 `settings-background-tasks-*` 与既有 `plan-balance-row`/`plan-balance-footer`（无 `settings-plan-balance-*` 残留）；suite 级 settings.json 快照还原存在（照 settings.e2e.ts 先例）。
- **E2E-02**：`e2e-tests/wdio.conf.ts` specs 覆盖新 spec（Read 确认：glob 命中则零改动合法，显式清单则已登记 background-tasks.e2e.ts）。
- **E2E-03**：同文件含用例 E「定时刷新自动出现新会话」（SLTERM_CLAUDE_PROJECTS_DIR 写 jsonl → 等 tick → `nav-history-node` 计数 N→N+1，全程无手动刷新）与用例 F「禁用不自动出现 / 重启用恢复」（计数不变 → 重启用 → +1）；grep 确认无用例 G（tick 失败静默——降级豁免，spec 内不写；豁免登记由 DOC-02 在 Stage 06 完成）。

## 人工验证点（门禁不内嵌，收尾人工执行）

1. `npm run e2e` 全量绿（含 background-tasks.e2e.ts 六例 A-F）。
2. 真实 claude 会话：导航树历史区按配置频率自动更新，与点击刷新钮结果一致。
3. 勾选禁用套餐余量 → 轮询停止、footer 隐藏、最后快照保留；重新启用 → 恢复。
4. E2E 默认配置下 sessionRefresh（3s）/planBalance（10s）全程在跑——既有 history.e2e.ts / settings.e2e.ts 连带复跑确认无干扰。

## 全量测试（回归防线，全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. （环境豁免，2026-08-31 登记）命令 5 预期 exit 127：本机 rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，见 .claude/test-inventory.md 豁免表）；测试类断言以「测试存在性 grep + `cargo check --tests` 编译级 + clippy」为兜底。
