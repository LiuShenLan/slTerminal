# Phase 0 结果验证

验证日期：2026-06-19 | 验证轮次：终版（D1–D7 + R1–R7 全部收口，CI 真绿）

---

## 一、自动化验证

### 1. 后端单测 L1 `cargo test`

- **验收标准**：退出码 0，含 AppError 序列化样例 + 集成测试 PASS
- **证据**：`test_app_error_serialization ... ok`，`test_lib_crate_linked ... ok`，共 2 passed

```
running 1 test
test error::tests::test_app_error_serialization ... ok

running 1 test
test test_lib_crate_linked ... ok
```

### 2. 前端单测 L2+L3 `npm test`

- **验收标准**：Vitest 退出码 0，2 passed（L2 mockIPC + L3 terminal-serialize）
- **证据**：Test Files 2 passed, Tests 2 passed

```
Test Files  2 passed (2)
     Tests  2 passed (2)
```

### 3. L3 终端渲染

- **验收标准**：feed "hi" → serialize 结果含 "hi"
- **证据**：`test/terminal/terminal-serialize.test.ts` 断言 `expect(result).toContain('hi')` 通过

### 4. L4 E2E

- **验收标准**：`npm run wdio` 退出 0，`waitUntil` 等标题就绪 + `toBe('slTerminal')` 精确断言 PASS
- **证据**：`waitUntil` 轮询到 `slTerminal` 后断言通过

```
[0-0] COMMAND getTitle()
[0-0] RESULT slTerminal
[0-0] PASSED in undefined - file:///.../e2e-tests/test.e2e.ts
Spec Files:  1 passed, 1 total (100% completed)
```

- **实现**：`@wdio/tauri-service` + `driverProvider: 'embedded'` + `tauri-plugin-wdio-webdriver`（Rust 端，v1.1.0）。零 msedgedriver 依赖。本地自动切换便携 Node 22（Node 26 undici 8 与 webdriverio 9.28 不兼容），CI 固定 Node 22。

### 5. Debug 构建 `npm run tauri build -- --debug --no-bundle`

- **验收标准**：成功产出 exe
- **证据**：`Built application at: D:\data\learn\code\slTerminal\src-tauri\target\debug\slterminal.exe`

> `--no-bundle` 偏差说明：省 bundling 时间，仍产 exe（`tauri build --debug` 标准用法）。5.0/4 原文写 `tauri build --debug`，本质等价。

### 6. ESLint 守卫

- **验收标准**：`@tauri-apps/api/core` 的 `invoke` 在 `src/ipc/` 外 import 触发 `no-restricted-imports` error
- **证据**：`eslint.config.js` 已配置 `no-restricted-imports` 规则，仅 `src/ipc/**/*.ts` 放行

### 7. 目录骨架

- **验收标准**：前端 8 目录 + 后端 5 模块 + 测试目录布局对齐 5.0 §3
- **证据**：

```
e2e-tests/
  wdio.conf.ts
  test.e2e.ts
test/terminal/
  terminal-serialize.test.ts
src-tauri/tests/
  integration_test.rs
src/__tests__/
  ipc-ping.test.ts
  setup.ts
```

前端 `src/ipc/ types/ stores/ workspace/ panels/ features/ theme/ lib/` 各含 `index.ts`；后端 `src-tauri/src/pty/ fs/ git/ claude/ notify/` 各含 `mod.rs`

### 8. CI GitHub Actions 全绿

- **验收标准**：`windows-latest` runner 所有 step 绿色勾，含 L4 E2E（无 `continue-on-error` 掩盖，无 msedgedriver 死代码）
- **证据**：`gh run view` 输出全 13 step ✓，耗时 5m14s

```
✓ build-and-test in 5m14s (ID 82263199995)
  ✓ Run actions/checkout@v5
  ✓ Setup Node.js (22)
  ✓ Setup Rust toolchain (-D warnings 默认生效)
  ✓ Cache Rust dependencies
  ✓ Install frontend dependencies
  ✓ Lint check
  ✓ Frontend build
  ✓ Frontend tests (Vitest L2 + L3)
  ✓ Backend build
  ✓ Backend tests (L1)
  ✓ Tauri debug build
  ✓ E2E tests (L4)
  ✓ Complete job
```

- CI 要点：`actions/checkout@v5` + `actions/setup-node@v5`（消 Node 20 deprecation）；`node-version: '22'`（避 Node 26 undici 不兼容）；Setup Rust toolchain 无 `rustflags: ""`（恢复默认 `-D warnings`）；L4 step 无 `continue-on-error`（真绿）；无 msedgedriver step（embedded driver 用 webview2-com，零依赖）

### 9. 版本依赖

- **验收标准**：`Cargo.toml` 与 `package.json` 依赖与 `plan/version-pins.md` 对齐
- **关键版本钉**：

| 包 | 版本 | 备注 |
|----|------|------|
| tauri (Rust) | 2.11.3 | 精确钉，lockfile 锁死 |
| @tauri-apps/api (JS) | 2.11.1 | 精确钉（去 caret），`tauri info` 验证 pair 兼容，红旗 #1 已收口 |
| @tauri-apps/cli (JS) | 2.11.2 | 精确钉 |
| tracing-subscriber | 0.3 | 默认 info 级 filter |
| tauri-plugin-wdio-webdriver | 1.1.0 | embedded WebDriver，替代 tauri-driver |
| @wdio/tauri-service | 1.1.0 | L4 launcher（embedded driverProvider） |
| @wdio/tauri-plugin | 1.1.0 | WDIO 插件 types/服务端交互 |
| notify | 9.0.0-rc.4 | Phase 0 占位，红旗 #8 已评估可接受 |

### 10. tracing 接入

- **验收标准**：`cargo run` 无 RUST_LOG 时输出 `slTerminal 启动`（info 级默认可见）
- **证据**：

```
2026-06-19T00:43:20.899493Z  INFO slterminal_lib: slTerminal 启动
```

- **实现**：`lib.rs` 使用 `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))`

### 11. 硬约束守规

- **验收标准**：不弱化断言、不放宽 lint、不改动测试逻辑（仅加固 getTitle 时序）、不引入业务功能
- **证据**：
  - getTitle 断言从 `toBeTruthy()` 加固为 `waitUntil` + `toBe('slTerminal')`（保留原有"验证标题"逻辑，增强健壮性）
  - ESLint 仅 `no-restricted-imports` 一条，`error` 级别未弱化
  - `AppError` `#[allow(dead_code)]` 带注释 "Phase 0 占位变体"，属作用域精准 allow
  - 新增代码均为基建性质（tracing、E2E 配置、集成测试占位）

---

## 二、人工验证

### 1. 暗色主题窗口

| 项目 | 结果 |
|------|------|
| 窗口暗色主题 | 通过 |

### 2. Dockview 填充窗口

| 项目 | 结果 |
|------|------|
| Dockview 填满窗口 | 通过 |

### 3. 控制台无报错

| 项目 | 结果 |
|------|------|
| 控制台无报错 | 通过 |

> `libpng warning: iCCP: known incorrect sRGB profile` 来自 WebView2 内部渲染管线，非我方代码，可忽略。

---

## 三、已知限制

1. **Node 26 undici 不兼容**：本地 Node 26 内置 undici 8 与 webdriverio 9.28 的 HTTP 请求构造不兼容（全局 `Headers` 类传入 undici 6 的 fetch 触发 `UND_ERR_INVALID_ARG`）。本地通过便携 Node 22（v22.21.1，`.temp/node22/node.exe`，首次自动下载）自动切换，CI 固定 Node 22。此问题待 webdriverio 适配 undici 8 后自动消除。
2. **`libpng iCCP` 警告**：来自 WebView2 内部渲染管线，非我方代码，属已知无害警告。
3. **`npm test` 噪声**：`HTMLCanvasElement.getContext() not implemented`（jsdom 环境限制，不影响断言）。
4. **L4 teardown 噪声**：`Failed to clear mock store`（`@wdio/tauri-service` session 销毁时序问题，不影响 spec 结果）。
5. **L4 startup 噪声**：`Failed to get window states: Tauri core.invoke not available after 5s timeout`（embedded driver 初始化时序，core.invoke 尚未就绪时窗口状态查询超时，不影响 spec 结果及 getTitle 断言）。

---

## 四、DoD 结论

对照 5.0 §8 DoD（骨架目录 + 四层测试各有可运行样例 + CI 绿 + invoke lint 生效），**Phase 0 达成**。11 项自动化验证全部通过，3 项人工验证通过，CI 真绿（13 step 全 ✓，含 L4 E2E，无 `continue-on-error`，无 msedgedriver 死代码）。
