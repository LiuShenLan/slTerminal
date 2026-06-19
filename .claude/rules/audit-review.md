---
paths:
   - "plan/execute/*结果验证.md"
description: "Phase 验收审计方法论——记录 Phase 0/1 审计积累的审查流程、常见陷阱、技术事实速查和证据判断准绳"
---

# Phase 验收审计规则

本文件在读取 `plan/execute/` 下以"结果验证"结尾的 md 文件时自动加载。记录 Phase 0 三轮 + Phase 1 两轮审计积累的审查方法论、常见陷阱和证据判断准绳。

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

### 3.5 Tauri 插件权限判断（新增于 Phase 1 审计）

- **不能仅凭"capabilities 中没有 X 权限"就判"缺权限"**。必须先确认 X 权限是否真实存在于该插件的权限清单中。
- 验证路径：
  1. 查 crate 源码中是否有 `permissions/` 目录 → 无目录 = 插件不注册 Tauri 权限
  2. 查 crate 源码中是否有 `#[tauri::command]` 注解 → 无注解 = 插件无 IPC 命令
  3. 查生成的 `acl-manifests.json`（`src-tauri/gen/schemas/`）中是否有该插件的权限条目 → 无条目 = 权限标识符在 Tauri ACL 中不存在
  4. 若 1/2/3 全否 → 添加该权限到 capabilities 会导致 `cargo build` 硬失败（`UnknownPermission` 错误）
- **纯 JS 注入类插件**（通过 `js_init_script()` 工作）**不经过 Tauri 权限系统**，无需在 capabilities 中放行。
- **反面案例（Phase 1）**：初审判 `tauri-plugin-prevent-default` "capabilities 缺 `prevent-default:default` 权限"，但该插件无 permissions 目录、无 IPC 命令、纯 JS 注入。添加此权限反会触发构建错误。

### 3.6 tauri-plugin-prevent-default 已知事实（新增于 Phase 1 审计）

- v5.0.0 Flags 枚举 **10 个常量**：FIND、CARET_BROWSING、DEV_TOOLS、DOWNLOADS、FOCUS_MOVE、RELOAD、SOURCE、OPEN、PRINT、CONTEXT_MENU。**无 COPY、无 PASTE**。
- 插件**完全不干扰 Ctrl+C**——WebView2 `AreBrowserAcceleratorKeysEnabled = false` 明确保留文本编辑键（Ctrl+C/V/X/A/Z）。
- `init()` = `Builder::default().build()` = `Flags::all()`——禁用全部已知浏览器快捷键。
- `Builder::new()` 和 `Builder::default()` 均存在且等价。
- 插件无 `permissions/` 目录、无 IPC 命令、纯 JS 注入绕开 Tauri ACL。

### 3.7 WebView2 快捷键已知事实（新增于 Phase 1 审计）

- **Ctrl+T、Ctrl+N、Ctrl+W** 在 WebView2 中**硬编码始终关闭**（always turned off），不可通过设置开关。来源：Microsoft Edge WebView2 特性差异文档。
- Ctrl+W 虽被禁用，但键盘事件**可穿透到 DOM**，被 xterm.js `attachCustomKeyEventHandler` 接收（可用于 bash readline `\x17`）。
- 判断"哪些快捷键被阻止"时，需区分三个层次：① WebView2 运行时硬编码关闭（Ctrl+T/N/W）；② 插件 Flags 覆盖的浏览器加速键（Ctrl+F/P/R 等）；③ xterm.js 自定义 handler 可处理的终端快捷键。

### 3.8 Dockview renderer 与 PTY 存活（新增于 Phase 1 审计）

- Dockview `AddPanelOptions.renderer` 类型：`'onlyWhenVisible'`（默认，切换标签时卸载组件）/ `'always'`（始终保持挂载）。
- **终端面板必须使用 `renderer: 'always'`**——否则切换标签时 React 组件卸载 → cleanup 中 `pty.kill()` → PTY 进程被杀。
- 审计时若发现 `renderer: 'always'` 已定义但未在 `addPanel()` 调用中传入，应标注为"代码死配置"。

### 3.9 测试质量检查要点（新增于 Phase 1 审计）

- **永真断言**：如 `expect(result.length).toBeGreaterThanOrEqual(0)`——对任何输入（含空字符串）恒为真，不验证任何行为。
- **"写入字面文本" ≠ "验证按键编码"**：`term.write('^C')` 测试的是"终端能解析 `^C` 文本"，而非"Ctrl+C 按键产生 `\x03` 字节序列发送到 PTY"。两者是不同层面的测试。
- L3 键盘测试应验证**实际按键产生的字节序列**（如 `\x1b[Z]`、`\x03`），而非仅验证"write 的文本能在 serialize 中找回"。

### 3.10 CodeMirror 依赖辨析（新增于 Phase 1 审计）

- npm 包 `codemirror` v6 是 CM6 **组合入口**（导出 `basicSetup`），非冗余。
- `@codemirror/basic-setup` ^0.20.0 是 `codemirror` v6 的**旧名包**（v0.20 系列已被 v6 替代），如存在且无引用 → 冗余依赖。
- 审计时判断"依赖是否冗余"：先 Grep 确认该包是否被 `import`，再查其 npm 发布历史确认是否已被替代包覆盖。

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

## 7. 审计报告的交叉核验（新增于 Phase 1 审计）

**审计报告产出后，其核验批注本身也可能出错。** Phase 1 审计经历了两轮交叉核验：

- **第一轮**：用户补充核验批注后，4 路 agent 并行检索 + 本地源码审查发现：用户关于 "capabilities 缺权限导致插件完全无效" 的结论不成立（插件纯 JS 注入绕开 ACL）。15 项核验中 2 项需修正。
- **第二轮**：用户修正并再次补充后，agent 检索 + ACL manifest 验证确认本轮变更全部正确。

**教训**：
- 审计报告的 `> 🔍 核验` 批注**也应被核验**——对照 L0–L2 证据（源码、实跑命令、ACL manifest）逐条确认。
- **关于 Tauri 权限的结论必须查 crate 源码**：不能假设每个插件都需要 capabilities 权限。验证标志：`permissions/` 目录 + `#[tauri::command]` 注解 + `acl-manifests.json` 条目。
- **用户补充的核验结论 ≠ 终局真相**——审计者应以同等证据标准交叉核验用户的核验批注。

## 8. 审计决策台账模板（新增于 Phase 1 审计）

每轮 grill 产出的决策应纳入审计报告的独立章节：

```
| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D1 | <议题> | <一句话决策 + 注意事项> | <落点文件> |
```

编号从 D1 开始，连续递增。若某决策在后继核验中被修正（如 D1 中 "capabilities 权限" 被移除），应在决策下方加核验批注说明修正原因和证据。
