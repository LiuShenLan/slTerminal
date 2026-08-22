# CI+基建+inventory Review 报告

## 问题清单

### [高] L1 inventory 用例数与文件数双重失实
- **维度**: inventory
- **证据**:
  - `.claude/test-inventory.md:5` 写「全量 3495 用例（Rust 724 + ...）」
  - `.claude/test-inventory.md:45` 写「L1 — Rust 单元/集成测试（34 文件 / 724 用例）」
  - `.claude/test-inventory.md:85` 写「L1 总数按静态 grep `#[test]` 实查对齐 **724（34 文件）**」
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 实际通过 726 条
  - `grep "#\[test\]"` 在 `src-tauri` 下命中 726 处、33 个文件
- **证据类型**: 实证（跑过/重跑验证）
- **问题**: 文档自称为「用例数唯一真值源」，但 L1 表头、顶部总数、注释说明均少记 2 条，文件数多记 1 个。这会导致后续 PR、阶段报告、CI 口径引用错误数字，削弱清单的可信度，也会掩盖「新增/删除测试后未同步」的漏登记问题。
- **建议**: 以本次实跑结果（726 / 33 文件）校准表头、顶部总数与历史变更注释，并建立「每次改动后 `cargo test` + `grep '#\[test\]'` 双核对」的登记纪律。

### [高] L2 inventory 用例数失实且段小计与行级合计不匹配
- **维度**: inventory
- **证据**:
  - `.claude/test-inventory.md:5` 写「前端 2633」
  - `.claude/test-inventory.md:89` 写「L2 — 前端单元/集成测试（154 文件 / 2630 用例）」
  - `npx vitest run` 实际通过 2635 条（154 文件）
  - 各段小计相加：IPC 140 + 终端面板 258 + CLI profile 95 + 编辑器面板 137 + 工作区 275 + Store 92 + 资源管理器 305 + 导航树 56 + 侧栏视图 160 + Commit 64 + hooks 配置 233 + Diff/GitShow 84 + 快捷键 128 + 主题 200 + 通知/Agent 状态 92 + Agent 历史 81 + 启动/关闭 35 + 文件查看器/HTML 91 + E2E 辅助 36 + 标题栏/统一浮层 30 = **2592**
- **证据类型**: 实证（跑过/重跑验证）
- **问题**: L2 表头、顶部总数与实际运行相差 2~43 条；段小计（2592）与表头（2633/2635）相差 41~43 条，说明行级用例存在漏登记或段小计未随文件内 `it.each`/工厂展开同步更新。作为「唯一真值源」，这种内部不一致会让基于清单的覆盖率判断、阶段验收和 PR review 失真。
- **建议**: 以 `npx vitest run` 实跑 2635 为基准，逐段核对行级用例数（特别注意 `it.each`、`describeIpcContract` 等展开），使段小计、文件行、顶部总数三处一致。

### [中] CI 未执行 `cargo fmt` 门禁，与本地静态检查清单不一致
- **维度**: CI / 基建
- **证据**:
  - `.claude/CLAUDE.md:105` 规定静态检查门禁含 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - `.github/workflows/ci.yml:28-92` 的 job 步骤包含 clippy、eslint、tsc、build、L2/L3/L4 等，但无 `cargo fmt --check` 步骤
- **证据类型**: 静态推断
- **问题**: 项目文档把 rustfmt 列为必须门禁，但 CI 没跑。代码若引入格式回归，本地 `cargo fmt --check` 会失败，却能通过 CI 合并，破坏「本地与 CI 门禁一致」的约定。
- **建议**: 在 CI 的 clippy 步骤之前加入 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`，与 `.claude/CLAUDE.md` 对齐。

### [中] `src-tauri/Cargo.toml` 将测试专用 crate 放在 `[dependencies]`
- **维度**: 基建
- **证据**:
  - `src-tauri/Cargo.toml:46`：`tempfile = "3"` 位于 `[dependencies]` 段
  - `src-tauri/Cargo.toml` 全文无 `[dev-dependencies]` 段
  - `grep tempfile::` 在 `src-tauri/src/**/*.rs` 中仅命中测试函数（如 `src-tauri/src/state.rs:494`、`:518`、`:1033`，`src-tauri/src/fs/mod.rs:419` 等），无生产代码调用
- **证据类型**: 静态推断
- **问题**: `tempfile` 是典型的测试隔离工具，却被声明为运行时依赖。这会导致 release 二进制不必要地引入并编译该 crate，增加产物体积与依赖面，也混淆了生产依赖与测试依赖的边界。
- **建议**: 将 `tempfile` 移入 `[dev-dependencies]`；若后续发现生产代码确实需要，再评估是否有更小的替代方案。

### [中] `src/__tests__/setup.ts` 全局 mock 隐性耦合全部 L2 用例
- **维度**: mock 合理性 / 隔离性
- **证据**:
  - `src/__tests__/setup.ts:88-120` 在全局对 `../ipc/notify`、`../ipc/agentHooks`、`@tauri-apps/api/window` 做 `vi.mock`
  - `src/__tests__/setup.ts:78-86` 注释已承认「本 vi.mock 会遮蔽 ../ipc/notify 的真实实现」，并指出需要真实实现的测试必须显式 `importOriginal` 覆盖
- **证据类型**: 静态推断
- **问题**: 全局 mock 让所有 L2 测试默认拿到固定桩（如 `onAgentEvent` 返回 no-op、`inject` 恒返回 `notInjected`）。虽然文档化了覆盖方式，但一旦某个测试忘记覆盖或默认桩的行为被修改，就会出现「改一处、崩一片」的耦合风险；同时也增加了新测试无意中依赖桩而非真实契约的概率。
- **建议**: 评估将部分全局 mock 下沉到真正需要的测试文件，或在 setup 中提供「默认真实 + 需要时显式 mock」的 opt-in 模式，减少隐式耦合。

### [中] CI job 与关键步骤均未设 `timeout-minutes`
- **维度**: CI / 稳定性与确定性
- **证据**:
  - `.github/workflows/ci.yml:11` job `build-and-test` 无 `timeout-minutes`
  - `.github/workflows/ci.yml:28-92` 各步骤均无 `timeout-minutes`
- **证据类型**: 静态推断
- **问题**: L1 含真实 ConPTY 集成测试，L4 需启动真实二进制并通过 WebDriver 通信，任一环节卡住（ConPTY 死锁、E2E 会话未就绪、WDIO 端口占用）都会占用 GitHub runner 直到默认 6 小时超时，浪费资源并严重延迟反馈。
- **建议**: 为 job 和各慢步骤设置合理的 `timeout-minutes`（例如 clippy 15 min、L1 30 min、E2E build+test 60 min），优先保护 ConPTY/E2E 步骤。

### [低] CI 未缓存前端依赖
- **维度**: CI / 基建
- **证据**:
  - `.github/workflows/ci.yml:17-20` `actions/setup-node` 未设置 `cache: npm` 或 `cache-dependency-path`
- **证据类型**: 静态推断
- **问题**: 每次 CI 都执行完整 `npm ci`，前端依赖无缓存，既拉长流水线，又提高网络/ registry 波动导致的偶发失败概率。
- **建议**: 在 `actions/setup-node` 中加入 `cache: npm`（或 `cache-dependency-path: package-lock.json`）。

## 审查覆盖声明

- 审阅文件（14 个）:
  - `.github/workflows/ci.yml`
  - `package.json`
  - `vitest.config.ts`
  - `vitest.l3.config.ts`
  - `e2e-tests/wdio.conf.ts`
  - `src-tauri/Cargo.toml`
  - `src/__tests__/setup.ts`
  - `src/__tests__/testMocks/xterm.ts`
  - `src/__tests__/testMocks/explorerMocks.ts`
  - `src/__tests__/helpers/ipc-contract.ts`
  - `.claude/test-inventory.md`
  - `.claude/CLAUDE.md`（静态门禁定义）
  - `src-tauri/src/**/*.rs`（通过 `grep` 抽样核对 `#\[test\]` 与 `tempfile` 使用）
  - `e2e-tests/*.e2e.ts`（枚举核对）

- 执行命令与结果:
  - `npx tsc --noEmit` → 通过
  - `npx eslint src/` → 通过
  - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` → 通过
  - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` → 通过
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` → 通过，共 726 条测试
  - `npx vitest run` → 通过，154 文件 / 2635 条测试
  - `npx vitest run --config vitest.l3.config.ts` → 通过，7 文件 / 138 条测试
  - `grep` 枚举 `e2e-tests/*.e2e.ts` 中 `it(`/`it.skip(` → 9 spec，共 40 条用例（38 active + 2 skip）
