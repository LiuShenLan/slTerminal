# Phase 0 验收审计

> 审计对象:`plan/execute/Phase 0 结果验证.md`。对照标准:`plan/5.0 phase-0-工程与测试基建.md` §7/§8、`plan/4. 完整开发计划.md` Phase 0。
> 审计日期:2026-06-18。仅审计,未改代码、未改原报告。

## 0. 结论

**Phase 0 未达成 DoD,原报告存在 4 处不实 + 6 处缺漏。** 关键:L4(E2E)spec 已写但未接进 `wdio.conf.ts`,CI 实为 exit 1(被 `continue-on-error` 掩盖),报告却写"CI 全绿 通过"。L1/L2/L3 真实通过,骨架与 invoke lint 真实落地;但 5.0 §8 DoD"四层测试各有可运行样例 + CI 绿"未满足。

## 1. DoD 逐条对照(5.0 §7.1 自动化 / §7.2 人工 / §8 DoD)

| DoD 项 | 标准来源 | 实际 | 判定 | 证据 |
|--------|---------|------|------|------|
| 后端单测 L1 | §7.1 | `cargo test` 通过,含 AppError 序列化 | ✅ 真实 | `error.rs:28-33` |
| 前端单测 L2 | §7.1 | Vitest 通过,mockIPC ping | ✅ 真实 | `src/__tests__/ipc-ping.test.ts` |
| L3 终端 | §7.1 | feed "hi" → serialize 含 hi | ✅ 真实 | `src/__tests__/terminal-serialize.test.ts:21` |
| L4 E2E | §7.1/§4 | spec 存在但**未接 wdio.conf**,CI 跑空 exit 1 | ❌ 未达成 | `wdio.conf.ts:9` `specs: []`;`test/specs/test.e2e.ts` 闲置 |
| tauri build | §7.1 | `--debug --no-bundle` 产 exe | ✅(偏差见 §4) | 产物路径存在 |
| CI 全绿 | §7.1/§8 | **未绿**,E2E step exit 1 | ❌ 未达成 | 报告所贴 `gh run view` 含 `X Process completed with exit code 1` |
| 骨架目录 | §8 | 前后端目录齐 | ✅ 真实 | — |
| invoke lint | §8/§5 | `no-restricted-imports` 生效 | ✅ 真实 | `eslint.config.js:9-14` |
| 暗色空窗无报错 | §7.2 | 人勾"通过",无观察记录 | ⚠️ 弱证据(M 层人勾可接受,见 D6) | — |

> §8 DoD = 骨架目录 + **四层测试各有可运行样例** + CI 绿 + invoke lint。L4 未跑通 + CI 未绿 ⇒ DoD 未达成。

## 2. 报告不实之处(原文与证据矛盾)

1. **§二.1「CI 全绿 通过」**:所贴 `gh run view` 明确 `X Process completed with exit code 1`(E2E step,`ci.yml:59-61` `continue-on-error: true` 掩盖)。CI 项非视觉人工项,不可勾"通过"。
2. **§9「所有依赖与 version-pins.md 对齐」**:
   - `notify`:`Cargo.toml:29` 用 `9.0.0-rc.4`(预发布),`version-pins.md` 钉表写 `8.2.1`、红旗 #8 明令"生产用 8.x"。不对齐。
   - `@tauri-apps/api`:`package.json:29` `^2.11.1`(caret 非精确钉),与 `tauri` crate `2.11.3` patch 不同步;红旗 #1 要求"锁死两端精确版本号",未做。
3. **§2「无 warning 被提升为 error」**:误导。dead_code warning 实际存在(AppError 五变体 never constructed,见 CI annotations),只是 `ci.yml:25` `rustflags: ""` 关掉了 `setup-rust-toolchain` 默认的 `-D warnings` 才没升 error。warning 仍在,并非"无 warning"。
4. **§8「CI … →E2E(continue-on-error: true)」**:暗示 E2E 已配置运行,实为 `wdio.conf.ts` 空壳(`specs: []`、`capabilities: [{}]`),跑零个 spec。

## 3. 缺漏之处(报告未覆盖的交付项)

1. **L4 接线缺失**:spec `test/specs/test.e2e.ts` 已写(启动 + `getTitle()` 断言,正合 §4),但 `wdio.conf.ts` 未引用、未配 `browserName:'wry'`/`tauri:options.application`/`onPrepare` 起 tauri-driver(5.0 §5)。spec 闲置。
2. **tracing 未接入**:`Cargo.toml:26` 声明 `tracing = "0.1"`,但 `src-tauri/src/` 全文 grep `tracing` 零命中——无 `tracing::info!`、无 subscriber、无 `tracing-subscriber` 依赖。5.0 §4.2/4 文档 Phase 0 列"接入 tracing 日志"为交付项,报告只字未提。
3. **Tauri 前后端版本互锁(红旗 #1)未兑现**:精确钉两端 / 对齐 patch / 加一致性检查,三件未做;报告未提。
4. **`src-tauri/tests/` 集成目录缺**:5.0 §3 列 `src-tauri/tests/`(集成),实际只有 `error.rs` 内联 `#[cfg(test)]`。
5. **测试目录布局未对齐 5.0 §3**:§3 列 `test/terminal/`、`e2e-tests/`;实际 L3 在 `src/__tests__/`、E2E 在 `test/specs/`、`wdio.conf.ts` 在根。
6. **红旗 #1/#8 处置未在报告体现**:钉表红旗清单要求"phase 0 落地后"收口,报告 §9 通篇未提收口状态。

## 4. 决策台账(grill 已定)

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D1 | L4 门控 | **补齐才算完成**:接 `wdio.conf.ts` 跑通现有 spec,CI 真绿后方算 DoD | `wdio.conf.ts`、`ci.yml` |
| D2 | CI 警告策略 | 恢复 `-D warnings`(去 `rustflags: ""`),给 `AppError` 加作用域 `#[allow(dead_code)]` + 注释"Phase 0 占位" | `ci.yml:25`、`error.rs:5` |
| D3 | notify 版本 | 保留 `9.0.0-rc.4`;同步改 `version-pins.md` 钉表为 9.0.0-rc.4、红旗 #8 标"已评估可接受";报告 §9 不再写"对齐" | `version-pins.md`、报告 §9 |
| D4 | tracing 接入 | 最小接入:加 `tracing-subscriber`,`run()` 里 init 控制台 subscriber + 一句启动 `info!` | `Cargo.toml`、`lib.rs` |
| D5 | Tauri 互锁 | `@tauri-apps/api` 去-caret 精确钉并与 `tauri` crate 对齐 patch;CI 加一步比对 Cargo.toml↔package.json 版本不一致即失败 | `package.json`、`Cargo.toml`、`ci.yml` |
| D6 | 人工项 2/3/4 | 维持"通过"人勾(M 层视觉项,人勾即证据);**但 §二.1 CI 项须改**,非视觉项不适用 | 报告 §二 |
| D7 | 目录布局 | 严格对齐 5.0 §3:建 `e2e-tests/`(移入 `wdio.conf.ts`+spec)、建 `test/terminal/`(放 L3)、补 `src-tauri/tests/`;清空 `test/specs/` | 测试目录 |

## 5. 小修建议(未单独 grill,附推荐)

- **Node 20 deprecation**:`actions/checkout@v4`→`v5`、`actions/setup-node@v4`→`v5`(node 24),消 CI annotation + 防未来断裂。`ci.yml:15,18`。
- **`tauri build --debug --no-bundle`**:可接受(仍产 exe),但 5.0/4 写的是 `tauri build --debug`;报告宜注明偏差理由(省 bundling 时间)。
- **`tauri-plugin-opener` / `@tauri-apps/plugin-opener`**:create-tauri-app 默认自带,非 plan 范围,无害;报告宜注明"脚手架自带,保留"。
- **`src-tauri/src/main.rs`**:确认仅 `app_lib::run()`(5.0 §5),审计已见 `main.rs` 存在,符合。

## 6. 收口路径(执行顺序)

1. D2 恢复 `-D warnings` + `AppError` 作用域 allow → 本地 `cargo build`/`cargo test` 仍绿。
2. D4 接 tracing → `cargo test` 绿。
3. D5 钉 Tauri 两端 + CI 一致性检查。
4. D3 改钉表 + 红旗 #8。
5. D7 迁测试目录到 5.0 §3 布局。
6. D1 接 `wdio.conf.ts`(specs 指向 `e2e-tests/`、配 `tauri:options.application` 指向 debug exe、`onPrepare` 起 tauri-driver)→ 本地 `npm run wdio` 跑通"启动+断言标题"。
7. 小修:bump actions 版本。
8. 重跑 `cargo test` / `npm test` / `npm run tauri build -- --debug --no-bundle` / `npm run wdio`(本地)→ push → `gh run watch` **真绿(含 L4)**。
9. 重写《Phase 0 结果验证.md》为如实版本(CI 未绿→绿、L4 未接→已接、tracing 未接入→已接入、§9 对齐→实际版本、§2 warning 表述修正)。
10. DoD 逐条复核通过 ⇒ Phase 0 收口。

> 一句话:L1–L3 与骨架真实通过,但 L4 接线缺失 + CI 未绿,DoD 未达成;报告 4 处不实、6 处缺漏,须按 D1–D7 + 小修收口后重验。
