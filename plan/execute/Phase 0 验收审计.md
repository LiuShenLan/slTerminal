# Phase 0 验收审计（第二轮 · 补救后复审 · 勘误版 v2）

> 审计对象：`plan/execute/Phase 0 结果验证.md`（补救后终版）。对照标准：`plan/5.0 phase-0-工程与测试基建.md` §7/§8、`plan/4. 完整开发计划.md` Phase 0、`plan/execute/Phase 0 执行计划.md` 决策表 E1–E5。
> 审计日期：2026-06-18。本轮**独立验证**（读代码 + 实跑命令 + 联网核实 + 本地 Cargo.lock + `npx tauri info`）。仅审计，未改代码、未改原报告。
> **勘误 v2**：继 v1 纠正 §3.1（D2 -D warnings）、§3.4（wdio-webdriver 版本）后，本版再纠正 §3.3（D5 tauri build 内置检测）——经 E4 决策 + `tauri info` 一手证据，报告该表述准确，撤销"弱保证"判定。详见 §3 与 §5.3。

## 0. 结论

**Phase 0 DoD 实质达成，报告无失实。** L1–L4 + build 本轮**实跑全过**；D1–D7 全部兑现（D5 经 E4 决策 + `tauri info` 证实实质已兑现）。报告原 4 处"失实"经逐条复核**全部撤销**（均系审计误判）。残留仅为小项：**getTitle 时序+断言**、**CI msedgedriver 死代码**、**红旗 #1 文本未关**，外加若干小缺漏。

## 1. 本轮独立验证记录（非报告自述）

| 项 | 命令 | 实跑结果 | 判定 |
|----|------|---------|------|
| L1 后端单测 | `cargo test`（src-tauri/） | 2 passed，零 warning | ✅ |
| L2+L3 前端单测 | `npm test` | 2 passed | ✅ |
| 前端构建 | `npm run build` | tsc 通过 + vite ✓ built | ✅ |
| L4 E2E | `npm run wdio` | 1 spec PASSED；**无 msedgedriver** 仍过 | ✅（getTitle 值见 §3.2） |
| Tauri 版本诊断 | `npx tauri info` | tauri 2.11.3 / api 2.11.1 / cli 2.11.2，**无 mismatch** | ✅ |
| 后端构建 | `cargo build`（cargo test 隐含） | Finished，零 error/warning | ✅ |
| Debug exe | exe 存在 + L4 启动成功 | 20MB exe @ 16:50 | ✅ |
| CI 全绿 | `gh run view` | **本环境无 `gh`，未独立复核** | ⚠️ 信报告贴的输出 |
| lockfile 锁版本 | Cargo.lock / package-lock | tauri 2.11.3 / api 2.11.1 均锁死 | ✅ |

> 本地 L1–L4 + build 全绿。`tauri info` 对 2.11.3/2.11.1 pair 无 mismatch ⇒ Tauri 内置检查判其兼容。L4 无 msedgedriver 仍过 ⇒ embedded driver 不依赖 msedgedriver。CI 仅信报告所贴输出（14 step ✓、L4 无 `continue-on-error`，与 `ci.yml` 一致）。

## 2. DoD 逐条对照（5.0 §7.1 / §7.2 / §8）

| DoD 项 | 实际 | 判定 | 证据 |
|--------|------|------|------|
| L1 后端单测 | 2 passed | ✅ | 本轮实跑 |
| L2 前端单测 | 2 passed | ✅ | 本轮实跑 |
| L3 终端 | feed "hi" → serialize 含 hi | ✅ | 本轮实跑 |
| L4 E2E | 1 spec PASSED | ✅ | 本轮实跑 |
| tauri build | exe 产出 + L4 启动 | ✅ | `--no-bundle` 偏差见 §4.9 |
| CI 全绿 | 报告称 14 step ✓ 含 L4、无 continue-on-error | ✅⚠️ | `ci.yml` 确认；本环境无 `gh` 未独立复核 |
| 骨架目录 | 前后端目录齐 | ✅ | — |
| invoke lint | `no-restricted-imports` 生效 | ✅ | `eslint.config.js` |
| 暗色空窗无报错 | 人勾"通过"（M 层） | ⚠️ 弱证据 | 上轮 D6 已接受人勾 |

> §8 DoD = 骨架目录 + 四层测试各有可运行样例 + CI 绿 + invoke lint。**L1–L4 实跑全过 ⇒ DoD 实质达成**；CI 绿为附条件（信报告、未独立复核）。

## 3. 报告问题复核（含勘误 v2）

### 3.1 ~~`-D warnings` 未生效~~ —— **撤销（v1 勘误）**

- `setup-rust-toolchain` 的 `rustflags` 输入默认 `-D warnings`；不传即生效，传 `rustflags: ""` 才禁用。`ci.yml:22-23` 已删空串 ⇒ -D warnings 经默认值生效。项目首轮 `rustflags: ""`"防 dead_code 阻塞"自证默认值。**D2 已兑现。**

### 3.2 getTitle 取值 —— **降级：时序敏感 + 断言过弱（非"失实"）**

- `browser.getTitle()` 取 `document.title`（index.html = "slTerminal"）。本轮实测得 `localhost` ⇒ 加载初期时序问题，非报告造假。真问题：spec 仅 `toBeTruthy()`，5.0 §4"断言标题"仅最低满足，且 getTitle 时序不稳。定性为"L4 可过但有健壮性隐患"。处理见 §5.2。

### 3.3 ~~D5"靠 tauri build 内置版本检测"是弱保证~~ —— **撤销（v2 勘误）**

- 上版称报告 §9/D5"靠 tauri build 内置版本检测"是"弱保证冒充收口"。**经复核此判断错误。**
- **证据①（E4 决策，执行计划:367）**：项目既定决策 E4 明文"D5 简化 = 精确钉版本 + 靠 `tauri build` 内置检测，**去自定义 CI 版本检查**"。即项目主动选择靠 tauri 内置检测、不要自定义 CI 检查——这正是报告所述，非"弱保证"。
- **证据②（`npx tauri info` 一手）**：Tauri 2.8+ 内置 JS/Rust 版本检查；`tauri info` 对 2.11.3/2.11.1 pair **无 mismatch 提示** ⇒ 判定兼容。
- **证据③（lockfile）**：Cargo.lock `tauri 2.11.3`、package-lock `@tauri-apps/api 2.11.1` 均锁死 ⇒ 漂移不可能。
- **结论**：D5 实质已兑现（精确钉 + lockfile + tauri build 内置检测，pair 经验证兼容）。**报告表述准确，撤销"弱保证"判定；D5′"补 CI 版本校验步"作废（违背 E4 且冗余）。** 唯一残留：`version-pins.md` 红旗 #1 文本未标"已收口"（§4.1）。

### 3.4 ~~wdio-webdriver 版本过度精确~~ —— **撤销（v1 勘误）**

- `Cargo.lock:3919` 解析 `tauri-plugin-wdio-webdriver v1.1.0`，报告值准确。撤销。

> **至此报告"4 处失实"全部撤销。报告无失实。**

## 4. 缺漏之处（报告未覆盖）

1. **红旗 #1 文本未关**：`version-pins.md` 红旗 #1 仍写"须锁死…加版本一致性检查"TODO 状态，未标"已收口"。D5 实质已兑现（§3.3），仅文档未同步。
2. **CI msedgedriver 步骤是死代码**：`ci.yml:31-35` 装 msedgedriver，但 E2E 用 embedded driver（`tauri-plugin-wdio-webdriver` → `webview2-com`，`Cargo.lock:3945`，零 msedgedriver 依赖；本轮本地无 msedgedriver 仍 PASS）。应删。
3. **version-pins §六 过时**：列 `tauri-driver 2.0.6`，实际用 embedded（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver`）；后者及 `@wdio/tauri-plugin`/`@wdio/tauri-service` 未入钉表。
4. **`patch-package` 死依赖**：`package.json:45` 有，但无 `patches/`、无 `postinstall`；Node 26 兼容靠 `run-wdio.cjs` 便携 Node 22。
5. **E2E getTitle 时序+断言**（§3.2）：取值偶发初值、断言仅 truthy。
6. **tracing `info!` 默认静默**：`lib.rs:22` `EnvFilter::from_default_env()` 无 `RUST_LOG` 默认 error 级，启动日志不输出。
7. **CI 全绿未独立复核**：本环境无 `gh`。
8. **Node 20 deprecation 未 bump**：`actions/checkout@v4`、`setup-node@v4`；报告 §四.2 已列延期。
9. **`tauri build --debug --no-bundle` 偏差**：5.0/4 写 `tauri build --debug`；注明即可。
10. **噪音警告（不阻断）**：`npm test` `getContext() not implemented`（jsdom）；L4 teardown `Failed to clear mock store`（@wdio/tauri-service）。

## 5. 决策台账

### 5.1 首轮 D1–D7 兑现状态（勘误 v2 后）

| # | 议题 | 状态 | 说明 |
|---|------|------|------|
| D1 | L4 接线 | ✅ 已兑现 | `wdio.conf.ts` 接好，本地 `npm run wdio` PASS（E3 方案） |
| D2 | -D warnings | ✅ 已兑现（v1 勘误） | action 默认 `-D warnings` 已生效（E5 方案） |
| D3 | notify 钉表 | ✅ 已兑现 | 钉表改 9.0.0-rc.4，红旗 #8 标已评估 |
| D4 | tracing 接入 | ✅ 兑现 + 小瑕疵 | subscriber + info! 已接入，info! 默认静默（§4.6） |
| D5 | Tauri 互锁 | ✅ 已兑现（v2 勘误） | E4：精确钉 + lockfile + tauri build 内置检测；`tauri info` 验证 pair 兼容。仅红旗 #1 文本未关 |
| D6 | 人工项 | ✅ 已兑现 | CI 项移至自动化 §一.8，§二 仅 3 视觉项人勾 |
| D7 | 目录布局 | ✅ 已兑现 | `e2e-tests/`、`test/terminal/`、`src-tauri/tests/`、`src/__tests__/` 齐；`test/specs/` 已清 |

### 5.2 本轮决策

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D2′ | ~~加 env:RUSTFLAGS~~ | **作废（v1 勘误）**：-D warnings 经 action 默认已生效 | 不改 `ci.yml` |
| D5′ | ~~补 CI 版本校验步~~ | **作废（v2 勘误）**：违背 E4 且冗余（lockfile 锁版本 + tauri build 内置检测已覆盖）。**唯一动作：`version-pins.md` 红旗 #1 标"已收口（精确钉 2.11.3/2.11.1 + lockfile + tauri build 内置检测，`tauri info` 验证兼容）"** | `version-pins.md` |
| — | getTitle 时序+断言 | 等 webview/标题就绪再取 + 断言改 `toBe('slTerminal')`；报告 §4 改"getTitle 时序敏感，已加等待 + 精确断言" | `test.e2e.ts`、报告 |
| — | CI msedgedriver 死代码 | 删 `ci.yml:31-35`（embedded driver 用 webview2-com，不需 msedgedriver） | `ci.yml` |

### 5.3 勘误说明

- **§3.1 D2**（v1）：误判 -D warnings 未生效，源于采信含糊搜索结果。依据：action `rustflags` 默认 `-D warnings`（联网）+ 项目首轮 `rustflags: ""` 用以禁用的历史铁证。
- **§3.4 wdio-webdriver 版本**（v1）：过度挑刺。`Cargo.lock` 解析 1.1.0，报告值准确。
- **§3.3 D5**（v2）：误判"弱保证冒充收口"，源于未查执行计划决策表 E4、未取一手证据。依据：E4 决策（执行计划:367）+ `npx tauri info` 无 mismatch + lockfile 锁版本。
- **共性病因**：标"失实"前未查项目决策表（E1–E5）、未取一手证据（action.yml / `tauri info` / `Cargo.lock`）。已记记忆，后续审计先查决策表与一手证据再下"失实"结论。

## 6. 小修建议（未单独 grill，附推荐）

- **patch-package**：移除死依赖，或注明闲置。
- **version-pins §六**：删/改 `tauri-driver` 行，补 `@wdio/tauri-service`、`@wdio/tauri-plugin`、`tauri-plugin-wdio-webdriver`（与 Cargo.lock 1.1.0 一致）。
- **tracing**：设默认 filter（`EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())`）使启动日志默认可见，或注明需 `RUST_LOG=info`。
- **Node 20 deprecation**：`actions/checkout@v4→v5`、`setup-node@v4→v5`（报告已列延期）。
- **`--no-bundle` 偏差**：报告注明"省 bundling 时间，仍产 exe"。

## 7. 收口路径（执行顺序）

1. `version-pins.md` 红旗 #1 标"已收口"（D5 实质已兑现，见 §3.3 证据）。
2. 删 `ci.yml:31-35` msedgedriver 步。
3. `test.e2e.ts` 加等待 + 断言改 `toBe('slTerminal')`；报告 §4 纠正 getTitle 措辞。
4. 小修：移 patch-package、更新钉表 §六、tracing 默认 filter、（可选）bump actions 版本。
5. 重跑 `cargo test` / `npm test` / `npm run build` / `npm run wdio`（本地）→ push → CI 真绿。
6. 重写《Phase 0 结果验证.md》：§4 改 getTitle 时序+精确断言、删 msedgedriver 相关、红旗 #1 标已收口。（-D warnings、D5 表述无需改，均准确。）
7. DoD 逐条复核 + 本轮 §1 实跑再现 ⇒ Phase 0 如实收口。

## 8. 一句话结论

L1–L4 本轮**实跑全过、DoD 实质达成、报告无失实**（原 4 处"失实"经 E1–E5 决策表 + 一手证据逐条撤销）。残留仅为小项：getTitle 时序+断言、CI msedgedriver 死代码、红旗 #1 文本未关，外加小缺漏；按 §7 收口后 Phase 0 即如实通过。
