---
paths:
   - "plan/execute/**/*结果验证.md"
description: "Phase 验收审计方法论——记录 Phase 0/1 审计积累的审查流程、常见陷阱、技术事实速查和证据判断准绳"
---

# Phase 验收审计规则

本文件在读取 `plan/execute/` 下以"结果验证"结尾的 md 文件时自动加载。记录验收审计方法论、证据判断准绳与常见陷阱。

## 1. 核心原则

**先查决策表 + 取一手证据，再下"失实"结论。** 三次误判均因跳过此步。

标"失实"前必须查 `plan/execute/Phase N 执行计划.md` 中的决策台账（E1–E5 等），确认报告声称是否与既定决策一致。决策表说了"去自定义 CI 检查"，就不要要求"加 CI 检查"。

## 2. 证据判据（从强到弱）

| 层级 | 来源 | 示例 | 可信度 |
|------|------|------|--------|
| L0 | 实跑命令 | `cargo test`、`npm run wdio`、`npx tauri info` | 最高 |
| L1 | 一手源文件 | `action.yml`（GitHub raw）、`Cargo.lock`、`ci.yml` | 高 |
| L2 | 项目决策表 | 执行计划中的 E1–E5 决策 | 高（既定决策是审计上限） |
| L3 | 联网一手资料 | crate 官方文档、Tauri changelog、npm registry | 中 |
| L4 | 二手搜索总结 | Web 搜索 AI 摘要、社区博客 | 低 |
| L5 | 被审报告自述 | 报告中的声称 | 待验证 |

- **只有 L0–L2 证据矛盾时才可标"失实"**。L3–L4 仅作线索。
- CI 全绿需 `gh run view` 独立复核；本环境若无 `gh`，标明"未独立复核"。

## 3. 审计流程

1. **读报告** → 提取所有"声称"。
2. **查决策表** → `grep -n '^| E' plan/execute/Phase\ N\ 执行计划.md`。
3. **取一手证据** → 按 §2 层级逐条验证代码、锁文件、action 默认值、`npx tauri info` 等。
4. **实跑命令** → L1 + L2 + L4 至少跑一遍。
5. **联网检索** → 对不确定技术点搜索后汇总，二手总结仅作线索。
6. **问题分类** → 失实 / 缺漏 / 小修建议 / 已知限制。
7. **grill 决策** → 每个需用户定夺的发现一次只问一个问题，给推荐答案（作为第一个选项）。

## 4. 技术事实速查

### 4.1 CI 与版本

- `actions-rust-lang/setup-rust-toolchain@v1` 的 `rustflags` 输入**默认值 = `"-D warnings"`**。不传 `with: rustflags` ⇒ CI 已启用 `-D warnings`；传 `rustflags: ""` 才是关闭。判断 action 默认值以 `action.yml` 为准。详见记忆 `setup-rust-toolchain-default-deny-warnings.md`。
- Tauri CLI 2.8+ 内置 JS/Rust 版本兼容性检查；`npx tauri info` 无 mismatch 提示 ⇒ pair 兼容。项目 E4 决策已定"精确钉 + tauri build 内置检测"为一致性方案，不要建议额外 CI 检查。
- `@tauri-apps/api` 最新即 2.11.1（无 2.11.3），Rust/JS patch 天然不同步是 Tauri 发布常态。
- 精确锁版本 = package.json/Cargo.toml 无 `^`/`~` + lockfile 已提交。`Cargo.lock` 中已解析版本（如 `tauri-plugin-wdio-webdriver v1.1.0`）是运行的精确版本；Cargo.toml 中的 `"1"` 只是 spec。

### 4.2 E2E / L4

→ 细节与命令见 `e2e-tests/CLAUDE.md`。

- embedded driver（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver`）依赖 `webview2-com`，**零 msedgedriver 依赖**。
- `browser.getTitle()` 取 `document.title`，可能因加载时序返回初始值，应用 `waitUntil` 轮询等待。
- `wdio.conf.ts` specs 必须指向实际 spec 文件，`capabilities` 必须配 `browserName:'tauri'`。

### 4.3 Tauri 插件权限

- **不能仅凭"capabilities 中没有 X 权限"就判"缺权限"**。必须先确认 X 权限是否真实存在于该插件权限清单中。
- 验证路径：
  1. 查 crate 源码 `permissions/` 目录 → 无目录 = 插件不注册 Tauri 权限。
  2. 查 crate 源码 `#[tauri::command]` 注解 → 无注解 = 无 IPC 命令。
  3. 查生成的 `src-tauri/gen/schemas/acl-manifests.json` → 无条目 = 权限标识符在 Tauri ACL 中不存在。
  4. 若 1/2/3 全否，添加该权限会导致 `cargo build` 硬失败（`UnknownPermission`）。
- **纯 JS 注入类插件**不经过 Tauri 权限系统，无需在 capabilities 中放行。
- 例：`tauri-plugin-prevent-default` v5.0.0 Flags 枚举 10 个常量（FIND/CARET_BROWSING/DEV_TOOLS/DOWNLOADS/FOCUS_MOVE/RELOAD/SOURCE/OPEN/PRINT/CONTEXT_MENU），**无 COPY/PASTE**；`init()` = `Flags::all()`；插件无 `permissions/`、无 IPC 命令；`Builder::new()` 与 `Builder::default()` 等价。WebView2 `AreBrowserAcceleratorKeysEnabled = false` 明确保留文本编辑键（Ctrl+C/V/X/A/Z）。

### 4.4 WebView2 快捷键

→ 三层控制详见 `src/features/shortcuts/CLAUDE.md`。

- Ctrl+T、Ctrl+N、Ctrl+W 在 WebView2 中硬编码关闭，但键盘事件可穿透到 DOM。
- 判断快捷键被阻止需分三层：WebView2 硬编码 / 插件 Flags / xterm.js 自定义 handler。

### 4.5 Dockview 与终端面板

→ 终端面板生命周期与 `renderer: 'always'` 要求见 `src/panels/CLAUDE.md`；布局/多实例语义见 `src/workspace/CLAUDE.md`。

- 终端面板必须用 `renderer: 'always'`，否则切换标签卸载组件会杀 PTY。
- `addPanel()` 若传入 `renderer: 'always'` 定义但未实际传入，应标"代码死配置"。

### 4.6 测试质量

- **永真断言**不验证行为，例：`expect(result.length).toBeGreaterThanOrEqual(0)`。
- **`term.write('^C')` ≠ 验证按键编码**：前者测终端解析文本，后者测实际按键产生的字节序列。
- L3 键盘测试应验证实际按键字节序列（如 `\x1b[Z]`、`\x03`）。

### 4.7 依赖冗余

- npm 包 `codemirror` v6 是 CM6 组合入口（导出 `basicSetup`），非冗余。
- `@codemirror/basic-setup` ^0.20.0 是旧名包，已被 v6 替代；如存在且无引用 → 冗余。
- 审计时 Grep 确认包是否被 `import`，再查 npm 发布历史。

### 4.8 Dockview `onReady` 同步 `addPanel` 竞态

- 已知 bug（dockview-core #341 / #866）：React Strict Mode 下 `onReady` 触发两次，同步 `addPanel` 可能操作已释放资源（"resource is already disposed"）。
- 变通：`useState` + `useEffect` 延迟调用，将 api 存入 state，下一渲染 tick 创建面板。
- 审计检查项：发现 `onReady` 回调中直接调用 `api.addPanel()` 应标潜在竞态。

### 4.9 静默失败路径

- 特征：`if (!x) return;` 无 `console.warn`/`console.error`。
- 自动化测试中不触发（mock 了写操作），人工验收表现为"无反应"而非报错。
- 审计时应专门检查此类路径，确认正常使用场景下是否可达。

### 4.10 xterm.js 数据流

→ 终端输入/输出、合帧、控制字符处理详见 `src/panels/CLAUDE.md`。

- `term.input(data)` → 仅触发 `onData`，不经 parser/buffer（前端→后端输入路径）。
- `term.write(data)` → 经 parser→buffer，不触发 `onData`（后端→前端渲染路径）。
- `serialize()` 从 buffer 单元格反向重建 ANSI，**无法恢复已被 parser 消费的字节**（如 `\x03` ETX）。
- 测试策略：serialize 验证 buffer 渲染结果；`onData` 验证 parser 前原始字节（按键编码）。两者不可替代。

### 4.11 serde tagged enum + rename_all 一致性

- `#[serde(tag = "type", content = "data", rename_all = "camelCase")]` 会把 Rust 变体名 `Output` 序列化为 `"output"`，若 TypeScript 写 `=== "Output"` 则永远失败 → PTY 输出静默丢弃。
- 审计检查项：
  1. 对每个 tag enum 逐变体列出 serde 实际 tag 值。
  2. 交叉核验 TypeScript 字面量类型与 `===` 匹配字面量。
  3. 用 DevTools Console `Object.keys()` 核验实际 IPC 消息（L0）。

### 4.12 IPC 通路端到端覆盖

- 对每个 IPC 数据通路（invoke → Rust → Channel → 前端 handler → DOM/term 写入），检查是否存在 roundtrip 验证。
- 若无 roundtrip → 标"IPC 通路无 E2E 覆盖"。
- 至少对每个 `#[serde(tag = "...")]` 枚举的每个变体分支验证一次。

### 4.13 npm 依赖 CSS 导入完整性

- 对含 CSS 样式的 npm 依赖（xterm.js、CodeMirror、Dockview、Monaco 等），Grep 确认 `import '...css'` 存在。
- 缺失 CSS import → 标"阻断"，因 jsdom 测试不可检测。
- 验证方式：读 `package.json` 的 `style`/`css` 字段；查 `node_modules/<pkg>/css/`；Grep 项目 `src/` 对应 import。

### 4.14 Vite/ESBuild 预构建损坏枚举/常量

- 修改 `node_modules` 调试前，确认实际加载文件在 `node_modules/.vite/deps/`（Vite 预构建产物），非 npm 原始 `dist/`。
- 修改 `node_modules` 后必须清 Vite 缓存：`rm -rf node_modules/.vite`。
- ESBuild 可能错误 tree-shake 枚举/常量，使原本字符串字面量变为 `undefined`。可在关键位置添加直接引用验证。
- 修复方向：用 `optimizeDeps.exclude` 排除受影响依赖，避免打包为单一 chunk。
- 审计检查项：第三方 UI 库行为异常且逻辑代码无误时，检查 `node_modules/.vite/deps/` chunk，grep 关键常量，必要时 `optimizeDeps.exclude` + 清缓存重试。

## 5. version-pins.md 流程

1. **开红旗** → `version-pins.md` 新增条目，描述风险与条件。
2. **补救/收口** → 修改代码/配置消除风险，或在报告记录"已评估可接受"。
3. **关红旗** → 原条目追加"已收口"或"不再适用"，附证据（如 `tauri info` 无 mismatch、Cargo.lock 锁定版本）。
4. **审计核验** → 对照报告与 `version-pins.md`，确认每个红旗的收口状态与报告一致。

## 6. 项目记忆索引

- `setup-rust-toolchain-default-deny-warnings.md`：action rustflags 默认 `-D warnings`；`rustflags:""` 才禁用。
- `audit-verify-before-flagging.md`：标"失实"前查决策表 E1–E5 + 取一手证据。
- `edit-chinese-md-with-builtin-tools.md`：中文文档用内置 Read+Edit，filesystem mcp 有乱码风险。

刷新记忆时用 `Read` 查阅，过时或错误时 `Write` 更新。

## 7. 审查交付规范

- **审计报告** → `plan/execute/Phase <N> 验收审计.md`。逐条对照 DoD，含独立验证记录、失实/缺漏清单、决策台账、收口路径。
- **grill 节奏** → 一次只问一个问题，附带推荐答案选项。
- **措辞精准** → "失实"仅用于 L0–L2 证据明确矛盾的情况。不确定时用降级措辞（"时序敏感""弱证据""待确认"）。
- **收口** → commit + push + `gh run watch` 确认 CI 全绿后，方可声明 Phase 完成。

## 8. 审计报告的交叉核验

审计报告产出后，其核验批注本身也可能出错。**审查→核查→核验的每层都可能引入新错误，没有哪层天然免疫。**

- 核查批注中的事实性声明（PR 状态、commit 归属、版本号、API 行为）须独立 agent + API 交叉验证。
- 关于 Tauri 权限的结论必须查 crate 源码（`permissions/` 目录 + `#[tauri::command]` + `acl-manifests.json`）。
- 用户补充的核验结论 ≠ 终局真相，审计者应以同等证据标准核验。

递归验证模式：

```
第一轮：spawn N agent → 审查报告
第二轮：spawn N agent → 核验用户核查批注
第三轮：spawn N agent → 核验第二轮
停止条件：连续两轮无新增事实性错误
```

## 9. 自动化通过 ≠ 端到端正常

- grep 验证的是**代码存在性**；单元测试验证的是**隔离行为**；两者都不等于**端到端用户体验**。
- 收口审计必须包含：人工验收清单的实际操作记录、对静默返回路径的代码审查、grep 结果仅在确认代码可被执行到时才构成证据。
- 某些竞态只在真实 WebView2 中暴露（如 Dockview `onReady` 同步 `addPanel`），jsdom 的 `ResizeObserver` 是 stub。

## 10. 其他常见陷阱

- **Tauri 2 + WebView2 全窗口关闭 freeze 风险**：关闭所有面板 → 级联 `pty.kill()` → `reader_handle.join()` 同步等待 → WebView2 事件循环死锁。规避：保留至少一个面板；kill 改为异步 fire-and-forget；升级到 dockview-core ≥4.2.2 修复 onReady 竞态。
- **"随 X 自动恢复"假设必须经代码路径追踪确证**：不同创建路径（onReady / Watermark / RHA / 右键菜单）可能经过不同 Dockview 内部代码路径。修复项之间的因果声称须经代码路径追踪 + 人工验收确证。
- **mock 全部核心逻辑的测试不计入功能覆盖率**：`vi.mock` 把 `useXterm` / `useCodeMirror` 完全 mock 后，测试仅验证 React 渲染 `<div>`，应标"零覆盖测试"，核心 hook 的 mock 视为技术债务。
- **插件 import 也应受架构约束**：`@tauri-apps/plugin-*` 暴露的函数同样是 OS 级 IPC 调用，应经 `src/ipc/` 封装。→ 详见根 `CLAUDE.md` 硬约束 #1 / `src/ipc/CLAUDE.md`。
- **死依赖检测**：审计时 Grep 每个依赖包名在 `src/` 中的出现次数；建议 CI 加入 `knip`/`depcheck`。
- **Dockview 容器布局时序**：`addPanel()` 后 Dockview 通过 `rAF` 异步提交 CSS 布局，Watermark 空态下新面板容器首次渲染时尺寸可能为 0×0。修复模式：双 `rAF` 延迟再 `fit()`；ResizeObserver 加非零尺寸守卫；面板首次可见时设初始尺寸。
