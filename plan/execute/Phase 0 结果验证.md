# Phase 0 结果验证

验证日期：2026-06-18

---

## 一、已通过的自动化验证

### 1. 前端构建 `npm run build`

- **验收标准**：Vite 构建退出码 0，产物在 `dist/`
- **证据**：`vite v8.0.16 building client environment for production...` → `✓ built in 111ms`，dist/index.html + CSS + JS 产出

### 2. 后端构建 `cargo build`（src-tauri/）

- **验收标准**：零 error，编译通过
- **证据**：`Compiling slterminal v0.1.0` → `Finished dev profile [unoptimized + debuginfo]`，无 warning 被提升为 error

### 3. 后端单测 L1 `cargo test`（src-tauri/）

- **验收标准**：退出码 0，含 `test_app_error_serialization ... ok`
- **证据**：`test error::tests::test_app_error_serialization ... ok`，序列化 JSON 含 `"测试错误"` 与 `"Io"` 字段（`error.rs:30–32`）

### 4. 前端单测 L2+L3 `npm test`

- **验收标准**：Vitest 退出码 0，2 passed
- **证据**：L2 mockIPC ping → `expect(result).toBe('pong')` 断言通过；L3 headless Terminal feed "hi" → `expect(result).toContain('hi')` 断言通过（`terminal-serialize.test.ts:21`）

### 5. Debug 构建产物 `npm run tauri build -- --debug --no-bundle`

- **验收标准**：成功产出 exe
- **证据**：`Built application at: D:\data\learn\code\slTerminal\src-tauri\target\debug\slterminal.exe`，`Test-Path` 返回 `True`

### 6. ESLint 守卫

- **验收标准**：`@tauri-apps/api/core` 的 `invoke` 在 `src/ipc/` 外 import 触发 `no-restricted-imports` error；删除反例后 `npx eslint src/` 通过
- **证据**：`eslint.config.js:9–14` 已配置 `no-restricted-imports` 规则，仅 `src/ipc/**/*.ts` 放行（第 19–23 行）

### 7. 目录骨架

- **验收标准**：前端 8 目录 + 后端 5 模块各有占位文件，`cargo build` 与 `npm run build` 均通过
- **证据**：前端 `src/ipc/ types/ stores/ workspace/ panels/ features/ theme/ lib/` 各含 `index.ts`；后端 `src-tauri/src/pty/ fs/ git/ claude/ notify/` 各含 `mod.rs`

### 8. CI 配置

- **验收标准**：`.github/workflows/ci.yml` 含完整 pipeline
- **证据**：job `build-and-test` 包含 lint→build→test L1/L2/L3→tauri build→E2E（`continue-on-error: true`），`on.workflow_dispatch` 手动触发，`rustflags: ""` 防止 dead_code warning 阻塞

### 9. 版本依赖

- **验收标准**：所有依赖与 `plan/version-pins.md` 对齐
- **证据**：`Cargo.toml``tauri 2.11.3`/`portable-pty 0.9.0`/`git2 0.21.0`；`package.json``react 19.2.7`/`vite 8.0.16`/`@xterm/xterm ^6.0.0`/`dockview-react 6.6.1`/`@tauri-apps/cli 2.11.2`，钉表红旗已记录

### 10. 硬约束守规

- **验收标准**：未使用 `#[allow(dead_code)]`、未删改测试断言、未放宽 lint 规则、未引入业务功能
- **证据**：全仓 grep `#[allow(dead_code)]` 零命中；`eslint.config.js` 仅设 `no-restricted-imports` 一条规则、且 `error` 级别未弱化为 `warn`；测试断言保持原样；模块仅含占位 `mod.rs`，无业务代码

---

## 二、待人工验证

### 1. CI GitHub Actions 全绿

**操作**：
1. `git push origin main`
2. 浏览器打开 https://github.com/LiuShenLan/slTerminal/actions
3. 确认最新 run 的 `build-and-test` job 所有 step 为绿色勾

**验证结果**（请填空）：

| 项目 | 结果 |
|------|------|
| CI 全绿 | ☐ 通过 / ☐ 未通过 |

### 2. 暗色主题窗口

**操作**：
1. 运行 `npm run tauri dev`
2. 观察弹出的窗口背景是否为暗色（非白色/亮色）
3. 确认标题栏等 chrome 元素也跟随暗色

**验证结果**（请填空）：

| 项目 | 结果 |
|------|------|
| 窗口暗色主题 | ☐ 通过 / ☐ 未通过 |

### 3. Dockview 填充窗口

**操作**：
1. 在 `npm run tauri dev` 启动的窗口中观察
2. 确认 Dockview 区域填满窗口客户区，无空白/白边
3. 布局为空状态时显示暗色背景，无异常留白

**验证结果**（请填空）：

| 项目 | 结果 |
|------|------|
| Dockview 填满窗口 | ☐ 通过 / ☐ 未通过 |

### 4. 控制台无报错

**操作**：
1. 启动 `npm run tauri dev` 后观察终端输出
2. 确认无 `ERROR`、`panic`、`unreachable` 等异常输出
3. 按 F12 打开 DevTools，确认 Console 无红色错误

> 注意：`libpng warning: iCCP: known incorrect sRGB profile` 来自 WebView2 内部渲染管线，非我方代码，属已知无害警告，可忽略。

**验证结果**（请填空）：

| 项目 | 结果 |
|------|------|
| 控制台无报错 | ☐ 通过 / ☐ 未通过 |
