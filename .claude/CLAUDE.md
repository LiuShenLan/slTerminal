# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目性质

**slTerminal** — 面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。

定位约束（贯穿全程，不可违背）：
- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）。

## 开发取向（未来最优）

长期迭代、无 deadline 项目：所有开发决策以**未来最优**为准——宁要未来的正确与简洁，不迁就历史包袱与既有妥协。权衡取舍时不计成本项：

- **git 历史**：允许大重构/重写/改名，不因破坏历史脉络而否决。
- **改动工作量**：允许为长远正确投入短期成本，不因"改动量大"否决。
- **现状兼容**：允许 breaking change——DTO/接口/布局/存储形态一律按理想终态重设计，不做兼容过渡。

适用边界：

- 对未来新决策生效；既有登记妥协（ADR 关闭项、test-exemptions、豁免登记）**不自动作废**，但重审或提案一律以本原则为准，「动现状成本高/风险大」不再构成否决理由。
- 不推翻：产品定位约束（Windows 原生/单窗口/仅暗色/复制语义等）、用户已明确否决的方向、安全与正确性纪律（硬性开发约束、测试覆盖门禁）。

## 文档规范

- **代码自证原则（ADR-0011）**：凡能通过阅读代码直接理解的信息（模块职责、文件清单、入口路径、接口签名、数据流、技术栈、现状描述等）一律不写入文档——此类内容必然腐化为失真文档。文档只记录代码无法自证的信息：设计决策与原因（why）、外部依赖的坑与红线、操作指令、约定与豁免登记。
- **渐进式披露**：读取某目录代码时，Claude Code 自动加载该路径的 CLAUDE.md（渐进式披露的物理基础），无需登记索引。
- **收录判定**：跨 ≥2 个模块适用、或每次会话必需的指令入根文件；仅触碰某模块才需要的归该模块子路径 CLAUDE.md。记录规则/说明/条件/经验/踩坑等文本内容时同样按此归位，注意精简/下沉。
- **子文件维护**：新建模块目录时同步创建该路径 CLAUDE.md（模板结构读任一现有子文件即知，不在此复述）；改动约定/决策/红线时同步对应 CLAUDE.md——只改实现不改约定时无需动文档。
- **短标识符解码**：代码注释中的短标识符（SEC-*/B*/FE-*/IC-* 等）就近在所属模块 CLAUDE.md 定义，根文件不设编号索引。
- 配套文档：领域术语表 `@../CONTEXT.md`；架构决策记录 `@.claude/adr.md`；自动化测试豁免与定位 `@.claude/test-exemptions.md`。

## 架构（两进程模型）

**Rust 后端拥有一切 OS 访问；Web 前端只做 UI，经 IPC 调用后端。**（技术选型清单读 package.json / Cargo.toml 即得，不复述。）

目录结构原则：实现落到既定分层，不另起炉灶。

## 硬性开发约束（新增功能必须遵守）

1. **前端绝不直接碰 OS/文件/进程**：`invoke` 只允许出现在 `src/ipc/`；其它文件只调用 `ipc/` 暴露的领域函数（→ ../src/ipc/CLAUDE.md）。
2. **后端按功能分模块**：模块间不互相穿透，共享只经 `state.rs` 的 `AppState`（→ ../src-tauri/src/CLAUDE.md）。
3. **命令统一注册**于 `lib.rs` 的 `generate_handler!`；一律返回 `Result<_, AppError>`；阻塞 I/O 用 `spawn_blocking`（→ ../src-tauri/src/CLAUDE.md）。
4. **DTO 双边对应**：`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。字段类型泛化后，其语义值集须在 CLI profile 与后端 provider 同步登记，并配合同步测试锁死一致性（→ ../src/types/CLAUDE.md）。
5. **面板封闭**：Dockview 面板只能是 `panels/` 下注册过的类型；新增类型 = 加目录 + 在 `panelRegistry.ts` 注册。合法形态含「hub 容器 + 注册表分派子编辑器」（→ ../src/panels/CLAUDE.md）。
6. **配色单点**：颜色定义于 `theme/schemes/<scheme>.ts`，组件经 `theme/colors.ts` facade token 引用，禁止硬编码颜色；既定例外清单及新增例外须同步登记对应模块 CLAUDE.md（→ ../src/theme/CLAUDE.md）。
7. **布局单点**：操作页面布局只经 `workspace/layoutSerde.ts` 用 Dockview `toJSON/fromJSON` 存取（→ ../src/workspace/CLAUDE.md）。
8. **会话元数据单点**：PTY 进程映射仅在 `panels/terminal/TerminalRegistry`（模块级 Map）管理；面板只订阅，不自存（→ ../src/panels/CLAUDE.md）。
9. **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在 pty 模块，业务逻辑不撒 cfg；测试 `#[cfg(windows)]` 原则上改运行时 `cfg!(windows)` 分支（→ ../src-tauri/src/pty/CLAUDE.md）。
10. **权限最小化**：Tauri 2 自定义命令默认放行，`capabilities/` 只管插件权限；不追加通配 `*`（→ ../src-tauri/src/CLAUDE.md）。
11. **测试覆盖**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在 `.claude/test-exemptions.md` 既定豁免清单登记，注明豁免原因与当前兜底层级，禁止未登记豁免。
12. **store 纯状态**：`src/stores/` 只存状态与状态转换，不存业务逻辑（校验/映射/编排放注册表、纯函数或上层组件）；持久化一律经 `src/ipc/` 对应领域函数，禁止在 store 内直接调用 `invoke`；禁止跨 store 隐式依赖（→ ../src/stores/CLAUDE.md）。
13. **注册表家族通用契约**：注册表类模块统一形态——模块级单例、`register(...)` / `getAll()` 接口、`_reset()`（仅测试用）；注册经 side-effect import 触发；测试 beforeEach/afterEach 调 `_reset()` 保证用例隔离（→ ../src/features/settingsCenter/CLAUDE.md）。
14. **git 追踪文件凭据红线（SEC-18）**：真实凭据值（API token/key、Authorization 头实际值）禁止写入任何 git 追踪文件——代码、测试夹具、文档、脚本一律不行；测试与文档仅允许假值占位符（`sk-test` 形态）。真实凭据只存 user 层 `~/.claude/settings.json`（仓库外）（→ ../src-tauri/src/plan_balance/CLAUDE.md）。

## 命令

- 开发运行：`npm run tauri dev`
- 构建：`npx tauri build --debug --no-bundle`
- **测试/使用流程（用户固定习惯）**：永远用 `npx tauri build --debug --no-bundle` 构建产物测试与使用，**不使用 dev 模式**（`npm run tauri dev` 仅保留为开发兜底）。产出 exe + dll（debug 模式），部署到本机或 win10 另一台 PC 使用

## 测试策略

四级测试金字塔，按执行速度和隔离度分层。豁免登记与测试定位 → `@.claude/test-exemptions.md`。

| 层级 | 名称 | 技术栈 | 运行命令 |
|------|------|--------|----------|
| L1 | Rust 单元/集成 | `cargo test`、`tempfile` 隔离 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |
| L2 | 前端单元/集成 | Vitest + jsdom | `npm test` |
| L3 | 终端 headless 渲染 | Vitest + `xterm/headless` | `npm run test:l3` |
| L4 | 端到端 (E2E) | WDIO + embedded driver | `npm run e2e`（= `build:e2e` + `wdio`） |

核心原则：
- **隔离优先**：L1 用 `tempfile::tempdir()` 隔离文件系统、`SPAWN_LOCK` 串行化 PTY；L2 用 `vi.mock()` 隔离 IPC/终端库；L4 用 embedded driver 隔离浏览器依赖
- **L1/L2 覆盖所有 PR**，L3/L4 覆盖关键路径变更
- **bugfix 须附防复发测试**：修复缺陷时除对改动代码添加全量测试外，还须对照修复前老代码补回归用例，防旧问题复现
- **模块测试模式见各子路径 CLAUDE.md**，不在根文件展开

### 测试命名约定

- **文件级（kebab-case）**：L2/L3 测试文件名 = 「领域对象-能力行为」（如 editor-keyboard.test.ts、theme-colors.test.ts）；禁止裸通用词（keyboard/path/colors/osc 等必须带领域对象）；成对文件用维度后缀消歧（如 main-bootstrap-poll / main-bootstrap-ipc-timeout）；L4 spec = 「领域.e2e.ts」且须登记 e2e-tests/wdio.conf.ts 显式 specs 数组（顺序即执行序，terminal.e2e.ts 末位承载杀 app 用例）；L1 tests/ 集成测试 = `<cmd>_tests.rs`（snake_case，如 git_status_tests.rs）
- **L1 内嵌测试（src-tauri/src/ 源文件内）**：测试模块全部领域具名 `mod <领域>_tests`（如 reader_tests、spawn_tests；一文件多测试模块用功能具名）；测试函数 = 「对象_行为_场景」snake_case 裸名（如 micro_batch_stops_at_limit、pwsh_args_no_noprofile_b17），禁止 test_ 前缀与规格编号前缀

静态检查门禁：
- TypeScript：`npx tsc --noEmit`
- ESLint：`npx eslint src/`
- Clippy：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- rustfmt：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

### 发布打包

**一键打包**：`.\.claude\package.ps1 -Version "0.1.0"`（release 模式，单文件 exe → zip）
加 `-Debug` 用 debug 模式（exe + dll 两个文件）。
