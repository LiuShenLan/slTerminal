---
paths:
   - "plan/execute/*结果验证.md"
description: "Phase 验收审计方法论——处理 plan/ 下文件时自动加载"
---

# Phase 验收审计规则

本文件在读取 `plan/` 路径下任何文件时自动加载。记录 Phase 0 三轮审计积累的审查方法论、常见陷阱和证据判断准绳。

## 0. 审计核心原则

**先查决策表 + 取一手证据，再下"失实"结论。** 本轮三次误判均因跳过此步。

## 1. 证据判据（从强到弱）

| 层级 | 来源 | 示例 | 可信度 |
|------|------|------|--------|
| L0 | 实跑命令 | `cargo test`、`npm run wdio`、`npx tauri info` | 最高 |
| L1 | 一手源文件 | `action.yml`（GitHub raw）、`Cargo.lock`、`ci.yml` | 高 |
| L2 | 项目决策表 | `plan/execute/Phase 0 执行计划.md` 中的 E1–E5 决策 | 高（既定决策是审计上限） |
| L3 | 联网一手资料 | crate 官方文档、Tauri 官方 changelog、npm registry | 中 |
| L4 | 二手搜索总结 | Web 搜索的 AI 摘要、社区博客 | 低（本轮以此为判据导致 §3.1 误判） |
| L5 | 被审报告自述 | `Phase 0 结果验证.md` 中的声称 | 待验证（不可直接采信） |

- **只有 L0–L2 证据矛盾时才可标"失实"**。L3–L4 仅作线索，不作判据。
- CI 全绿需 `gh run view` 独立复核；本环境若无 `gh`，标明"未独立复核"。

## 2. 审计流程（按顺序执行）

1. **读报告** → 提取所有"声称"（测试通过、CI 绿、版本对齐、配置生效等）。
2. **查决策表** → `grep -n '^| E' plan/execute/Phase\ 0\ 执行计划.md`。对照 E1–E5 决策，确认报告的收口声称是否与既定决策一致。**决策表说了"去自定义 CI 检查"（如 E4），就不要要求"加 CI 检查"。**
3. **取一手证据** → 按 §1 层级逐条验证：
   - 代码一致性：`Bash cat`/`grep` 读取 `ci.yml`、`lib.rs`、`Cargo.toml`、`package.json`、`version-pins.md`、`test.e2e.ts` 等
   - 锁文件验证：`grep` Cargo.lock / package-lock.json 核对版本
   - action 默认值：以 `raw.githubusercontent.com/<owner>/<repo>/<ref>/action.yml` 为准
   - Tauri 版本兼容性：以 `npx tauri info` 是否报 mismatch 为准
4. **实跑命令** → `cargo test`、`npm test`、`npm run build`、`npm run wdio`（至少跑 L1+L2+L4）。
5. **联网检索** → 对不确定的技术点（如 tauri 版本检查行为），搜索后汇总，再判断。二手总结仅作线索，以一手资料核实。
6. **问题分类** → 区分"失实"（报告与证据矛盾）、"缺漏"（未覆盖的交付项）、"小修建议"（无害改进）、"已知限制"。
7. **grill 决策** → 每个需用户定夺的发现，用 `AskUserQuestion` 提一个问题，给推荐答案（作为第一个选项）。不重复 grill 已定决策。

## 3. 已知易误判的技术事实（勿重复踩坑）

### 3.1 CI 相关
- `actions-rust-lang/setup-rust-toolchain@v1` 的 `rustflags` 输入**默认值 = `"-D warnings"`**。不传 `with: rustflags` ⇒ CI 已启用 `-D warnings`。传 `rustflags: ""` 才是关闭。
- 判断 action 默认值以 `action.yml` 为准，不要信 web 搜索的二手总结。参见 `memory/setup-rust-toolchain-default-deny-warnings.md`。

### 3.2 Tauri 版本检查
- Tauri CLI 2.8+ 内置 JS/Rust 版本兼容性检查。`npx tauri info` 若**无 mismatch 提示** ⇒ pair 兼容。
- 项目 E4 决策已定"精确钉 + tauri build 内置检测"为版本一致性方案，**不要建议额外 CI 检查**。
- `@tauri-apps/api` 最新即 2.11.1（无 2.11.3），Rust/JS patch 天然不同步是 Tauri 发布常态。

### 3.3 E2E / L4 相关
- embedded driver（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver`）依赖 `webview2-com`，**零 msedgedriver 依赖**。
- `browser.getTitle()` 取 `document.title`（非 OS 窗口标题）。可能因 webview 加载时序返回初始值（如 `localhost`），应用 `waitUntil` 轮询等待。
- `wdio.conf.ts` specs 必须指向实际 spec 文件，`capabilities` 必须配 `browserName:'tauri'`。

### 3.4 版本锁定
- 精确锁版本 = package.json/Cargo.toml 无 `^`/`~` + lockfile 已提交。
- `Cargo.lock` 中的已解析版本（如 `tauri-plugin-wdio-webdriver v1.1.0`）是运行的精确版本；Cargo.toml 中的 `"1"` 是 spec。报告写 1.1.0 是准确的（与 lockfile 一致）。

## 4. 红旗生命周期

1. **开红旗** → version-pins.md 新增条目，描述风险与条件。
2. **补救/收口** → 修改代码/配置消除风险，或在报告记录"已评估可接受"。
3. **关红旗** → 原条目追加"已收口"或"不再适用"，附证据（如 `tauri info` 无 mismatch、Cargo.lock 锁定版本）。
4. **审计核验** → 对照报告与 version-pins.md，确认每个红旗的收口状态与报告一致。

## 5. 当前项目记忆索引

本项目已在 `C:\Users\liush\.claude\projects\D--data-learn-code-slTerminal\memory\` 积累的记忆：

| 记忆 | 内容 |
|------|------|
| `setup-rust-toolchain-default-deny-warnings.md` | action rustflags 默认 -D warnings；`rustflags:""` 才禁用 |
| `audit-verify-before-flagging.md` | 标"失实"前查决策表 E1–E5 + 取一手证据 |
| `edit-chinese-md-with-builtin-tools.md` | 中文文档用内置 Read+Edit，filesystem mcp 有乱码风险 |

刷新记忆时用 `Read` 查阅上述文件，验证当前代码是否仍与之吻合。若记忆过时或错误，`Write` 更新。

## 6. 审查交付规范

- **审计报告** → `plan/execute/Phase <N> 验收审计.md`。逐条对照 DoD，含独立验证记录、失实/缺漏清单、决策台账、收口路径。
- **grill 节奏** → 一次只问一个问题，附带推荐答案选项。不猜测用户意图，不替用户做价值判断。
- **措辞精准** → "失实"仅用于 L0–L2 证据明确矛盾的情况。不确定时用降级措辞（"时序敏感""弱证据""待确认"）。
- **收口** → commit + push + `gh run watch` 确认 CI 全绿后，方可声明 Phase 完成。
